use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct BlackboardKeyId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum BlackboardValueType {
    Bool,
    Int,
    Float,
    Entity,
    Vec2,
    Vec3,
    Quat,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum BlackboardKeyDirection {
    Input,
    Output,
    InOut,
    Local,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub enum BlackboardValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    Entity(Entity),
    Vec2(Vec2),
    Vec3(Vec3),
    Quat(Quat),
    Text(String),
}

impl BlackboardValue {
    pub fn value_type(&self) -> BlackboardValueType {
        match self {
            Self::Bool(_) => BlackboardValueType::Bool,
            Self::Int(_) => BlackboardValueType::Int,
            Self::Float(_) => BlackboardValueType::Float,
            Self::Entity(_) => BlackboardValueType::Entity,
            Self::Vec2(_) => BlackboardValueType::Vec2,
            Self::Vec3(_) => BlackboardValueType::Vec3,
            Self::Quat(_) => BlackboardValueType::Quat,
            Self::Text(_) => BlackboardValueType::Text,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i32> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f32> {
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_entity(&self) -> Option<Entity> {
        match self {
            Self::Entity(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_vec2(&self) -> Option<Vec2> {
        match self {
            Self::Vec2(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_vec3(&self) -> Option<Vec3> {
        match self {
            Self::Vec3(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_quat(&self) -> Option<Quat> {
        match self {
            Self::Quat(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value.as_str()),
            _ => None,
        }
    }
}

impl From<bool> for BlackboardValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i32> for BlackboardValue {
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}

impl From<f32> for BlackboardValue {
    fn from(value: f32) -> Self {
        Self::Float(value)
    }
}

impl From<Entity> for BlackboardValue {
    fn from(value: Entity) -> Self {
        Self::Entity(value)
    }
}

impl From<Vec2> for BlackboardValue {
    fn from(value: Vec2) -> Self {
        Self::Vec2(value)
    }
}

impl From<Vec3> for BlackboardValue {
    fn from(value: Vec3) -> Self {
        Self::Vec3(value)
    }
}

impl From<Quat> for BlackboardValue {
    fn from(value: Quat) -> Self {
        Self::Quat(value)
    }
}

impl From<String> for BlackboardValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for BlackboardValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub enum BlackboardCondition {
    Exists,
    Missing,
    IsTrue,
    IsFalse,
    Equals(BlackboardValue),
    NotEquals(BlackboardValue),
    FloatGreaterThan(f32),
    FloatLessThan(f32),
    IntGreaterThan(i32),
    IntLessThan(i32),
}

impl BlackboardCondition {
    pub fn evaluate(&self, value: Option<&BlackboardValue>) -> bool {
        match self {
            Self::Exists => value.is_some(),
            Self::Missing => value.is_none(),
            Self::IsTrue => value.and_then(BlackboardValue::as_bool).unwrap_or(false),
            Self::IsFalse => !value.and_then(BlackboardValue::as_bool).unwrap_or(false),
            Self::Equals(expected) => value == Some(expected),
            Self::NotEquals(expected) => value != Some(expected),
            Self::FloatGreaterThan(expected) => value
                .and_then(BlackboardValue::as_float)
                .is_some_and(|v| v > *expected),
            Self::FloatLessThan(expected) => value
                .and_then(BlackboardValue::as_float)
                .is_some_and(|v| v < *expected),
            Self::IntGreaterThan(expected) => value
                .and_then(BlackboardValue::as_int)
                .is_some_and(|v| v > *expected),
            Self::IntLessThan(expected) => value
                .and_then(BlackboardValue::as_int)
                .is_some_and(|v| v < *expected),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct BlackboardKeyDefinition {
    pub id: BlackboardKeyId,
    pub name: String,
    pub value_type: BlackboardValueType,
    pub direction: BlackboardKeyDirection,
    pub required: bool,
    pub default_value: Option<BlackboardValue>,
    pub description: String,
}

#[derive(Clone, Debug, Default, PartialEq, Reflect)]
pub struct BlackboardSchema {
    pub keys: Vec<BlackboardKeyDefinition>,
}

impl BlackboardSchema {
    pub fn key(&self, key: BlackboardKeyId) -> Option<&BlackboardKeyDefinition> {
        self.keys.get(key.0 as usize)
    }

    pub fn find_key(&self, name: &str) -> Option<BlackboardKeyId> {
        self.keys
            .iter()
            .find(|key| key.name == name)
            .map(|key| key.id)
    }
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct BlackboardChange {
    pub key: BlackboardKeyId,
    pub name: String,
    pub revision: u64,
    pub old_value: Option<BlackboardValue>,
    pub new_value: Option<BlackboardValue>,
}

#[derive(Component, Clone, Debug, Default, PartialEq, Reflect)]
pub struct BehaviorTreeBlackboard {
    pub schema: BlackboardSchema,
    pub values: Vec<Option<BlackboardValue>>,
    pub revisions: Vec<u64>,
    pub total_revision: u64,
    pub dirty_keys: Vec<BlackboardKeyId>,
    pub recent_changes: Vec<BlackboardChange>,
}

impl BehaviorTreeBlackboard {
    pub fn from_schema(schema: &BlackboardSchema) -> Self {
        let mut values = Vec::with_capacity(schema.keys.len());
        let revisions = vec![0; schema.keys.len()];
        for key in &schema.keys {
            values.push(key.default_value.clone());
        }
        Self {
            schema: schema.clone(),
            values,
            revisions,
            total_revision: 0,
            dirty_keys: Vec::new(),
            recent_changes: Vec::new(),
        }
    }

    pub fn resize_to_schema(&mut self, schema: &BlackboardSchema, preserve_values: bool) {
        let mut next = Self::from_schema(schema);
        if preserve_values {
            for key in &schema.keys {
                if let Some(previous_key) = self.schema.find_key(&key.name)
                    && let Some(previous_value) =
                        self.values.get(previous_key.0 as usize).cloned().flatten()
                    && previous_value.value_type() == key.value_type
                {
                    next.values[key.id.0 as usize] = Some(previous_value);
                }
            }
        }
        *self = next;
    }

    pub fn value(&self, key: BlackboardKeyId) -> Option<&BlackboardValue> {
        self.values.get(key.0 as usize).and_then(Option::as_ref)
    }

    pub fn set(
        &mut self,
        key: BlackboardKeyId,
        value: impl Into<BlackboardValue>,
    ) -> Result<bool, String> {
        let Some(definition) = self.schema.key(key) else {
            return Err(format!("unknown blackboard key {:?}", key));
        };
        let value = value.into();
        if definition.value_type != value.value_type() {
            return Err(format!(
                "blackboard key '{}' expects {:?}, got {:?}",
                definition.name,
                definition.value_type,
                value.value_type()
            ));
        }
        let index = key.0 as usize;
        if self
            .values
            .get(index)
            .is_some_and(|current| current.as_ref() == Some(&value))
        {
            return Ok(false);
        }
        let old_value = self.values[index].clone();
        self.values[index] = Some(value.clone());
        self.total_revision += 1;
        self.revisions[index] = self.total_revision;
        if !self.dirty_keys.contains(&key) {
            self.dirty_keys.push(key);
        }
        self.recent_changes.push(BlackboardChange {
            key,
            name: definition.name.clone(),
            revision: self.total_revision,
            old_value,
            new_value: Some(value),
        });
        Ok(true)
    }

    pub fn clear(&mut self, key: BlackboardKeyId) -> Result<bool, String> {
        let Some(definition) = self.schema.key(key) else {
            return Err(format!("unknown blackboard key {:?}", key));
        };
        let index = key.0 as usize;
        if self.values[index].is_none() {
            return Ok(false);
        }
        let old_value = self.values[index].take();
        self.total_revision += 1;
        self.revisions[index] = self.total_revision;
        if !self.dirty_keys.contains(&key) {
            self.dirty_keys.push(key);
        }
        self.recent_changes.push(BlackboardChange {
            key,
            name: definition.name.clone(),
            revision: self.total_revision,
            old_value,
            new_value: None,
        });
        Ok(true)
    }

    pub fn take_dirty_keys(&mut self) -> Vec<BlackboardKeyId> {
        std::mem::take(&mut self.dirty_keys)
    }

    pub fn take_recent_changes(&mut self) -> Vec<BlackboardChange> {
        std::mem::take(&mut self.recent_changes)
    }

    pub fn get_bool(&self, key: BlackboardKeyId) -> Option<bool> {
        self.value(key).and_then(BlackboardValue::as_bool)
    }

    pub fn get_int(&self, key: BlackboardKeyId) -> Option<i32> {
        self.value(key).and_then(BlackboardValue::as_int)
    }

    pub fn get_float(&self, key: BlackboardKeyId) -> Option<f32> {
        self.value(key).and_then(BlackboardValue::as_float)
    }

    pub fn get_entity(&self, key: BlackboardKeyId) -> Option<Entity> {
        self.value(key).and_then(BlackboardValue::as_entity)
    }

    pub fn get_vec2(&self, key: BlackboardKeyId) -> Option<Vec2> {
        self.value(key).and_then(BlackboardValue::as_vec2)
    }

    pub fn get_vec3(&self, key: BlackboardKeyId) -> Option<Vec3> {
        self.value(key).and_then(BlackboardValue::as_vec3)
    }

    pub fn get_quat(&self, key: BlackboardKeyId) -> Option<Quat> {
        self.value(key).and_then(BlackboardValue::as_quat)
    }

    pub fn get_text(&self, key: BlackboardKeyId) -> Option<&str> {
        self.value(key).and_then(BlackboardValue::as_text)
    }
}

#[cfg(test)]
#[path = "blackboard_tests.rs"]
mod tests;
