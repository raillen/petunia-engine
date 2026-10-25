//! Input by named actions.
//!
//! Game logic asks `held("move_forward")`, never "is key 87 down". Bindings are plain data, so
//! a player can remap them and every action can have several keys (for example arrows and
//! WASD), which matters for accessibility. The platform adapter turns hardware keys into the
//! string ids used here (`"KeyW"`, `"Space"`, ...).

use std::collections::{BTreeMap, BTreeSet};

/// Action names used by the fly camera.
pub mod actions {
    pub const MOVE_FORWARD: &str = "move_forward";
    pub const MOVE_BACKWARD: &str = "move_backward";
    pub const MOVE_LEFT: &str = "move_left";
    pub const MOVE_RIGHT: &str = "move_right";
    pub const MOVE_UP: &str = "move_up";
    pub const MOVE_DOWN: &str = "move_down";
    pub const BOOST: &str = "boost";
    pub const RELEASE_CURSOR: &str = "release_cursor";
}

/// Which keys trigger which actions. Several keys per action are allowed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActionMap {
    bindings: BTreeMap<String, BTreeSet<String>>,
}

impl ActionMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Default bindings for the fly camera: WASD and arrows to move, Space/E up, Q/Ctrl down,
    /// Shift to go faster, Escape to release the mouse.
    pub fn fly_defaults() -> Self {
        let mut map = Self::new();
        for (action, keys) in [
            (actions::MOVE_FORWARD, ["KeyW", "ArrowUp"].as_slice()),
            (actions::MOVE_BACKWARD, ["KeyS", "ArrowDown"].as_slice()),
            (actions::MOVE_LEFT, ["KeyA", "ArrowLeft"].as_slice()),
            (actions::MOVE_RIGHT, ["KeyD", "ArrowRight"].as_slice()),
            (actions::MOVE_UP, ["Space", "KeyE"].as_slice()),
            (actions::MOVE_DOWN, ["KeyQ", "ControlLeft"].as_slice()),
            (actions::BOOST, ["ShiftLeft", "ShiftRight"].as_slice()),
            (actions::RELEASE_CURSOR, ["Escape"].as_slice()),
        ] {
            for key in keys {
                map.bind(action, key);
            }
        }
        map
    }

    pub fn bind(&mut self, action: &str, key: &str) {
        self.bindings
            .entry(action.to_string())
            .or_default()
            .insert(key.to_string());
    }

    pub fn unbind(&mut self, action: &str, key: &str) {
        if let Some(keys) = self.bindings.get_mut(action) {
            keys.remove(key);
        }
    }

    pub fn keys_for(&self, action: &str) -> Vec<&str> {
        self.bindings
            .get(action)
            .map(|keys| keys.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }
}

/// Which keys are down right now and which went down this frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActionState {
    held_keys: BTreeSet<String>,
    pressed_keys: BTreeSet<String>,
}

impl ActionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Call at the start of every frame, before feeding new events.
    pub fn begin_frame(&mut self) {
        self.pressed_keys.clear();
    }

    pub fn key_down(&mut self, key: &str) {
        if self.held_keys.insert(key.to_string()) {
            self.pressed_keys.insert(key.to_string());
        }
    }

    pub fn key_up(&mut self, key: &str) {
        self.held_keys.remove(key);
    }

    /// Forgets everything held (window lost focus), so no key stays stuck.
    pub fn release_all(&mut self) {
        self.held_keys.clear();
    }

    pub fn held(&self, map: &ActionMap, action: &str) -> bool {
        map.keys_for(action)
            .iter()
            .any(|key| self.held_keys.contains(*key))
    }

    pub fn pressed(&self, map: &ActionMap, action: &str) -> bool {
        map.keys_for(action)
            .iter()
            .any(|key| self.pressed_keys.contains(*key))
    }

    /// `+1` when `positive` is held, `-1` when `negative` is held, `0` for both or neither.
    pub fn axis(&self, map: &ActionMap, negative: &str, positive: &str) -> f32 {
        let plus = if self.held(map, positive) { 1.0 } else { 0.0 };
        let minus = if self.held(map, negative) { 1.0 } else { 0.0 };
        plus - minus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_keys_can_trigger_the_same_action() {
        let map = ActionMap::fly_defaults();
        let mut state = ActionState::new();
        state.key_down("ArrowUp");
        assert!(state.held(&map, actions::MOVE_FORWARD));
        state.key_up("ArrowUp");
        state.key_down("KeyW");
        assert!(state.held(&map, actions::MOVE_FORWARD));
    }

    #[test]
    fn pressed_lasts_one_frame_and_axis_cancels() {
        let map = ActionMap::fly_defaults();
        let mut state = ActionState::new();
        state.key_down("KeyD");
        assert!(state.pressed(&map, actions::MOVE_RIGHT));
        state.begin_frame();
        assert!(!state.pressed(&map, actions::MOVE_RIGHT));
        assert!(state.held(&map, actions::MOVE_RIGHT));
        state.key_down("KeyA");
        assert_eq!(
            state.axis(&map, actions::MOVE_LEFT, actions::MOVE_RIGHT),
            0.0
        );
    }

    #[test]
    fn remapping_replaces_default_behavior() {
        let mut map = ActionMap::fly_defaults();
        map.bind(actions::MOVE_FORWARD, "KeyI");
        let mut state = ActionState::new();
        state.key_down("KeyI");
        assert!(state.held(&map, actions::MOVE_FORWARD));
    }
}
