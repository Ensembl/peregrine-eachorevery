pub mod eoestruct {
    mod eoestructdata;

    #[cfg(test)]
    mod test {
        mod eoestructtest;
    }
    
    mod structbuilt;
    mod build;
    mod eoetruthy;
    mod eoestruct;
    mod expand;
    mod eoejson;
    mod replace;
    mod structtemplate; 
    mod structvalue;

    #[cfg(any(debug_assertions,test))]
    mod eoedebug;
    
    pub use expand::{ struct_select };
    pub use eoestruct::{ StructVarGroup, StructConst };
    pub use eoejson::{ struct_to_json, struct_from_json, select_to_json };
    pub use structbuilt::{ StructBuilt };
    pub use structtemplate::{ StructTemplate, StructVar, StructPair };
    pub use structvalue::{ StructValue };
}

mod approxnumber;
mod eoefilter;
mod eachorevery;

pub use crate::eachorevery::{ EachOrEvery, EachOrEveryGroupCompatible };
pub use crate::eoefilter::{ EachOrEveryFilter, EachOrEveryFilterBuilder };
