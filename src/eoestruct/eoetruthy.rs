use super::{eoestructdata::DataVisitor, eoestruct::{StructConst, StructResult, struct_error}, StructBuilt, StructValue};

/* Falsy values are:
 * false, 0, "", [], {}, null
 * Everything else is truthy.
 *
 * We implement this with visitors by allowing no more than one constant, array start, hash start,
 * and if a constant then being a falsy one.
 * 
 * As we want to short-circuit on true, that is treated as an error state and so our visitor is
 * called ProveFalsy.
 */

struct ProveFalsy {
    once: bool
}

impl ProveFalsy {
    fn once(&mut self) -> StructResult {
        if self.once { return Err(struct_error("")) }
        self.once = true;
        Ok(())
    }
}

impl DataVisitor for ProveFalsy {
    fn visit_const(&mut self, input: &StructConst) -> StructResult { 
        self.once()?;
        let truthy = match input {
            StructConst::Number(n) => *n != 0.,
            StructConst::String(s) => s != "",
            StructConst::Boolean(b) => *b,
            StructConst::Null => false,
        };
        if truthy { Err(struct_error("")) } else { Ok(()) }
    }
    fn visit_array_start(&mut self) -> StructResult { self.once() }
    fn visit_object_start(&mut self) -> StructResult { self.once() }
    fn visit_pair_start(&mut self, _key: &str) -> StructResult { Err(struct_error("")) }
}

pub(super) fn truthy(input: &StructBuilt) -> bool {
    let mut falsy = ProveFalsy { once: false };
    input.expand(None,&mut falsy).is_err()
}

impl StructBuilt {
    pub fn truthy(&self) -> bool { truthy(self) }
}

impl StructValue {
    pub fn truthy(&self) -> bool {
        match self {
            StructValue::Const(c) => c.truthy(),
            StructValue::Array(a) => a.len() != 0,
            StructValue::Object(obj) => obj.len() != 0
        }
    }
}
