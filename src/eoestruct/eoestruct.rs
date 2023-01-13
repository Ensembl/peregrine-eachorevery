use crate::{eachorevery::{EachOrEvery, EachOrEveryGroupCompatible}, approxnumber::ApproxNumber, EachOrEveryFilter};
use hashbrown::HashMap;
use ordered_float::OrderedFloat;
use serde::Serialize;
use std::{hash::{Hash}, cmp::Ordering, sync::{Arc, Mutex}};
use super::StructVar;
use lazy_static::lazy_static;

lazy_static! {
    static ref IDS : Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
}    

#[cfg(any(debug_assertions,test))]
pub fn struct_error(error: &str) -> String { error.to_string() }

#[cfg(not(any(debug_assertions,test)))]
pub(super) fn struct_error(msg: &str) -> String { String::new() }

#[derive(Copy,Clone,PartialEq,Eq,Hash)]
#[cfg_attr(debug_assertions,derive(Debug))]
pub struct StructValueId(pub(super) u64);

impl StructValueId {
    pub(super) fn new() -> StructValueId { 
        let mut idsrc = IDS.lock().unwrap();
        *idsrc += 1;
        let id = *idsrc;
        drop(idsrc);
        StructValueId(id)
    }
}

pub struct StructVarGroup(pub(super) Vec<StructValueId>);

impl StructVarGroup {
    pub fn new() -> StructVarGroup { StructVarGroup(vec![]) }
}

#[cfg_attr(debug_assertions,derive(Debug))]
#[derive(Clone)]
pub enum StructConst {
    Number(f64),
    String(String),
    Boolean(bool),
    Null
}

#[derive(PartialOrd,Ord,PartialEq,Eq)]
enum OrderableConstEnum<'a> {
    Number(OrderedFloat<f64>),
    String(&'a str),
    Boolean(bool),
    Null
}

#[derive(PartialOrd,Ord,PartialEq,Eq)]
struct OrderableConst<'a>(u8,OrderableConstEnum<'a>);

impl StructConst {
    pub fn truthy(&self) -> bool {
        match self {
            StructConst::Number(_) => true,
            StructConst::String(_) => true,
            StructConst::Boolean(b) => *b,
            StructConst::Null => false
        }
    }

    fn orderable<'a>(&'a self) -> OrderableConst<'a> {
        match self {
            StructConst::Number(n) => OrderableConst(2,OrderableConstEnum::Number(OrderedFloat(*n))),
            StructConst::String(s) => OrderableConst(3,OrderableConstEnum::String(s)),
            StructConst::Boolean(b) => OrderableConst(1,OrderableConstEnum::Boolean(*b)),
            StructConst::Null => OrderableConst(0,OrderableConstEnum::Null)
        }
    }
}

impl PartialOrd for StructConst {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.orderable().partial_cmp(&other.orderable())
    }
}

impl Ord for StructConst {
    fn cmp(&self, other: &Self) -> Ordering {
        self.orderable().cmp(&other.orderable())
    }
}

impl PartialEq for StructConst {
    fn eq(&self, other: &Self) -> bool {
        self.orderable().eq(&other.orderable())
    }
}

impl Eq for StructConst {}

const SIG_FIG : i32 = 14; // In theory 15 preserved by f64

impl Hash for StructConst {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            StructConst::Number(n) => { ApproxNumber(*n,SIG_FIG).hash(state); },
            StructConst::String(s) => s.hash(state),
            StructConst::Boolean(b) => b.hash(state),
            StructConst::Null => {}
        }
    }
}

impl Serialize for StructConst {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer {
        match self {
            StructConst::Number(n) => serializer.serialize_f64(*n),
            StructConst::String(s) => serializer.serialize_str(s),
            StructConst::Boolean(b) => serializer.serialize_bool(*b),
            StructConst::Null => serializer.serialize_none()
        }
    }
}

pub struct LateValues(HashMap<StructValueId,StructVarValue>);

impl LateValues {
    pub fn new() -> LateValues { LateValues(HashMap::new()) }

    pub fn add(&mut self, var: &StructVar, val: &StructVar) -> Result<(),String> {
        let id = match &var.value {
            StructVarValue::Late(id) => id.clone(),
            _ => { return Err(struct_error("can only bind to late variables")) }
        };
        if let StructVarValue::Late(_) = &val.value {
            return Err(struct_error("cannot bind late variables to late variables")) 
        }
        self.0.insert(id,val.value.clone());
        Ok(())
    }
}

#[derive(Clone)]
/* Guarantee: all EachOrEverys in here will be Each after construction */
pub enum StructVarValue {
    Number(EachOrEvery<f64>),
    String(EachOrEvery<String>),
    Boolean(EachOrEvery<bool>),
    Late(StructValueId)
}

impl Hash for StructVarValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            StructVarValue::Number(n) => {
                n.map(|v| ApproxNumber(*v,SIG_FIG)).hash(state);
            },
            StructVarValue::String(s) => s.hash(state),
            StructVarValue::Boolean(b) => b.hash(state),
            StructVarValue::Late(v) => v.hash(state)
        }
    }
}

fn to_const<X>(input: &EachOrEvery<X>) -> Option<&X> {
    if input.len().is_none() {
        Some(input.get(0).unwrap()) // EoE every is guaranteed to be Some
    } else {
        None
    }
}
 
fn format<X: std::fmt::Debug>(value: &EachOrEvery<X>, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(len) = value.len() {
        let mut sep = false;
        write!(f,"<")?;
        for value in value.iter(len).unwrap() { // guaranteed by outer conditional
            if sep { write!(f,",")?; }
            write!(f,"{:?}",value)?;
            sep = true;
        }
        write!(f,">")?;
    } else {
        let value = value.iter(1).unwrap().next().unwrap(); // EoE every is guaranteed to be Some
        write!(f,"{:?}",value)?;
    }
    Ok(())
}

impl std::fmt::Debug for StructVarValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StructVarValue::Number(x) => format(x,f),
            StructVarValue::String(x) => format(x,f),
            StructVarValue::Boolean(x) => format(x,f),
            StructVarValue::Late(_) => write!(f,"?")
        }
    }
}

impl StructVarValue {
    pub(super) fn to_const(&self) -> Option<StructConst> {
        match self {
            StructVarValue::Number(input) => {
                to_const(input).map(|x| StructConst::Number(*x))
            },
            StructVarValue::String(input) => {
                to_const(input).map(|x| StructConst::String(x.clone()))
            },
            StructVarValue::Boolean(input) => {
                to_const(input).map(|x| StructConst::Boolean(*x))
            },
            StructVarValue::Late(_) => None
        }
    }

    fn resolve<'a>(&'a self, lates: Option<&'a LateValues>) -> Result<&StructVarValue,String> {
        match self {
            StructVarValue::Late(id) => {
                lates.and_then(|lates| lates.0.get(id))
                     .ok_or_else(|| struct_error("missing late value"))?
                     .resolve(lates)
            },
            x => Ok(x)
        }
    }

    pub(super) fn is_finite(&self, lates: Option<&LateValues>) -> Result<bool,String> {
        Ok(match self.resolve(lates)? {
            StructVarValue::Number(x) => x.len().is_some(),
            StructVarValue::String(x) => x.len().is_some(),
            StructVarValue::Boolean(x) => x.len().is_some(),
            StructVarValue::Late(_) => panic!("invariant error: late after resolve()")
        })
    }

    pub(super) fn check_build_compatible(&self, compat: &mut EachOrEveryGroupCompatible) {
        match self {
            StructVarValue::Number(input) => { compat.add(input); },
            StructVarValue::String(input) => { compat.add(input); },
            StructVarValue::Boolean(input) => { compat.add(input); },
            StructVarValue::Late(_) => {}
        };
    }

    pub(super) fn check_compatible(&self, lates: Option<&LateValues>, compat: &mut EachOrEveryGroupCompatible) -> Result<(),String> {
        match self.resolve(lates)? {
            StructVarValue::Number(input) => { compat.add(input); },
            StructVarValue::String(input) => { compat.add(input); },
            StructVarValue::Boolean(input) => { compat.add(input); },
            StructVarValue::Late(_) => panic!("invariant error: late after resolve()")
        };
        Ok(())
    }

    pub(super) fn get<'a>(&'a self, lates: Option<&LateValues>, index: usize) -> Result<StructConst,String> {
       Ok(match self.resolve(lates)? {
            StructVarValue::Number(input) => {
                StructConst::Number(*input.get(index).unwrap())
            },
            StructVarValue::String(input) => {
                StructConst::String(input.get(index).unwrap().clone())
            },
            StructVarValue::Boolean(input) => {
                StructConst::Boolean(*input.get(index).unwrap())
            },
            StructVarValue::Late(_) => panic!("invariant error: late after resolve()")
        })
    }

    pub(super) fn exists<'a>(&'a self, lates: Option<&LateValues>, index: usize) -> Result<bool,String> {
        Ok(match self.resolve(lates)? {
            StructVarValue::Number(input) => {
                input.get(index).is_some()
            },
            StructVarValue::String(input) => {
                input.get(index).is_some()
            },
            StructVarValue::Boolean(input) => {
                input.get(index).is_some()
            },
            StructVarValue::Late(_) => panic!("invariant error: late after resolve()")
        })
    }

    pub(super) fn filter(&self, filter: &EachOrEveryFilter) -> StructVarValue {
        match self {
            StructVarValue::Number(n) => StructVarValue::Number(n.filter(filter)),
            StructVarValue::String(s) => StructVarValue::String(s.filter(filter)),
            StructVarValue::Boolean(b) => StructVarValue::Boolean(b.filter(filter)),
            StructVarValue::Late(x) => StructVarValue::Late(x.clone())
        }
    }
}
