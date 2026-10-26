use std::collections::{BTreeMap, VecDeque};

use petunia_schema::{CommandSpec, IdKind, Reversibility, Schema, check_params, validate_id};
use petunia_world::World;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::builtin;
use crate::error::CommandError;

/// History is bounded: old entries fall off the back.
pub const MAX_HISTORY: usize = 500;

/// History is also bounded by memory: an undo step of an editable mesh keeps the whole old mesh,
/// so a few hundred edits of a large mesh would otherwise hold gigabytes. The newest step always
/// stays, whatever its size.
pub const MAX_HISTORY_BYTES: usize = 256 * 1024 * 1024;

/// Most commands in one `world.batch` (a hostile caller cannot ask for endless work).
pub const MAX_BATCH: usize = 10_000;

/// Name of the built-in batch command: several records applied as one, undone as one.
pub const BATCH_COMMAND: &str = "world.batch";

/// A command as data. This is what travels over the network, MCP and macros.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandRecord {
    pub command: String,
    #[serde(default)]
    pub payload: Json,
}

impl CommandRecord {
    pub fn new(command: &str, payload: Json) -> Self {
        Self {
            command: command.to_string(),
            payload,
        }
    }
}

/// What a command function returns: a result for the caller and, when reversible, the record
/// that undoes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub result: Json,
    pub inverse: Option<CommandRecord>,
}

/// A registered command function. It must validate everything before it changes the world so a
/// failure leaves the world untouched.
pub type ApplyFn = fn(&mut World, &Json) -> Result<Outcome, CommandError>;

struct Entry {
    spec: CommandSpec,
    apply: ApplyFn,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    command: String,
    inverse: CommandRecord,
    /// Size of the inverse as JSON: what the step costs to keep.
    bytes: usize,
}

impl HistoryEntry {
    fn new(command: String, inverse: CommandRecord) -> Self {
        let bytes = json_bytes(&inverse.payload) + inverse.command.len();
        Self {
            command,
            inverse,
            bytes,
        }
    }
}

/// Bytes of a JSON value written compactly, counted without building the text.
fn json_bytes(value: &Json) -> usize {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    match serde_json::to_writer(&mut counter, value) {
        Ok(()) => counter.0,
        Err(_) => 0,
    }
}

/// Table of commands plus undo and redo stacks.
pub struct Dispatcher {
    entries: BTreeMap<String, Entry>,
    undo: VecDeque<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    history_budget_bytes: usize,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            undo: VecDeque::new(),
            redo: Vec::new(),
            history_budget_bytes: MAX_HISTORY_BYTES,
        }
    }

    /// Dispatcher with the engine's built-in commands registered.
    pub fn with_builtin() -> Result<Self, CommandError> {
        let mut dispatcher = Self::new();
        for (spec, apply) in builtin::commands() {
            dispatcher.register(spec, apply)?;
        }
        dispatcher.register(builtin::batch_spec(), refuse_nested_batch)?;
        Ok(dispatcher)
    }

    pub fn register(&mut self, spec: CommandSpec, apply: ApplyFn) -> Result<(), CommandError> {
        validate_id(&spec.id, IdKind::Command)?;
        if self.entries.contains_key(&spec.id) {
            return Err(CommandError::Registration(format!(
                "`{}` is already registered",
                spec.id
            )));
        }
        self.entries.insert(spec.id.clone(), Entry { spec, apply });
        Ok(())
    }

    /// The API of this dispatcher as a schema document.
    pub fn schema(&self) -> Schema {
        let mut schema = Schema::new();
        schema.commands = self
            .entries
            .values()
            .map(|entry| entry.spec.clone())
            .collect();
        schema.queries = builtin::query_specs();
        schema
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Validates and runs one command. On success the inverse (if any) joins the undo stack and
    /// the redo stack is cleared.
    pub fn execute(
        &mut self,
        world: &mut World,
        record: &CommandRecord,
    ) -> Result<Json, CommandError> {
        let outcome = self.run(world, record)?;
        let reversible = self
            .entries
            .get(&record.command)
            .is_some_and(|entry| entry.spec.reversibility == Reversibility::Undoable);
        if reversible && let Some(inverse) = outcome.inverse {
            self.push_undo(HistoryEntry::new(record.command.clone(), inverse));
            self.redo.clear();
        }
        Ok(outcome.result)
    }

    /// Undoes the last command. Returns its name.
    pub fn undo(&mut self, world: &mut World) -> Result<String, CommandError> {
        let entry = self.undo.pop_back().ok_or(CommandError::NothingToUndo)?;
        let outcome = self.run(world, &entry.inverse)?;
        if let Some(inverse) = outcome.inverse {
            self.redo
                .push(HistoryEntry::new(entry.command.clone(), inverse));
        }
        Ok(entry.command)
    }

    /// Redoes the last undone command. Returns its name.
    pub fn redo(&mut self, world: &mut World) -> Result<String, CommandError> {
        let entry = self.redo.pop().ok_or(CommandError::NothingToRedo)?;
        let outcome = self.run(world, &entry.inverse)?;
        if let Some(inverse) = outcome.inverse {
            self.push_undo(HistoryEntry::new(entry.command.clone(), inverse));
        }
        Ok(entry.command)
    }

    fn run(&self, world: &mut World, record: &CommandRecord) -> Result<Outcome, CommandError> {
        if record.command == BATCH_COMMAND {
            return self.run_batch(world, record);
        }
        let entry =
            self.entries
                .get(&record.command)
                .ok_or_else(|| CommandError::UnknownCommand {
                    name: record.command.clone(),
                    suggestions: self.suggest(&record.command),
                })?;
        check_params(&entry.spec.params, &record.payload).map_err(|message| {
            CommandError::InvalidPayload {
                command: record.command.clone(),
                message,
            }
        })?;
        (entry.apply)(world, &record.payload)
    }

    /// Applies every record in order. If one fails, the ones already applied are undone in
    /// reverse, so the world is left exactly as it was. The inverse is one batch of the inverses,
    /// in reverse order: undo and redo treat the whole batch as one step.
    fn run_batch(
        &self,
        world: &mut World,
        record: &CommandRecord,
    ) -> Result<Outcome, CommandError> {
        let invalid = |message: &str| CommandError::InvalidPayload {
            command: BATCH_COMMAND.to_string(),
            message: message.to_string(),
        };
        let records: Vec<CommandRecord> = record
            .payload
            .get("records")
            .cloned()
            .map(serde_json::from_value)
            .ok_or_else(|| invalid("falta a lista `records`"))?
            .map_err(|error| invalid(&format!("lista `records` inválida: {error}")))?;
        if records.len() > MAX_BATCH {
            return Err(invalid(&format!("no máximo {MAX_BATCH} comandos por lote")));
        }
        let mut inverses: Vec<CommandRecord> = Vec::with_capacity(records.len());
        let mut results = Vec::with_capacity(records.len());
        for original in &records {
            if original.command == BATCH_COMMAND {
                self.roll_back(world, &inverses);
                return Err(invalid("um lote não pode conter outro lote"));
            }
            let inner = match resolve_references(&original.payload, &results) {
                Ok(payload) => CommandRecord::new(&original.command, payload),
                Err(message) => {
                    self.roll_back(world, &inverses);
                    return Err(invalid(&message));
                }
            };
            match self.run(world, &inner) {
                Ok(outcome) => {
                    results.push(outcome.result);
                    if let Some(inverse) = outcome.inverse {
                        inverses.push(inverse);
                    }
                }
                Err(error) => {
                    self.roll_back(world, &inverses);
                    return Err(error);
                }
            }
        }
        inverses.reverse();
        Ok(Outcome {
            result: Json::Array(results),
            inverse: Some(CommandRecord::new(
                BATCH_COMMAND,
                serde_json::json!({ "records": inverses }),
            )),
        })
    }

    fn roll_back(&self, world: &mut World, inverses: &[CommandRecord]) {
        for inverse in inverses.iter().rev() {
            let _ = self.run(world, inverse);
        }
    }

    fn push_undo(&mut self, entry: HistoryEntry) {
        self.undo.push_back(entry);
        while self.undo.len() > MAX_HISTORY {
            self.undo.pop_front();
        }
        let mut total: usize = self.undo.iter().map(|entry| entry.bytes).sum();
        while total > self.history_budget_bytes && self.undo.len() > 1 {
            if let Some(oldest) = self.undo.pop_front() {
                total -= oldest.bytes;
            }
        }
    }

    /// Memory the undo history may hold (bytes of JSON; default [`MAX_HISTORY_BYTES`]). Lower it
    /// on machines with little memory; the newest step always stays.
    pub fn set_history_budget(&mut self, bytes: usize) {
        self.history_budget_bytes = bytes.min(MAX_HISTORY_BYTES);
    }

    /// Steps that can be undone, and the memory they hold (bytes of JSON).
    pub fn undo_depth(&self) -> (usize, usize) {
        (
            self.undo.len(),
            self.undo.iter().map(|entry| entry.bytes).sum(),
        )
    }

    /// Commands in the same module, or sharing the verb, as a hint for typos.
    fn suggest(&self, name: &str) -> Vec<String> {
        let (module, verb) = name.split_once('.').unwrap_or((name, ""));
        self.entries
            .keys()
            .filter(|known| {
                known
                    .split_once('.')
                    .is_some_and(|(known_module, known_verb)| {
                        known_module == module || (!verb.is_empty() && known_verb == verb)
                    })
            })
            .take(3)
            .cloned()
            .collect()
    }
}

/// The batch is run by the dispatcher itself; this entry only exists so the batch has a spec in
/// the schema. It is never called.
fn refuse_nested_batch(_: &mut World, _: &Json) -> Result<Outcome, CommandError> {
    Err(CommandError::InvalidPayload {
        command: BATCH_COMMAND.to_string(),
        message: "o lote é executado pelo despachante".to_string(),
    })
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Inside a batch, `{"$entity_of": n}` stands for the entity created by command `n` of the same
/// batch (counting from zero, earlier commands only). This lets one batch create an object and
/// the things that point to it, as one undo step. The inverse records hold real ids, so undo and
/// redo never see the references.
fn resolve_references(payload: &Json, results: &[Json]) -> Result<Json, String> {
    match payload {
        Json::Object(fields) => {
            if let (1, Some(reference)) = (fields.len(), fields.get("$entity_of")) {
                let index = reference
                    .as_u64()
                    .ok_or("`$entity_of` precisa de um número")?;
                return results
                    .get(index as usize)
                    .and_then(|result| result.get("entity"))
                    .cloned()
                    .ok_or_else(|| {
                        format!("`$entity_of: {index}` não aponta para uma entidade criada antes no lote")
                    });
            }
            let mut resolved = serde_json::Map::new();
            for (key, value) in fields {
                resolved.insert(key.clone(), resolve_references(value, results)?);
            }
            Ok(Json::Object(resolved))
        }
        Json::Array(values) => values
            .iter()
            .map(|value| resolve_references(value, results))
            .collect::<Result<Vec<_>, _>>()
            .map(Json::Array),
        other => Ok(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_world::{TableSpec, Value, ValueKind};
    use serde_json::json;

    #[test]
    fn history_drops_the_oldest_steps_when_it_passes_its_memory_budget() {
        let mut world = World::new(1);
        world
            .define_table(TableSpec::new("Note").column(
                "text",
                ValueKind::Text,
                Value::Text(String::new()),
            ))
            .expect("table");
        let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
        let created = dispatcher
            .execute(
                &mut world,
                &CommandRecord::new("world.create", json!({ "rows": { "Note": {} } })),
            )
            .expect("create");
        let entity = created["entity"].as_u64().expect("entity");
        let text = "x".repeat(1000);
        for _ in 0..20 {
            dispatcher
                .execute(
                    &mut world,
                    &CommandRecord::new(
                        "world.set",
                        json!({ "entity": entity, "table": "Note", "column": "text", "value": text }),
                    ),
                )
                .expect("set");
        }
        let (steps, bytes) = dispatcher.undo_depth();
        assert_eq!(steps, 21);
        assert!(bytes > 19 * 1000 && bytes < 21 * 1100, "{bytes}");
        assert_eq!(json_bytes(&json!({ "a": [1, 2] })), r#"{"a":[1,2]}"#.len());

        // A budget of about five steps keeps only the newest five, and undo still works.
        dispatcher.set_history_budget(5 * 1100);
        dispatcher
            .execute(
                &mut world,
                &CommandRecord::new(
                    "world.set",
                    json!({ "entity": entity, "table": "Note", "column": "text", "value": "end" }),
                ),
            )
            .expect("set");
        let (steps, bytes) = dispatcher.undo_depth();
        assert!(steps <= 5 && bytes <= 5 * 1100, "{steps} {bytes}");
        dispatcher.undo(&mut world).expect("undo");
        let handle = world
            .handle_of(petunia_world::PersistentId(entity))
            .expect("handle");
        assert_eq!(world.get(handle, "Note", "text"), Ok(&Value::Text(text)));
        // Even a tiny budget keeps the newest step.
        dispatcher.set_history_budget(1);
        dispatcher.redo(&mut world).expect("redo");
        assert_eq!(dispatcher.undo_depth().0, 1);
    }
}
