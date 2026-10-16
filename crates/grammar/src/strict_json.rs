use std::cell::RefCell;
use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, Error, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use crate::response_error::ResponseError;

pub(crate) fn parse_strict(text: &str) -> Result<Value, ResponseError> {
    let duplicate = RefCell::new(None);
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let parsed = StrictSeed { duplicate: &duplicate }
        .deserialize(&mut deserializer)
        .and_then(|value| deserializer.end().map(|()| value));
    match (parsed, duplicate.into_inner()) {
        (_, Some(key)) => Err(ResponseError::DuplicateKey(key)),
        (Ok(value), None) => Ok(value),
        (Err(error), None) => Err(ResponseError::Json(error.to_string())),
    }
}

#[derive(Clone, Copy)]
struct StrictSeed<'cell> {
    duplicate: &'cell RefCell<Option<String>>,
}

impl<'de> DeserializeSeed<'de> for StrictSeed<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for StrictSeed<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without repeated keys")
    }

    fn visit_bool<E: Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("a JSON number is finite"))
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E: Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut elements = Vec::new();
        while let Some(element) = sequence.next_element_seed(self)? {
            elements.push(element);
        }
        Ok(Value::Array(elements))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                let message = format!("repeated key {key}");
                *self.duplicate.borrow_mut() = Some(key);
                return Err(A::Error::custom(message));
            }
            let value = map.next_value_seed(self)?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}
