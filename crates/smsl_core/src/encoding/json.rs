use std::{cell::Cell, fmt};

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};

use super::{MAX_VALUE_DEPTH, MAX_VALUE_NODES};
use crate::Diagnostic;

pub(super) fn decode(source: &str) -> Result<Value, Diagnostic> {
    crate::limits::input(source, false)?;
    let mut decoder = serde_json::Deserializer::from_str(source);
    let count = Cell::new(0);
    let limited = Cell::new(false);
    let value = Seed {
        depth: 1,
        count: &count,
        limited: &limited,
    }
    .deserialize(&mut decoder)
    .map_err(|error| {
        if limited.get() {
            super::resource("Value tree exceeds resource limits")
        } else {
            Diagnostic::new(
                "syntax",
                error.to_string(),
                Some(crate::diagnostic::json_location(source, &error)),
            )
        }
    })?;
    decoder.end().map_err(|error| {
        Diagnostic::new(
            "syntax",
            error.to_string(),
            Some(crate::diagnostic::json_location(source, &error)),
        )
    })?;
    Ok(value)
}

struct Seed<'a> {
    depth: usize,
    count: &'a Cell<usize>,
    limited: &'a Cell<bool>,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        self.count.set(self.count.get() + 1);
        if self.depth > MAX_VALUE_DEPTH || self.count.get() > MAX_VALUE_NODES {
            self.limited.set(true);
            return Err(de::Error::custom("resource_limit"));
        }
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a strict SMSL value")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(Seed {
            depth: self.depth + 1,
            count: self.count,
            limited: self.limited,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("Duplicate key {key:?}")));
            }
            let value = map.next_value_seed(Seed {
                depth: self.depth + 1,
                count: self.count,
                limited: self.limited,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
