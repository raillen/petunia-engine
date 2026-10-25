use crate::spec::SchemaError;

/// What an id names. Commands and queries are `module.verb`; events are `module.event_name`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdKind {
    Command,
    Query,
    Event,
}

/// Abbreviations the engine API never uses (AGENTS.md: full words beat abbreviations).
const BANNED_ABBREVIATIONS: &[&str] = &[
    "dt", "ctx", "mat", "geom", "idx", "cfg", "msg", "tmp", "pos", "vel", "rot",
];

/// Decides whether a verb may appear in a command or query id.
///
/// The design notes propose a small closed set of verbs so the API stays predictable:
/// `create remove get set find on play stop show hide move share`.
///
/// TODO(human): decide the verb policy. A closed set keeps the API easy to learn and predict
/// (and rejects synonyms), but every new verb then needs a recorded decision. An open policy
/// is more flexible but lets synonyms (`delete`, `destroy`, `spawn`) creep in.
/// Replace the body below with your policy; the current placeholder accepts every verb so
/// nothing breaks while you decide.
fn is_allowed_verb(verb: &str) -> bool {
    let _ = verb;
    true
}

/// Checks the shape of an id: exactly two lowercase `snake_case` segments, no banned
/// abbreviation, and (for commands and queries) an allowed verb.
pub fn validate_id(id: &str, kind: IdKind) -> Result<(), SchemaError> {
    let fail = |reason: &str| SchemaError::InvalidId {
        id: id.to_string(),
        reason: reason.to_string(),
    };

    let mut parts = id.split('.');
    let (Some(module), Some(last), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(fail("must have exactly two parts: `module.verb`"));
    };
    for segment in [module, last] {
        let mut characters = segment.chars();
        let starts_with_letter = characters.next().is_some_and(|c| c.is_ascii_lowercase());
        let rest_ok = characters.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !starts_with_letter || !rest_ok {
            return Err(fail(
                "parts use lowercase letters, digits and `_`, starting with a letter",
            ));
        }
        if BANNED_ABBREVIATIONS.contains(&segment) {
            return Err(fail("uses a banned abbreviation; write the full word"));
        }
    }
    if kind != IdKind::Event && !is_allowed_verb(last) {
        return Err(fail("the verb is not in the allowed set"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_module_dot_verb() {
        assert!(validate_id("world.create", IdKind::Command).is_ok());
        assert!(validate_id("rope.rope_cut", IdKind::Event).is_ok());
    }

    #[test]
    fn rejects_bad_shapes_and_abbreviations() {
        assert!(validate_id("create", IdKind::Command).is_err());
        assert!(validate_id("world.create.now", IdKind::Command).is_err());
        assert!(validate_id("World.create", IdKind::Command).is_err());
        assert!(validate_id("world.pos", IdKind::Command).is_err());
    }
}
