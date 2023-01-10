use std::{sync::Arc, collections::{HashMap, BTreeMap} };
use super::{StructTemplate, StructError, eoestruct::{struct_error, StructVarValue}, StructPair, StructVar, structvalue::StructValue };

struct PathSet<X> {
    kids: HashMap<String,Box<PathSet<X>>>,
    here: Option<X>
}

impl<X> PathSet<X> {
    fn empty() -> PathSet<X> {
        PathSet {
            kids: HashMap::new(),
            here: None
        }
    }

    fn take(&mut self, path: &[&str]) -> Option<X> {
        if path.len() == 0 {
            self.here.take()
        } else if let Some(kid) = self.kids.get_mut(path[0]) {
            kid.take(&path[1..])
        } else {
            None
        }
    }

    fn set(&mut self, path: &[&str], value: X) {
        if path.len() == 0 {
            self.here = Some(value);
        } else {
            if !self.kids.contains_key(path[0]) {
                self.kids.insert(path[0].to_string(),Box::new(PathSet::empty()));
            }
            self.kids.get_mut(path[0]).unwrap().set(&path[1..],value);
        }
    }
}

impl StructTemplate {
    fn do_find_path<'a,X>(&'a self, repls: &mut PathSet<Option<X>>, cb: Arc<dyn Fn(&StructTemplate) -> Result<X,StructError>>) -> Result<(),StructError> {
        if let Some(repl) = repls.here.as_mut() {
            repl.replace(cb(self)?);
            Ok(())
        } else {
            match self {
                StructTemplate::Array(v) => {
                    for (i,item) in v.iter().enumerate() {
                        if let Some(kid_repls) = repls.kids.get_mut(&i.to_string()) {
                            item.do_find_path(kid_repls,cb.clone())?
                        }
                    }
                },
                StructTemplate::Object(pp) => {
                    for pair in pp.iter() {
                        if let Some(kid_repls) = repls.kids.get_mut(&pair.0) {
                            pair.1.do_find_path(kid_repls,cb.clone())?
                        }
                    }
                },
                StructTemplate::All(_,t) => {
                    if let Some(kid_repls) = repls.kids.get_mut("*") {
                        t.do_find_path(kid_repls,cb)?;
                    }
                },
                StructTemplate::Condition(_,t) => {
                    if let Some(kid_repls) = repls.kids.get_mut("&") {
                        t.do_find_path(kid_repls,cb.clone())?;
                    }
                },
                _ => { return Err(struct_error("bad path")); }
            }
            Ok(())
        }
    }

    fn do_replace_path<'a,X>(&'a self, repls: &mut PathSet<X>, cb: Arc<dyn Fn(&StructTemplate,X) -> Result<StructTemplate,StructError>>) -> Result<StructTemplate,StructError> {
        if let Some(repl) = repls.here.take() {
            cb(self,repl)
        } else {
            match self {
                StructTemplate::Array(v) => {
                    let mut out = vec![];
                    for (i,item) in v.iter().enumerate() {
                        out.push(if let Some(kid_repls) = repls.kids.get_mut(&i.to_string()) {
                            item.do_replace_path(kid_repls,cb.clone())?
                        } else {
                            item.clone()
                        });                    
                    }
                    return Ok(StructTemplate::Array(Arc::new(out)));
                },
                StructTemplate::Object(pp) => {
                    let mut out = vec![];
                    for pair in pp.iter() {
                        let value = if let Some(kid_repls) = repls.kids.get_mut(&pair.0) {
                            pair.1.do_replace_path(kid_repls,cb.clone())?
                        } else {
                            pair.1.clone()
                        };
                        out.push(StructPair(pair.0.clone(),value));
                    }
                    return Ok(StructTemplate::Object(Arc::new(out)));
                },
                StructTemplate::All(v,t) => {
                    let value = if let Some(kid_repls) = repls.kids.get_mut("*") {
                        Arc::new(t.do_replace_path(kid_repls,cb)?)
                    } else {
                        t.clone()
                    };
                    return Ok(StructTemplate::All(v.clone(),value));
                },
                StructTemplate::Condition(v,t) => {
                    let value = if let Some(kid_repls) = repls.kids.get_mut("&") {
                        Arc::new(t.do_replace_path(kid_repls,cb.clone())?)
                    } else {
                        t.clone()
                    };
                    return Ok(StructTemplate::Condition(v.clone(),value));
                },
                _ => {}
            }
            Err(struct_error("bad path"))
        }
    }

    pub fn extract(&self, path: &[&str]) -> Result<StructTemplate,StructError> {
        let mut repls = PathSet::empty();
        repls.set(path,None);
        self.do_find_path(&mut repls, Arc::new(|x| { Ok(x.clone()) }))?;
        repls.take(path).flatten().ok_or_else(|| 
            struct_error("bad path")
        )
    }

    pub fn extract_value(&self, path: &[&str]) -> Result<StructVarValue,StructError> {
        let mut repls = PathSet::empty();
        repls.set(path,None);
        self.do_find_path(&mut repls, Arc::new(|x| {
            match x {
                StructTemplate::Var(v) => {
                    Ok(v.value.clone())
                },
                StructTemplate::Condition(v,_) => {
                    Ok(v.value.clone())
                },
                _ => { Err(struct_error("bad path")) }
            }
        }))?;
        repls.take(path).flatten().ok_or_else(|| 
            struct_error("bad path")
        )
    }

    fn do_replace(&self, path: &[&str], value: StructTemplate) -> Result<StructTemplate,StructError> {
        let mut repls = PathSet::empty();
        repls.set(path,value);
        let out = self.do_replace_path(&mut repls, Arc::new(|_,new| { Ok(new) }));
        if repls.take(path).is_some() {
            return Err(struct_error("bad path"));
        }
        out
    }

    pub fn replace(&self, path: &[&str], mut value: StructTemplate, copy: &[(&[&str],&[&str])]) -> Result<StructTemplate,StructError> {
        for (src,dst) in copy {
            value = value.do_replace(dst,self.extract(src)?)?;
        }
        self.do_replace(path,value)
    }

    pub fn substitute(&self, path: &[&str], value: StructVar) -> Result<StructTemplate,StructError> {
        let mut repls = PathSet::empty();
        repls.set(path,value);
        self.do_replace_path(&mut repls, Arc::new(|old,new| {
            match old {
                StructTemplate::Var(v) => {
                    Ok(StructTemplate::Var(StructVar { value: new.value, id: v.id }))
                },
                StructTemplate::Condition(v,e) => {
                    Ok(StructTemplate::Condition(StructVar { value: new.value, id: v.id },e.clone()))
                },
                _ => { Err(struct_error("bad path")) }
            }
        }))
    }
}

impl StructValue {
    fn do_find_path<'a,X>(&'a self, repls: &mut PathSet<Option<X>>, cb: Arc<dyn Fn(&StructValue) -> Result<X,StructError>>) -> Result<(),StructError> {
        if let Some(repl) = repls.here.as_mut() {
            repl.replace(cb(self)?);
            Ok(())
        } else {
            match self {
                StructValue::Array(v) => {
                    for (i,item) in v.iter().enumerate() {
                        if let Some(kid_repls) = repls.kids.get_mut(&i.to_string()) {
                            item.do_find_path(kid_repls,cb.clone())?
                        }
                    }
                },
                StructValue::Object(pp) => {
                    for (key,value) in pp.iter() {
                        if let Some(kid_repls) = repls.kids.get_mut(key) {
                            value.do_find_path(kid_repls,cb.clone())?
                        }
                    }
                },
                _ => { return Err(struct_error("bad path")); }
            }
            Ok(())
        }
    }

    fn do_replace_path<'a,X>(&'a self, repls: &mut PathSet<X>, cb: Arc<dyn Fn(&StructValue,X) -> Result<StructValue,StructError>>) -> Result<StructValue,StructError> {
        if let Some(repl) = repls.here.take() {
            cb(self,repl)
        } else {
            match self {
                StructValue::Array(v) => {
                    let mut out = vec![];
                    for (i,item) in v.iter().enumerate() {
                        out.push(if let Some(kid_repls) = repls.kids.get_mut(&i.to_string()) {
                            item.do_replace_path(kid_repls,cb.clone())?
                        } else {
                            item.clone()
                        });                    
                    }
                    return Ok(StructValue::Array(Arc::new(out)));
                },
                StructValue::Object(pp) => {
                    let mut out = BTreeMap::new();
                    for (key,value) in pp.iter() {
                        let value = if let Some(kid_repls) = repls.kids.get_mut(key) {
                            value.do_replace_path(kid_repls,cb.clone())?
                        } else {
                            value.clone()
                        };
                        out.insert(key.clone(),value);
                    }
                    return Ok(StructValue::Object(Arc::new(out)));
                }
                _ => {}
            }
            Err(struct_error("bad path"))
        }
    }

    pub fn extract(&self, path: &[&str]) -> Result<StructValue,StructError> {
        let mut repls = PathSet::empty();
        repls.set(path,None);
        self.do_find_path(&mut repls, Arc::new(|x| { Ok(x.clone()) }))?;
        repls.take(path).flatten().ok_or_else(|| 
            struct_error("bad path")
        )
    }

    fn do_replace(&self, path: &[&str], value: StructValue) -> Result<StructValue,StructError> {
        let mut repls = PathSet::empty();
        repls.set(path,value);
        let out = self.do_replace_path(&mut repls, Arc::new(|_,new| { Ok(new) }));
        if repls.take(path).is_some() {
            return Err(struct_error("bad path"));
        }
        out
    }

    pub fn replace(&self, path: &[&str], mut value: StructValue, copy: &[(&[&str],&[&str])]) -> Result<StructValue,StructError> {
        for (src,dst) in copy {
            value = value.do_replace(dst,self.extract(src)?)?;
        }
        self.do_replace(path,value)
    }
}
