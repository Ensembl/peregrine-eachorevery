use std::collections::HashMap;
use super::{eoestruct::{StructResult, StructError, StructConst, StructValueId}, StructTemplate, structbuilt::StructBuilt};


#[cfg(debug_assertions)]
pub(super) fn comma_separate<'a,F,Y>(input: &[Y], mut cb: F, output: &mut String) -> StructResult
        where F: FnMut(&Y,&mut String) -> StructResult {
    let mut first = true;
    for item in input {
        if !first { output.push_str(","); }
        cb(item,output)?;
        first = false;
    }
    Ok(())
}

#[cfg(debug_assertions)]
struct TemplateVarsFormatter {
    name: HashMap<StructValueId,usize>
}

#[cfg(debug_assertions)]
impl TemplateVarsFormatter {
    pub(super) fn new() -> TemplateVarsFormatter {
        TemplateVarsFormatter {
            name: HashMap::new()
        }
    }

    fn get(&mut self, value: &StructValueId) -> String {
        let len = self.name.len();
        let index = *self.name.entry(*value).or_insert(len);
        let vars = ('a'..'z').collect::<String>();
        let series = index / (vars.len());
        let series = if series > 0 { format!("{}",series) } else { "".to_string() };
        let offset = index % (vars.len());
        format!("{}{}",series,vars.chars().nth(offset).unwrap())
    }
}

impl std::fmt::Debug for StructTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let out = self.format().ok().unwrap_or_else(|| "*unprintable*".to_string());
        write!(f,"{}",out)
    }
}

impl StructTemplate {
    #[cfg(debug_assertions)]
    pub(super) fn format(&self) -> Result<String,StructError> {
        let mut output = String::new();
        let mut formatter = TemplateVarsFormatter::new();
        self.format_level(&mut formatter, &mut output)?;
        Ok(output)
    }

    #[cfg(debug_assertions)]
    fn format_level(&self, formatter: &mut TemplateVarsFormatter, output: &mut String) -> StructResult {
        match self {
            StructTemplate::Var(var) => {
                output.push_str(&format!("{}={:?}",formatter.get(&var.id),var.value));
            },
            StructTemplate::Const(val) => {
                output.push_str(&match val {
                    StructConst::Number(value) => format!("{:?}",value),
                    StructConst::String(value) => format!("{:?}",value),
                    StructConst::Boolean(value) => format!("{:?}",value),
                    StructConst::Null => format!("null")
                });
            },
            StructTemplate::Array(values) => {
                output.push_str("[");
                comma_separate(&values,|item,output| {
                    item.format_level(formatter,output)
                },output)?;
                output.push_str("]");
            },
            StructTemplate::Object(object) => {
                output.push_str("{");
                comma_separate(&object,|item,output| {
                    output.push_str(&format!("{:?}: ",item.0));
                    item.1.format_level(formatter,output)
                }, output)?;
                output.push_str("}");
            },
            StructTemplate::All(vars, expr) => {
                output.push_str(&format!("A{}.( ",vars.iter().map(|x| formatter.get(x)).collect::<Vec<_>>().join("")));
                expr.format_level(formatter,output)?;
                output.push_str(" )");
            },
            StructTemplate::Condition(var, expr) => {
                output.push_str(&format!("Q[{}={:?}] (",formatter.get(&var.id),var.value));
                expr.format_level(formatter,output)?;
                output.push_str(" )");
            }
        }
        Ok(())
    }    
}

#[cfg(debug_assertions)]
impl std::fmt::Debug for StructBuilt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let out = self.format().ok().unwrap_or_else(|| "*unprintable*".to_string());
        write!(f,"{}",out)
    }
}

impl StructBuilt {
    pub(super) fn format(&self) -> Result<String,StructError> {
        let mut output = String::new();
        self.format_level(&mut output)?;
        Ok(output)
    }

    pub(super) fn format_level(&self, output: &mut String) -> StructResult {
        match self {
            StructBuilt::Var(depth,width) => {
                output.push_str(&format!("D({},{})",depth,width));
            },
            StructBuilt::Const(val) => {
                output.push_str(&match val {
                    StructConst::Number(value) => format!("{:?}",value),
                    StructConst::String(value) => format!("{:?}",value),
                    StructConst::Boolean(value) => format!("{:?}",value),
                    StructConst::Null => format!("null")
                });
            },
            StructBuilt::Array(values,_) => {
                output.push_str("[");
                comma_separate(&values,|item,output| {
                    item.format_level(output)
                },output)?;
                output.push_str("]");
            },
            StructBuilt::Object(object) => {
                output.push_str("{");
                comma_separate(&object,|item,output| {
                    output.push_str(&format!("{:?}: ",item.0));
                    item.1.format_level(output)
                }, output)?;
                output.push_str("}");
            },
            StructBuilt::All(vars, expr) => {
                output.push_str(&format!("A[{}].( ",vars.iter().map(|x| format!("{:?}",x)).collect::<Vec<_>>().join("")));
                expr.format_level(output)?;                
                output.push_str(")");
            },
            StructBuilt::Condition(depth,width,expr) => {
                output.push_str(&format!("Q[{},{}].( ",depth,width));
                expr.format_level(output)?;                
                output.push_str(")");
            }
        }
        Ok(())
    }
}