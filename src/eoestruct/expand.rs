use std::sync::Arc;
use crate::eachorevery::{EachOrEveryGroupCompatible};
use super::{eoestruct::{StructConst, StructVarValue, StructResult, struct_error, StructError, LateValues}, eoestructdata::DataVisitor, structbuilt::StructBuilt, structvalue::StructValue};

pub trait StructSelectorVisitor {
    fn constant(&mut self, constant: &StructConst) -> StructResult;
    fn missing(&mut self) -> StructResult;
}

fn separate<'a,F,Y>(input: &mut dyn Iterator<Item=Y>, mut cb: F, visitor: &mut dyn DataVisitor) -> StructResult
        where F: FnMut(Y,&mut dyn DataVisitor) -> StructResult {
    let mut first = true;
    for item in input {
        if !first { visitor.visit_separator()?; }
        cb(item,visitor)?;
        first = false;
    }
    Ok(())        
}

struct AllState {
    vars: Vec<Option<Arc<StructVarValue>>>,
    next_index: usize,
    first: usize
}

struct GlobalState<'a> {
    lates: Option<&'a LateValues>,
    alls: Vec<AllState>
}

fn check_compatible(vars: &[Option<Arc<StructVarValue>>], lates: Option<&LateValues>) -> StructResult {
    let mut compat = EachOrEveryGroupCompatible::new(None);
    for item in vars.iter().filter_map(|x| x.as_deref()) {
        item.check_compatible(lates, &mut compat)?;
    }
    if !compat.compatible() {
        return Err(struct_error("late variables incompatible with earlies"));
    }
    Ok(())
}

impl AllState {
    fn new(vars: Vec<Option<Arc<StructVarValue>>>, lates: Option<&LateValues>,next_index: usize) -> Result<AllState,StructError> {
        check_compatible(&vars,lates)?;
        let first = vars.iter().position(|x| 
            x.as_ref().map(|x| x.is_finite(lates).ok().unwrap_or(false)).unwrap_or(false)
        ).ok_or_else(|| struct_error("no infinite recursion allowed"))?;
        Ok(AllState { vars, next_index, first })
    }

    fn get(&self, lates: Option<&LateValues>, width: usize) -> Result<StructConst,StructError> {
        self.vars[width].as_ref().unwrap().get(lates,self.next_index-1)
    }

    fn row(&mut self, lates: Option<&LateValues>) -> Result<bool,StructError> {
        self.next_index += 1;
        self.vars[self.first].as_ref().unwrap().exists(lates,self.next_index-1)
    }
}

impl StructBuilt {
    fn split(&self, output: &mut dyn DataVisitor, data: &mut GlobalState) -> StructResult {
        match self {
            StructBuilt::Var(depth,width) => {
                output.visit_const(&data.alls[*depth].get(data.lates,*width)?)?;
            },
            StructBuilt::Const(value) => {
                output.visit_const(value)?;
            },
            StructBuilt::Array(values,_) => {
                output.visit_array_start()?;
                separate(&mut values.iter(),|value,visitor| {
                    value.split(visitor,data)
                },output)?;
                output.visit_array_end()?;
            },
            StructBuilt::Object(values) => {
                output.visit_object_start()?;
                separate(&mut values.iter(), |kv,visitor| {
                    visitor.visit_pair_start(&kv.0)?;
                    kv.1.split(visitor,data)?;
                    visitor.visit_pair_end(&kv.0)
                },output)?;
                output.visit_object_end()?;
            },
            StructBuilt::All(vars,expr) => {
                let all = AllState::new(vars.to_vec(),data.lates,0)?;
                data.alls.push(all);
                output.visit_array_start()?;
                let mut first = true;
                loop {
                    let top = data.alls.last_mut().unwrap(); // data only manipulated here and just pushed
                    if !top.row(data.lates)? { break; }
                    if !first { output.visit_separator()?; }
                    expr.split(output,data)?;
                    first = false;
                }
                output.visit_array_end()?;
                data.alls.pop();
            }
            StructBuilt::Condition(depth,width,expr) => {
                if data.alls[*depth].get(data.lates,*width)?.truthy() {
                    expr.split(output,data)?;
                }
            }
        }
        Ok(())
    }

    pub fn expand(&self, lates: Option<&LateValues>, output: &mut dyn DataVisitor) -> StructResult {
        self.split(output,&mut GlobalState { alls: vec![], lates })
    }

    fn is_present(&self, data: &mut GlobalState) -> Result<bool,StructError> {
        Ok(match self {
            StructBuilt::Condition(depth,width,_expr) =>
                data.alls[*depth].get(data.lates,*width)?.truthy(),
            _ =>
                true
        })
    }

    fn do_select(&self, visitor: &mut dyn StructSelectorVisitor, data: &mut GlobalState, path: &[String]) -> StructResult {
        match self {
            StructBuilt::Var(depth,width) => {
                if path.len() != 0 { visitor.missing()?; return Ok(()); }
                visitor.constant(&data.alls[*depth].get(data.lates,*width)?)?;
            },
            StructBuilt::Const(c) => {
                if path.len() != 0 { visitor.missing()?; return Ok(()); }
                visitor.constant(c)?;
            },
            StructBuilt::Array(array,has_conditions) => {
                if path.len() == 0 { visitor.missing()?; return Ok(()); }
                if &path[0] == "*" {
                    for value in array.iter() {
                        value.do_select(visitor,data,&path[1..])?;
                    }
                } else if let Some(offset) = path[0].parse::<usize>().ok() {
                    if *has_conditions {
                        let mut cur_offset = 0;
                        let mut items = array.iter();
                        while let Some(next) = items.next() {
                            if next.is_present(data)? {
                                if cur_offset == offset {
                                    next.do_select(visitor,data,&path[1..])?;
                                    return Ok(());
                                }
                                cur_offset += 1;
                            }
                        }
                        visitor.missing()?;
                        return Ok(())
                    } else {
                        if let Some(value) = array.get(offset) {
                            value.do_select(visitor,data,&path[1..])?;
                        } else {
                            visitor.missing()?;
                        }
                    }
                } else {
                    return Err(struct_error("bad path component"));
                }
            },
            StructBuilt::Object(obj) => {
                if path.len() == 0 { visitor.missing()?; return Ok(()); }
                let mut result = None;
                for (key,value) in obj.iter() { // TODO to hash?
                    if key == &path[0] {
                        result = Some(value);
                    }
                }
                if let Some(value) = result {
                    value.do_select(visitor,data,&path[1..])?;
                } else {
                    visitor.missing()?;
                }
            },
            StructBuilt::All(vars,expr) => {
                if path.len() == 0 { visitor.missing()?; return Ok(()); }
                if &path[0] == "*" {
                    let all = AllState::new(vars.to_vec(),data.lates,0)?;
                    data.alls.push(all);    
                    loop {
                        let top = data.alls.last_mut().unwrap(); // data only manipulated here and just pushed
                        if !top.row(data.lates)? { break; }
                        expr.do_select(visitor,data,&path[1..])?;
                    }
                } else if let Some(offset) = path[0].parse::<usize>().ok() {
                    let all = AllState::new(vars.to_vec(),data.lates,offset)?;
                    data.alls.push(all);    
                    let top = data.alls.last_mut().unwrap(); // data only manipulated here and just pushed
                    if !top.row(data.lates)? {
                        visitor.missing()?;
                        return Ok(());
                    }
                    expr.do_select(visitor,data,&path[1..])?;
                } else {
                    return Err(struct_error("bad path component"));
                }
                data.alls.pop();
            },
            StructBuilt::Condition(depth,width,expr) => {
                if data.alls[*depth].get(data.lates,*width)?.truthy() {
                    expr.do_select(visitor,data,path)?;
                } else {
                    visitor.missing()?;
                }
            }
        }
        Ok(())
    }

    pub fn select(&self, lates: Option<&LateValues>, path: &[String], visitor: &mut dyn StructSelectorVisitor) -> StructResult {
        self.do_select(visitor,&mut GlobalState { alls: vec![], lates },path)
    }
}

impl StructValue {
    fn split(&self, output: &mut dyn DataVisitor, data: &mut GlobalState) -> StructResult {
        match self {
            StructValue::Const(value) => {
                output.visit_const(value)?;
            },
            StructValue::Array(values) => {
                output.visit_array_start()?;
                separate(&mut values.iter(),|value,visitor| {
                    value.split(visitor,data)
                },output)?;
                output.visit_array_end()?;
            },
            StructValue::Object(values) => {
                output.visit_object_start()?;
                separate(&mut values.iter(), |kv,visitor| {
                    visitor.visit_pair_start(&kv.0)?;
                    kv.1.split(visitor,data)?;
                    visitor.visit_pair_end(&kv.0)
                },output)?;
                output.visit_object_end()?;
            }
        }
        Ok(())
    }

    pub fn expand(&self, lates: Option<&LateValues>, output: &mut dyn DataVisitor) -> StructResult {
        self.split(output,&mut GlobalState { alls: vec![], lates })
    }
}

struct SelectJsonArray {
    output: Vec<Option<StructConst>>
}

impl StructSelectorVisitor for SelectJsonArray {
    fn constant(&mut self, constant: &StructConst) -> StructResult {
        self.output.push(Some(constant.clone()));
        Ok(())
    }

    fn missing(&mut self) -> StructResult {
        self.output.push(None);
        Ok(())
    }
}

pub fn struct_select(data: &StructBuilt, path: &[String], lates: Option<&LateValues>) -> Result<Vec<Option<StructConst>>,StructError> {
    let mut out = SelectJsonArray { output: vec![] };
    data.select(lates,path,&mut out)?;
    Ok(out.output)
}
