use std::sync::Arc;
use crate::eachorevery::{EachOrEveryGroupCompatible};
use super::{eoestruct::{StructVarValue, StructValueId, StructResult, StructError, struct_error}, StructTemplate, structbuilt::StructBuilt, StructPair, StructVar, StructVarGroup};

struct Binding {
    id: StructValueId,
    pos: (usize,usize),
    value: Option<StructVarValue>
}

impl Binding {
    fn new(id: &StructValueId, depth: usize, width: usize) -> Binding {
        Binding { id: id.clone(), value: None, pos: (depth,width) }
    }
}

fn check_build_compatible(vars: &[Option<Arc<StructVarValue>>]) -> StructResult {
    let vars = vars.iter().filter_map(|x| x.as_ref()).collect::<Vec<_>>();
    if vars.len() == 0 {
        return Err(struct_error("no variables specified"));
    }
    let mut compat = EachOrEveryGroupCompatible::new(None);
    for var in vars {
        var.check_build_compatible(&mut compat);
    }
    if !compat.compatible() {
        return Err(struct_error("bindings of differing length"));
    }
    Ok(())
}

fn direct_conditionals(input: &[StructBuilt]) -> bool {
    for item in input {
        match item {
            StructBuilt::Condition(_,_,_) => { return true; }
            _ => {}
        }
    }
    false
}

impl StructTemplate {
    fn make(&self, bindings: &mut Vec<Binding>, all_depth: usize, first: bool) -> Result<StructBuilt,StructError> {
        Ok(match self {
            StructTemplate::Var(var) => {
                let index = bindings.iter().position(|id| id.id == var.id);
                let index = index.ok_or_else(|| struct_error("free variable in template"))?;
                bindings[index].value = Some(var.value.clone());
                let pos = bindings[index].pos;
                StructBuilt::Var(pos.0,pos.1)
            },
            StructTemplate::Const(c) => { StructBuilt::Const(c.clone()) }
            StructTemplate::Array(v) => {
                let data = v.iter().map(|x| x.make(bindings,all_depth,false)).collect::<Result<Vec<_>,_>>()?;
                let cond = direct_conditionals(&data);
                StructBuilt::Array(Arc::new(data),cond)
            },
            StructTemplate::Object(v) => {
                StructBuilt::Object(Arc::new(
                    v.iter().map(|x| Ok::<_,StructError>((x.0.clone(),x.1.make(bindings,all_depth,false)?))).collect::<Result<Vec<_>,_>>()?
                ))
            },
            StructTemplate::All(ids, expr) => {
                for (i,id) in ids.iter().enumerate() {
                    bindings.push(Binding::new(id,all_depth,i));
                }
                let obj = expr.make(bindings,all_depth+1,false)?;
                let keep_len = bindings.len()-ids.len();
                let removed = bindings.split_off(keep_len);
                let removed = removed.iter().map(|binding| {
                    binding.value.clone().map(|x| Arc::new(x))
                }).collect::<Vec<_>>();
                if removed.is_empty() {
                    let data = vec![obj];
                    let cond = direct_conditionals(&data);
                    StructBuilt::Array(Arc::new(data),cond)
                } else {
                    check_build_compatible(&removed)?;
                    StructBuilt::All(removed,Arc::new(obj))
                }
            },
            StructTemplate::Condition(var,expr) => {
                if first {
                    return Err(struct_error("conditionals banned at top level"));
                }
                let index = bindings.iter().position(|id| id.id == var.id);
                let index = index.ok_or_else(|| struct_error("free variable in template"))?;
                bindings[index].value = Some(var.value.clone());
                let pos = bindings[index].pos;
                let expr = expr.make(bindings,all_depth,false)?;
                StructBuilt::Condition(pos.0,pos.1,Arc::new(expr))
            }
        })
    }

    pub fn build(&self) -> Result<StructBuilt,StructError> {
        self.make(&mut vec![],0,true)
    }
}

impl StructBuilt {
    fn unmake(&self, vars: &mut Vec<Vec<Option<StructVar>>>) -> Result<StructTemplate,StructError> {
        Ok(match self {
            StructBuilt::Var(depth,index) => {
                StructTemplate::Var(vars[*depth][*index].clone().unwrap())
            },
            StructBuilt::Const(c) => StructTemplate::Const(c.clone()),
            StructBuilt::Array(e, _) => {
                let entries = e.as_ref().iter().map(|e|
                    e.unmake(vars)
                ).collect::<Result<Vec<_>,_>>()?;
                StructTemplate::Array(Arc::new(entries))
            },
            StructBuilt::Object(p) => {
                let pairs = p.as_ref().iter().map(|(k,b)| 
                    Ok::<_,StructError>(StructPair(k.to_string(),b.unmake(vars)?))
                ).collect::<Result<Vec<_>,_>>()?;
                StructTemplate::Object(Arc::new(pairs))
            },
            StructBuilt::All(values,expr) => {
                let mut group = StructVarGroup::new();
                let our_vars = values.iter().map(|v| {
                    v.as_ref().map(|v|
                        StructVar::new(&mut group,v.as_ref().clone())
                    )
                }).collect::<Vec<_>>();
                let ids = our_vars.iter().filter_map(|x| x.as_ref()).map(|v| v.id).collect();
                vars.push(our_vars);
                let value = expr.unmake(vars)?;
                vars.pop();
                StructTemplate::All(ids,Arc::new(value))
            },
            StructBuilt::Condition(depth, index, expr) => {
                StructTemplate::Condition(vars[*depth][*index].clone().unwrap(),Arc::new(expr.unmake(vars)?))
            }
        })
    }

    pub fn unbuild(&self) -> Result<StructTemplate,StructError> {
        self.unmake(&mut vec![])
    }
}
