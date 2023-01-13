use std::sync::Arc;
use super::eoestruct::{StructConst, StructVarValue};

#[derive(Clone)]
pub enum StructBuilt {
    Var(usize,usize),
    Const(StructConst),
    Array(Arc<Vec<StructBuilt>>,bool),
    Object(Arc<Vec<(String,StructBuilt)>>),
    All(Vec<Option<Arc<StructVarValue>>>,Arc<StructBuilt>),
    Condition(usize,usize,Arc<StructBuilt>)
}

impl StructBuilt {
    pub fn is_null(&self) -> bool {
        if let StructBuilt::Const(StructConst::Null) = self { true } else { false }
    }
}
