use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value as Json};

use crate::error::WorldError;
use crate::ids::{EntityHandle, PersistentId};
use crate::limits::Limits;
use crate::rng::Rng;
use crate::table::{Table, TableSpec, is_valid_name};
use crate::value::{MAX_SAFE_INT, Value, ValueKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotState {
    Free,
    /// Handle handed out, entity not alive yet (activated at a safe point).
    Reserved,
    Alive,
}

#[derive(Clone, Debug)]
struct Slot {
    generation: u32,
    state: SlotState,
    persistent: Option<PersistentId>,
}

/// Everything about one entity, enough to bring it back with the same persistent id (undo).
#[derive(Clone, Debug, PartialEq)]
pub struct EntitySnapshot {
    pub persistent_id: PersistentId,
    pub tags: Vec<String>,
    pub rows: BTreeMap<String, Vec<Value>>,
}

/// The world: entities, tables, tags, a tick counter and a deterministic RNG.
#[derive(Clone, Debug)]
pub struct World {
    slots: Vec<Slot>,
    free_list: Vec<u32>,
    tables: BTreeMap<String, Table>,
    tags: BTreeMap<String, BTreeSet<EntityHandle>>,
    by_persistent: BTreeMap<PersistentId, EntityHandle>,
    next_persistent_id: u64,
    alive_count: usize,
    tick: u64,
    rng: Rng,
    limits: Limits,
}

impl World {
    /// New empty world. `seed` feeds the deterministic RNG.
    pub fn new(seed: i64) -> Self {
        Self::with_limits(seed, Limits::default())
    }

    pub fn with_limits(seed: i64, limits: Limits) -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            tables: BTreeMap::new(),
            tags: BTreeMap::new(),
            by_persistent: BTreeMap::new(),
            next_persistent_id: 1,
            alive_count: 0,
            tick: 0,
            rng: Rng::new(seed),
            limits,
        }
    }

    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    // ---- tables ---------------------------------------------------------------------------

    pub fn define_table(&mut self, spec: TableSpec) -> Result<(), WorldError> {
        spec.validate(&self.limits)?;
        if self.tables.contains_key(&spec.name) {
            return Err(WorldError::DuplicateTable(spec.name));
        }
        if self.tables.len() >= self.limits.max_tables {
            return Err(WorldError::LimitExceeded("tables"));
        }
        self.tables.insert(
            spec.name.clone(),
            Table {
                spec,
                rows: BTreeMap::new(),
            },
        );
        Ok(())
    }

    pub fn table_spec(&self, table: &str) -> Option<&TableSpec> {
        self.tables.get(table).map(|stored| &stored.spec)
    }

    pub fn table_names(&self) -> Vec<String> {
        self.tables.keys().cloned().collect()
    }

    pub fn column_kind(&self, table: &str, column: &str) -> Result<ValueKind, WorldError> {
        let spec = self
            .table_spec(table)
            .ok_or_else(|| WorldError::UnknownTable(table.into()))?;
        spec.columns
            .iter()
            .find(|candidate| candidate.name == column)
            .map(|found| found.kind)
            .ok_or_else(|| WorldError::UnknownColumn {
                table: table.to_string(),
                column: column.to_string(),
            })
    }

    // ---- entities -------------------------------------------------------------------------

    pub fn entity_count(&self) -> usize {
        self.alive_count
    }

    /// Creates an entity right away with the next persistent id.
    pub fn spawn(&mut self) -> Result<EntityHandle, WorldError> {
        let id = self.take_persistent_id()?;
        let handle = self.allocate_slot(SlotState::Alive)?;
        self.bind_persistent(handle, id);
        Ok(handle)
    }

    /// Creates an entity with a known persistent id (loading a file, or undoing a removal).
    pub fn spawn_with_persistent_id(
        &mut self,
        id: PersistentId,
    ) -> Result<EntityHandle, WorldError> {
        if id.0 == 0 || id.0 > MAX_SAFE_INT as u64 {
            return Err(WorldError::InvalidValue {
                column: "entity".to_string(),
                reason: "o id da entidade precisa estar entre 1 e 2^53-1".to_string(),
            });
        }
        if self.by_persistent.contains_key(&id) {
            return Err(WorldError::DuplicatePersistentId(id.0));
        }
        let handle = self.allocate_slot(SlotState::Alive)?;
        self.bind_persistent(handle, id);
        self.next_persistent_id = self.next_persistent_id.max(id.0 + 1);
        Ok(handle)
    }

    /// Hands out a handle now; the entity becomes alive when [`World::activate`] runs at a
    /// safe point of the frame.
    pub fn reserve(&mut self) -> Result<EntityHandle, WorldError> {
        self.allocate_slot(SlotState::Reserved)
    }

    pub fn activate(&mut self, handle: EntityHandle) -> Result<(), WorldError> {
        let id = self.take_persistent_id()?;
        let slot = self.slot_mut(handle)?;
        if slot.state != SlotState::Reserved {
            return Err(WorldError::StaleHandle);
        }
        slot.state = SlotState::Alive;
        self.alive_count += 1;
        self.bind_persistent_without_count(handle, id);
        Ok(())
    }

    /// Frees a reservation that was never activated.
    pub fn cancel_reservation(&mut self, handle: EntityHandle) -> Result<(), WorldError> {
        let slot = self.slot_mut(handle)?;
        if slot.state != SlotState::Reserved {
            return Err(WorldError::StaleHandle);
        }
        slot.state = SlotState::Free;
        slot.generation = slot.generation.wrapping_add(1);
        self.free_list.push(handle.index());
        Ok(())
    }

    pub fn is_alive(&self, handle: EntityHandle) -> bool {
        self.slot(handle)
            .is_ok_and(|slot| slot.state == SlotState::Alive)
    }

    /// Removes the entity and returns a snapshot that can bring it back.
    pub fn despawn(&mut self, handle: EntityHandle) -> Result<EntitySnapshot, WorldError> {
        let snapshot = self.snapshot(handle)?;
        for table in self.tables.values_mut() {
            table.rows.remove(&handle);
        }
        for members in self.tags.values_mut() {
            members.remove(&handle);
        }
        self.by_persistent.remove(&snapshot.persistent_id);
        let slot = self.slot_mut(handle)?;
        slot.state = SlotState::Free;
        slot.persistent = None;
        slot.generation = slot.generation.wrapping_add(1);
        self.free_list.push(handle.index());
        self.alive_count = self.alive_count.saturating_sub(1);
        Ok(snapshot)
    }

    pub fn snapshot(&self, handle: EntityHandle) -> Result<EntitySnapshot, WorldError> {
        let persistent_id = self.persistent_id(handle)?;
        let rows = self
            .tables
            .iter()
            .filter_map(|(name, table)| {
                table
                    .rows
                    .get(&handle)
                    .map(|row| (name.clone(), row.clone()))
            })
            .collect();
        Ok(EntitySnapshot {
            persistent_id,
            tags: self.tags_of(handle),
            rows,
        })
    }

    pub fn persistent_id(&self, handle: EntityHandle) -> Result<PersistentId, WorldError> {
        let slot = self.slot(handle)?;
        match (slot.state, slot.persistent) {
            (SlotState::Alive, Some(id)) => Ok(id),
            _ => Err(WorldError::StaleHandle),
        }
    }

    pub fn handle_of(&self, id: PersistentId) -> Result<EntityHandle, WorldError> {
        self.by_persistent
            .get(&id)
            .copied()
            .ok_or(WorldError::UnknownPersistentId(id.0))
    }

    /// All living entities, in ascending handle order.
    pub fn all_entities(&self) -> Vec<EntityHandle> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.state == SlotState::Alive)
            .filter_map(|(index, slot)| {
                u32::try_from(index)
                    .ok()
                    .map(|index| EntityHandle::new(index, slot.generation))
            })
            .collect()
    }

    pub fn next_persistent_id(&self) -> u64 {
        self.next_persistent_id
    }

    /// Raises the persistent id counter (never lowers it).
    pub fn raise_next_persistent_id(&mut self, value: u64) {
        self.next_persistent_id = self.next_persistent_id.max(value);
    }

    // ---- tags -----------------------------------------------------------------------------

    pub fn validate_tag(&self, tag: &str) -> Result<(), WorldError> {
        if is_valid_name(tag) {
            Ok(())
        } else {
            Err(WorldError::InvalidName(tag.to_string()))
        }
    }

    pub fn add_tag(&mut self, handle: EntityHandle, tag: &str) -> Result<(), WorldError> {
        self.validate_tag(tag)?;
        self.persistent_id(handle)?;
        let already = self
            .tags
            .get(tag)
            .is_some_and(|members| members.contains(&handle));
        if !already && self.tags_of(handle).len() >= self.limits.max_tags_per_entity {
            return Err(WorldError::LimitExceeded("tags per entity"));
        }
        self.tags.entry(tag.to_string()).or_default().insert(handle);
        Ok(())
    }

    pub fn tags_of(&self, handle: EntityHandle) -> Vec<String> {
        self.tags
            .iter()
            .filter(|(_, members)| members.contains(&handle))
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn entities_with_tag(&self, tag: &str) -> Vec<EntityHandle> {
        self.tags
            .get(tag)
            .map(|members| members.iter().copied().collect())
            .unwrap_or_default()
    }

    // ---- rows and cells -------------------------------------------------------------------

    /// Reads a JSON object as a full row of `table`: unknown columns are rejected, missing
    /// columns take their default. Nothing is changed.
    pub fn parse_row(
        &self,
        table: &str,
        values: &Map<String, Json>,
    ) -> Result<Vec<Value>, WorldError> {
        let spec = self
            .table_spec(table)
            .ok_or_else(|| WorldError::UnknownTable(table.into()))?;
        if let Some(unknown) = values.keys().find(|key| spec.column_index(key).is_none()) {
            return Err(WorldError::UnknownColumn {
                table: table.to_string(),
                column: unknown.clone(),
            });
        }
        spec.columns
            .iter()
            .map(|column| match values.get(&column.name) {
                Some(json) => Value::from_json(column.kind, json, self.limits.max_text_bytes)
                    .map_err(|reason| WorldError::InvalidValue {
                        column: column.name.clone(),
                        reason,
                    }),
                None => Ok(column.default.clone()),
            })
            .collect()
    }

    /// Row as plain JSON (the form saved in scene files).
    pub fn row_to_json(
        &self,
        table: &str,
        values: &[Value],
    ) -> Result<Map<String, Json>, WorldError> {
        let spec = self
            .table_spec(table)
            .ok_or_else(|| WorldError::UnknownTable(table.into()))?;
        Ok(spec
            .columns
            .iter()
            .zip(values)
            .map(|(column, value)| (column.name.clone(), value.to_json()))
            .collect())
    }

    pub fn insert_row_values(
        &mut self,
        handle: EntityHandle,
        table: &str,
        values: Vec<Value>,
    ) -> Result<(), WorldError> {
        self.persistent_id(handle)?;
        let max_text = self.limits.max_text_bytes;
        let stored = self
            .tables
            .get_mut(table)
            .ok_or_else(|| WorldError::UnknownTable(table.into()))?;
        if values.len() != stored.spec.columns.len() {
            return Err(WorldError::InvalidValue {
                column: table.to_string(),
                reason: "a linha tem um número errado de colunas".to_string(),
            });
        }
        for (column, value) in stored.spec.columns.iter().zip(&values) {
            if value.kind() != column.kind {
                return Err(WorldError::KindMismatch {
                    column: column.name.clone(),
                    expected: column.kind,
                    got: value.kind(),
                });
            }
            value
                .validate(max_text)
                .map_err(|reason| WorldError::InvalidValue {
                    column: column.name.clone(),
                    reason,
                })?;
        }
        if stored.rows.contains_key(&handle) {
            return Err(WorldError::RowExists(table.to_string()));
        }
        stored.rows.insert(handle, values);
        Ok(())
    }

    pub fn insert_row_json(
        &mut self,
        handle: EntityHandle,
        table: &str,
        values: &Map<String, Json>,
    ) -> Result<(), WorldError> {
        let parsed = self.parse_row(table, values)?;
        self.insert_row_values(handle, table, parsed)
    }

    pub fn remove_row(
        &mut self,
        handle: EntityHandle,
        table: &str,
    ) -> Result<Vec<Value>, WorldError> {
        self.persistent_id(handle)?;
        let stored = self
            .tables
            .get_mut(table)
            .ok_or_else(|| WorldError::UnknownTable(table.into()))?;
        stored
            .rows
            .remove(&handle)
            .ok_or_else(|| WorldError::RowMissing(table.to_string()))
    }

    pub fn has_row(&self, handle: EntityHandle, table: &str) -> bool {
        self.tables
            .get(table)
            .is_some_and(|stored| stored.rows.contains_key(&handle))
    }

    pub fn row(&self, handle: EntityHandle, table: &str) -> Result<&[Value], WorldError> {
        self.persistent_id(handle)?;
        let stored = self
            .tables
            .get(table)
            .ok_or_else(|| WorldError::UnknownTable(table.into()))?;
        stored
            .rows
            .get(&handle)
            .map(Vec::as_slice)
            .ok_or_else(|| WorldError::RowMissing(table.to_string()))
    }

    pub fn get(
        &self,
        handle: EntityHandle,
        table: &str,
        column: &str,
    ) -> Result<&Value, WorldError> {
        let row = self.row(handle, table)?;
        let spec = self
            .table_spec(table)
            .ok_or_else(|| WorldError::UnknownTable(table.into()))?;
        let position = spec
            .column_index(column)
            .ok_or_else(|| WorldError::UnknownColumn {
                table: table.to_string(),
                column: column.to_string(),
            })?;
        row.get(position)
            .ok_or(WorldError::RowMissing(table.to_string()))
    }

    /// Replaces one cell and returns the old value (so a command can build its inverse).
    pub fn set(
        &mut self,
        handle: EntityHandle,
        table: &str,
        column: &str,
        value: Value,
    ) -> Result<Value, WorldError> {
        self.persistent_id(handle)?;
        let max_text = self.limits.max_text_bytes;
        let stored = self
            .tables
            .get_mut(table)
            .ok_or_else(|| WorldError::UnknownTable(table.into()))?;
        let position =
            stored
                .spec
                .column_index(column)
                .ok_or_else(|| WorldError::UnknownColumn {
                    table: table.to_string(),
                    column: column.to_string(),
                })?;
        let expected = stored.spec.columns.get(position).map(|spec| spec.kind);
        if expected != Some(value.kind()) {
            return Err(WorldError::KindMismatch {
                column: column.to_string(),
                expected: expected.unwrap_or(ValueKind::Bool),
                got: value.kind(),
            });
        }
        value
            .validate(max_text)
            .map_err(|reason| WorldError::InvalidValue {
                column: column.to_string(),
                reason,
            })?;
        let row = stored
            .rows
            .get_mut(&handle)
            .ok_or_else(|| WorldError::RowMissing(table.to_string()))?;
        let cell = row
            .get_mut(position)
            .ok_or(WorldError::RowMissing(table.to_string()))?;
        Ok(std::mem::replace(cell, value))
    }

    /// Entities that have a row in `table`, in ascending handle order.
    pub fn entities_with_table(&self, table: &str) -> Result<Vec<EntityHandle>, WorldError> {
        let stored = self
            .tables
            .get(table)
            .ok_or_else(|| WorldError::UnknownTable(table.into()))?;
        Ok(stored.rows.keys().copied().collect())
    }

    // ---- time and randomness --------------------------------------------------------------

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn advance_tick(&mut self) {
        self.tick += 1;
    }

    pub fn rng_mut(&mut self) -> &mut Rng {
        &mut self.rng
    }

    // ---- internals ------------------------------------------------------------------------

    fn slot(&self, handle: EntityHandle) -> Result<&Slot, WorldError> {
        self.slots
            .get(handle.index() as usize)
            .filter(|slot| slot.generation == handle.generation())
            .ok_or(WorldError::StaleHandle)
    }

    fn slot_mut(&mut self, handle: EntityHandle) -> Result<&mut Slot, WorldError> {
        self.slots
            .get_mut(handle.index() as usize)
            .filter(|slot| slot.generation == handle.generation())
            .ok_or(WorldError::StaleHandle)
    }

    fn take_persistent_id(&mut self) -> Result<PersistentId, WorldError> {
        if self.next_persistent_id > MAX_SAFE_INT as u64 {
            return Err(WorldError::LimitExceeded("persistent ids"));
        }
        let id = PersistentId(self.next_persistent_id);
        self.next_persistent_id += 1;
        Ok(id)
    }

    fn allocate_slot(&mut self, state: SlotState) -> Result<EntityHandle, WorldError> {
        let in_use = self.slots.len() - self.free_list.len();
        if in_use >= self.limits.max_entities {
            return Err(WorldError::LimitExceeded("entities"));
        }
        if let Some(index) = self.free_list.pop() {
            let slot = self
                .slots
                .get_mut(index as usize)
                .ok_or(WorldError::StaleHandle)?;
            slot.state = state;
            return Ok(EntityHandle::new(index, slot.generation));
        }
        let index = u32::try_from(self.slots.len())
            .map_err(|_| WorldError::LimitExceeded("entity slots"))?;
        self.slots.push(Slot {
            generation: 0,
            state,
            persistent: None,
        });
        Ok(EntityHandle::new(index, 0))
    }

    fn bind_persistent(&mut self, handle: EntityHandle, id: PersistentId) {
        self.alive_count += 1;
        self.bind_persistent_without_count(handle, id);
    }

    fn bind_persistent_without_count(&mut self, handle: EntityHandle, id: PersistentId) {
        if let Ok(slot) = self.slot_mut(handle) {
            slot.persistent = Some(id);
        }
        self.by_persistent.insert(id, handle);
    }
}

#[cfg(test)]
mod tests {
    use petunia_math::Vec3;

    use super::*;

    fn world_with_transform() -> World {
        let mut world = World::new(1);
        world
            .define_table(TableSpec::new("Transform").column(
                "position",
                ValueKind::Vec3,
                Value::Vec3(Vec3::ZERO),
            ))
            .unwrap();
        world
    }

    #[test]
    fn stale_handle_never_points_to_another_entity() {
        let mut world = world_with_transform();
        let first = world.spawn().unwrap();
        world.despawn(first).unwrap();
        let second = world.spawn().unwrap();
        assert_eq!(first.index(), second.index(), "slot is reused");
        assert!(!world.is_alive(first), "old handle is stale");
        assert!(world.is_alive(second));
        assert_eq!(world.persistent_id(first), Err(WorldError::StaleHandle));
    }

    #[test]
    fn despawn_snapshot_restores_the_same_persistent_id() {
        let mut world = world_with_transform();
        let handle = world.spawn().unwrap();
        world.add_tag(handle, "Crate").unwrap();
        world
            .insert_row_values(
                handle,
                "Transform",
                vec![Value::Vec3(Vec3::new(1.0, 2.0, 3.0))],
            )
            .unwrap();
        let snapshot = world.despawn(handle).unwrap();
        let restored = world
            .spawn_with_persistent_id(snapshot.persistent_id)
            .unwrap();
        assert_eq!(
            world.persistent_id(restored).unwrap(),
            snapshot.persistent_id
        );
    }

    #[test]
    fn set_rejects_wrong_kind_and_non_finite_values() {
        let mut world = world_with_transform();
        let handle = world.spawn().unwrap();
        world
            .insert_row_values(handle, "Transform", vec![Value::Vec3(Vec3::ZERO)])
            .unwrap();
        assert!(matches!(
            world.set(handle, "Transform", "position", Value::Float(1.0)),
            Err(WorldError::KindMismatch { .. })
        ));
        assert!(matches!(
            world.set(
                handle,
                "Transform",
                "position",
                Value::Vec3(Vec3::new(f32::NAN, 0.0, 0.0))
            ),
            Err(WorldError::InvalidValue { .. })
        ));
    }

    #[test]
    fn iteration_is_ordered_by_handle() {
        let mut world = world_with_transform();
        let handles: Vec<_> = (0..4).map(|_| world.spawn().unwrap()).collect();
        for handle in handles.iter().rev() {
            world
                .insert_row_values(*handle, "Transform", vec![Value::Vec3(Vec3::ZERO)])
                .unwrap();
        }
        assert_eq!(world.entities_with_table("Transform").unwrap(), handles);
    }
}
