use std::{sync::Arc, fmt, collections::BTreeMap, cmp::Ordering, hash::Hash };
use serde::{de::{Visitor, MapAccess}, Deserialize, Deserializer, Serialize, ser::{SerializeSeq, SerializeMap}};
use super::{StructConst, eoestructdata::{DataStackTransformer, eoestack_run}, StructBuilt, eoestruct::{LateValues}, StructError };
use serde_json::{Value as JsonValue, Number};

#[cfg_attr(debug_assertions,derive(Debug))]
#[derive(Clone)]
pub enum StructValue {
    Const(StructConst),
    Array(Arc<Vec<StructValue>>),
    Object(Arc<BTreeMap<String,StructValue>>)
}

impl PartialOrd for StructValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.arm_order().cmp(&other.arm_order()) {
            Ordering::Equal => {
                match (self,other) {
                    (StructValue::Const(a), StructValue::Const(b)) => a.partial_cmp(b),
                    (StructValue::Array(a), StructValue::Array(b)) => a.partial_cmp(b),
                    (StructValue::Object(a), StructValue::Object(b)) => a.partial_cmp(b),
                    _ => panic!("impossible")
                }
            },
            x => Some(x)
        }        
    }
}

impl Ord for StructValue {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

struct ValueTransformer;

impl DataStackTransformer<StructConst,StructValue> for ValueTransformer {
    fn make_singleton(&mut self, value: StructConst) -> StructValue {
        StructValue::Const(value)
    }

    fn make_array(&mut self, value: Vec<StructValue>) -> StructValue {
        StructValue::Array(Arc::new(value))
    }

    fn make_object(&mut self, value: Vec<(String,StructValue)>) -> StructValue {
        StructValue::Object(Arc::new(value.iter().map(|x| x.clone()).collect()))
    }
}

impl StructValue {
    pub fn new_number(input: f64) -> StructValue {
        Self::Const(StructConst::Number(input))
    }

    pub fn new_string(input: String) -> StructValue {
        Self::Const(StructConst::String(input))
    }

    pub fn new_boolean(input: bool) -> StructValue {
        Self::Const(StructConst::Boolean(input))
    }

    pub fn new_null() -> StructValue {
        Self::Const(StructConst::Null)
    }

    pub fn new_array(input: Vec<StructValue>) -> StructValue {
        Self::Array(Arc::new(input))
    }

    pub fn new_object(mut input: Vec<(String,StructValue)>) -> StructValue {
        Self::Object(Arc::new(input.drain(..).collect()))
    }

    pub fn new_expand(input: &StructBuilt, lates: Option<&LateValues>) -> Result<StructValue,StructError> {
        eoestack_run(input,lates,ValueTransformer)
    }

    pub fn new_json_value(value: &JsonValue) -> StructValue {
        match value {
            JsonValue::Null => StructValue::Const(StructConst::Null),
            JsonValue::Bool(b) => StructValue::Const(StructConst::Boolean(*b)),
            JsonValue::Number(n) => StructValue::Const(StructConst::Number(n.as_f64().unwrap())),
            JsonValue::String(s) => StructValue::Const(StructConst::String(s.to_string())),
            JsonValue::Array(a) => StructValue::Array(
                Arc::new(a.iter().map(|x| Self::new_json_value(x)).collect())
            ),
            JsonValue::Object(obj) => StructValue::Object(
                Arc::new(obj.iter().map(|(k,v)| (k.to_string(),Self::new_json_value(v))).collect())
            ),
        }
    }

    pub fn to_json_value(&self) -> JsonValue {
        match self {
            StructValue::Const(c) => {
                match c {
                    StructConst::Number(n) => JsonValue::Number(Number::from_f64(*n).unwrap()),
                    StructConst::String(s) => JsonValue::String(s.to_string()),
                    StructConst::Boolean(b) => JsonValue::Bool(*b),
                    StructConst::Null => JsonValue::Null
                }
            },
            StructValue::Array(a) => JsonValue::Array(
                a.iter().map(|x| x.to_json_value()).collect()
            ),
            StructValue::Object(obj) => JsonValue::Object(
                obj.iter().map(|(k,v)| (k.to_string(),v.to_json_value())).collect()
            )
        }
    }

    pub fn build(&self) -> StructBuilt {
        match self {
            StructValue::Const(c) => StructBuilt::Const(c.clone()),
            StructValue::Array(a) => {
                StructBuilt::Array(Arc::new(a.iter().map(|x| x.build()).collect()),false)
            },
            StructValue::Object(j) => {
                StructBuilt::Object(Arc::new(j.iter().map(|(k,v)| {
                    (k.to_string(),v.build())
                }).collect()))
            }
        }
    }

    fn arm_order(&self) -> u8 {
        match self {
            StructValue::Const(_) => 0,
            StructValue::Array(_) => 1,
            StructValue::Object(_) => 2,
        }
    }
}

impl PartialEq for StructValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Const(l0), Self::Const(r0)) => l0 == r0,
            (Self::Array(l0), Self::Array(r0)) => l0 == r0,
            (Self::Object(l0), Self::Object(r0)) => l0 == r0,
            _ => false
        }
    }
}

impl Eq for StructValue {}

impl Hash for StructValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            StructValue::Const(c) => c.hash(state),
            StructValue::Array(a) => a.hash(state),
            StructValue::Object(obj) => obj.hash(state)
        }
    }
}

macro_rules! sv_ds_number {
    ($name:ident,$type:ty) => {
        fn $name<E>(self, v: $type) -> Result<Self::Value, E> where E: serde::de::Error {
            Ok(StructValue::Const(StructConst::Number(v as f64)))
        }    
    };
}

impl Serialize for StructValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer {
        match self {
            StructValue::Const(c) => c.serialize(serializer),
            StructValue::Array(a) => {
                let mut seq = serializer.serialize_seq(Some(a.len()))?;
                for v in a.iter() {
                    seq.serialize_element(v)?;
                }
                seq.end()
            },
            StructValue::Object(obj) => {
                let mut map = serializer.serialize_map(Some(obj.len()))?;
                for (k,v) in obj.iter() {
                    map.serialize_entry(k,v)?;
                }
                map.end()
            }
        }
    }
}

struct StructValueVisitor;

impl<'de> Visitor<'de> for StructValueVisitor {
    type Value = StructValue;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a StructValue")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where A: serde::de::SeqAccess<'de> {
        let mut data : Vec<StructValue> = vec![];
        while let Some(value) = seq.next_element()? { data.push(value); }
        Ok(StructValue::Array(Arc::new(data)))
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where M: MapAccess<'de> {
        let mut data : BTreeMap<String,StructValue> = BTreeMap::new();
        while let Some((key,value)) = access.next_entry()? {
            data.insert(key,value);
        }
        Ok(StructValue::Object(Arc::new(data)))
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where E: serde::de::Error {
        Ok(StructValue::Const(StructConst::Boolean(v)))
    }

    sv_ds_number!(visit_i64,i64);
    sv_ds_number!(visit_i128,i128);
    sv_ds_number!(visit_u64,u64);
    sv_ds_number!(visit_u128,u128);
    sv_ds_number!(visit_f64,f64);

    fn visit_none<E>(self) -> Result<Self::Value, E>
            where E: serde::de::Error {
        Ok(StructValue::Const(StructConst::Null))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where E: serde::de::Error {
        Ok(StructValue::Const(StructConst::String(v.to_string())))
    }
}

impl<'de> Deserialize<'de> for StructValue {
    fn deserialize<D>(deserializer: D) -> Result<StructValue, D::Error>
            where D: Deserializer<'de> {
        deserializer.deserialize_any(StructValueVisitor)
    }
}
