use std::collections::{HashMap, HashSet};
use crate::eachorevery::EachOrEvery;
use super::{eoestruct::{StructConst, StructError, struct_error, StructVarGroup, LateValues, StructResult}, structtemplate::{StructVar, StructPair}, StructTemplate, eoestructdata::{DataStackTransformer, eoestack_run}, structbuilt::StructBuilt, expand::StructSelectorVisitor};
use serde_json::{Value as JsonValue, Number, Map};

struct JsonTransformer;

impl DataStackTransformer<StructConst,JsonValue> for JsonTransformer {
    fn make_singleton(&mut self, value: StructConst) -> JsonValue {
        match value {
            StructConst::Number(input) => JsonValue::Number(Number::from_f64(input).unwrap()),
            StructConst::String(input) => JsonValue::String(input),
            StructConst::Boolean(input) => JsonValue::Bool(input),
            StructConst::Null => JsonValue::Null
        }
    }

    fn make_array(&mut self, value: Vec<JsonValue>) -> JsonValue {
        JsonValue::Array(value)
    }

    fn make_object(&mut self, value: Vec<(String,JsonValue)>) -> JsonValue {
        JsonValue::Object(value.iter().map(|x| x.clone()).collect())
    }
}

pub fn struct_to_json(input: &StructBuilt, lates: Option<&LateValues>) -> Result<JsonValue,StructError> {
    eoestack_run(input,lates,JsonTransformer)
}

fn to_var_type<F,G,X>(input: &[JsonValue], cb: F, cb2: G) -> Result<StructVar,StructError>
        where F: Fn(&JsonValue) -> Option<X>, G: FnOnce(EachOrEvery<X>) -> StructVar {
    let values = input.iter().map(cb).collect::<Option<Vec<_>>>();
    Ok(cb2(values.map(|x| EachOrEvery::each(x)).ok_or(struct_error("non-homogenous variable"))?))
}

pub(super) fn array_to_var(group: &mut StructVarGroup, values: &[JsonValue]) -> Result<StructVar,StructError> {
    if let Some(first) = values.first() {
        match first {
            JsonValue::Bool(_) => {
                to_var_type(values, |x| {
                    if let JsonValue::Bool(x) = x { Some(*x) } else { None }
                }, |x| {
                    StructVar::new_boolean(group,x)
                })
            },
            JsonValue::Number(_) => {
                to_var_type(values, |x| {
                    if let JsonValue::Number(x) = x { Some(x.as_f64().unwrap()) } else { None }
                }, |x| {
                    StructVar::new_number(group,x)
                })
            },
            JsonValue::String(_) => {
                to_var_type(values, |x| {
                    if let JsonValue::String(x) = x { Some(x.to_string()) } else { None }
                }, |x| {
                    StructVar::new_string(group,x)
                })
            },
            _ =>  Err(struct_error("var in json of unknown type"))
        }
    } else {
        /* zero-length is fine */
        Ok(StructVar::new_boolean(group,EachOrEvery::each(vec![])))
    }
}

struct EoeFromJson {
    specs: HashSet<String>,
    ifs: HashSet<String>,
    vars: Vec<HashMap<String,StructVar>>,
    lates: Vec<(String,StructVar)>
}

impl EoeFromJson {
    fn new(mut specs: Vec<String>, mut ifs: Vec<String>, json: &JsonValue) ->  Result<(StructTemplate,Vec<(String,StructVar)>),StructError> {
        let mut obj = EoeFromJson{
            specs: specs.drain(..).collect(),
            ifs: ifs.drain(..).collect(),
            vars: vec![],
            lates: vec![]
        };
        let template = obj.build(json)?;
        Ok((template,obj.lates))
    }

    fn to_var(&mut self, group: &mut StructVarGroup, key: &str, input: &JsonValue) -> Result<StructVar,StructError> {
        let values = match input {
            JsonValue::Array(x) => x.as_slice(),
            JsonValue::Null => {
                let late = StructVar::new_late(group);
                self.lates.push((key.to_string(), late.clone()));
                return Ok(late);
            },
            _ => &[]
        };
        array_to_var(group,values)
    }
    
    fn to_all(&mut self, map: &Map<String,JsonValue>) -> Result<Option<StructTemplate>,StructError> {
        let mut group = StructVarGroup::new();
        let mut expr = None;
        for key in map.keys() {
            if self.specs.contains(key) { expr = Some(key); break; }
        }
        let expr = if let Some(expr) = expr { expr } else { return Ok(None); };
        let mut vars = vec![];
        let mut var_names = HashMap::new();
        for (key,value) in map.iter() {
            if key == expr { continue; }
            let var = self.to_var(&mut group,key,&value)?;
            vars.push(var.clone());
            var_names.insert(key.clone(),var);
        }
        self.vars.push(var_names);
        let expr = self.build(map.get(expr).unwrap())?; // expr guranteed in map during setting
        self.vars.pop();
        Ok(Some(StructTemplate::new_all(&mut group,expr)))
    }

    fn to_condition(&mut self, map: &Map<String,JsonValue>) -> Result<Option<StructTemplate>,StructError> {
        let mut expr = None;
        for key in map.keys() {
            if self.ifs.contains(key) { expr = Some(key); break; }
        }
        let expr = if let Some(expr) = expr { expr } else { return Ok(None); };
        let value = self.build(map.get(expr).unwrap())?; // expr guranteed in map during setting
        for map in self.vars.iter().rev() {
            if let Some(var) = map.get(expr) {
                return Ok(Some(StructTemplate::new_condition(var.clone(),value)));
            }
        }
        Ok(None)
    }

    fn build(&mut self, json: &JsonValue) ->  Result<StructTemplate,StructError> {
        Ok(match json {
            JsonValue::Null => StructTemplate::new_null(),
            JsonValue::Bool(x) => StructTemplate::new_boolean(x.clone()),
            JsonValue::Number(x) => StructTemplate::new_number(x.as_f64().unwrap()),
            JsonValue::String(x) => {
                for map in self.vars.iter().rev() {
                    if let Some(var) = map.get(x) {
                        return Ok(StructTemplate::new_var(var));
                    }
                }
                StructTemplate::new_string(x.clone())
            },
            JsonValue::Array(x) => {
                let values = x.iter().map(|x| self.build(x)).collect::<Result<_,_>>()?;
                StructTemplate::new_array(values)
            },
            JsonValue::Object(x) => {
                if let Some(all) = self.to_all(&x)? {
                    all
                } else if let Some(cond) = self.to_condition(&x)? {
                    cond
                } else {
                    StructTemplate::new_object(x.iter().map(|(k,v)|{
                        Ok::<StructPair,StructError>(StructPair(k.to_string(),self.build(v)?))
                    }).collect::<Result<_,_>>()?)
                }
            }
        })
    }
}

pub fn struct_from_json(alls: Vec<String>, ifs: Vec<String>, json: &JsonValue) -> Result<(StructTemplate,Vec<(String,StructVar)>),StructError> {
    EoeFromJson::new(alls,ifs,json)
}

struct SelectJsonArray {
    output: Vec<JsonValue>
}

impl StructSelectorVisitor for SelectJsonArray {
    fn constant(&mut self, constant: &StructConst) -> StructResult {
        let value = match constant {
            StructConst::Number(n) => { JsonValue::Number(Number::from_f64(*n).unwrap()) }
            StructConst::String(s) => { JsonValue::String(s.clone()) }
            StructConst::Boolean(b) => { JsonValue::Bool(*b) },
            StructConst::Null => { JsonValue::Null }
        };
        self.output.push(value);
        Ok(())
    }

    fn missing(&mut self) -> StructResult {
        self.output.push(JsonValue::Null);
        Ok(())
    }
}

pub fn select_to_json(data: &StructBuilt, path: &[String], lates: Option<&LateValues>) -> Result<JsonValue,StructError> {
    let mut out = SelectJsonArray { output: vec![] };
    data.select(lates,path,&mut out)?;
    Ok(JsonValue::Array(out.output))
}
