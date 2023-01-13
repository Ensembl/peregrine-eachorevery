use super::{eoestruct::{StructConst, LateValues}, structbuilt::StructBuilt };

pub trait DataVisitor {
    fn visit_const(&mut self, _input: &StructConst) -> Result<(),String> { Ok(()) }
    fn visit_separator(&mut self) -> Result<(),String> { Ok(()) }
    fn visit_array_start(&mut self) -> Result<(),String> { Ok(()) }
    fn visit_array_end(&mut self) -> Result<(),String> { Ok(()) }
    fn visit_object_start(&mut self) -> Result<(),String> { Ok(()) }
    fn visit_object_end(&mut self) -> Result<(),String> { Ok(()) }
    fn visit_pair_start(&mut self, _key: &str) -> Result<(),String> { Ok(()) }
    fn visit_pair_end(&mut self, _key: &str) -> Result<(),String> { Ok(()) }
}

pub trait DataStackTransformer<T,X> {
    fn make_singleton(&mut self, value: T) -> X;
    fn make_array(&mut self, value: Vec<X>) -> X;
    fn make_object(&mut self, value: Vec<(String,X)>) -> X;
}

enum DataStackEntry<X> {
    Node(Option<X>),
    Array(Vec<X>),
    Object(Vec<(String,X)>)
}

struct DataStack<T,X> {
    stack: Vec<DataStackEntry<X>>,
    keys: Vec<String>,
    transformer: Box<dyn DataStackTransformer<T,X>>
}

impl<T,X> DataStack<T,X> {
    fn new<F>(transformer: F) -> DataStack<T,X> where F: DataStackTransformer<T,X> + 'static {
        DataStack {
            stack: vec![DataStackEntry::Node(None)],
            keys: vec![],
            transformer: Box::new(transformer)
        }
    }

    fn get(mut self) -> X {
        if let Some(DataStackEntry::Node(Some(n))) = self.stack.pop() {
            n
        } else {
            panic!("inocorrect stack size at completion"); // we require this ofcallers
        }
    }

    fn push_array(&mut self) {
        self.stack.push(DataStackEntry::Array(vec![]));
    }

    fn push_object(&mut self) {
        self.stack.push(DataStackEntry::Object(vec![]));
    }

    fn push_key(&mut self, key: &str) {
        self.keys.push(key.to_string());
    }

    fn add(&mut self, item: X) {
        match self.stack.last_mut().unwrap() { // guranteed by visitor invariant/caller
            DataStackEntry::Array(entries) => {
                entries.push(item);
            },
            DataStackEntry::Object(entries) => {
                let key = self.keys.pop().unwrap(); // guranteed by visitor invariant
                entries.push((key,item));
            },
            DataStackEntry::Node(value) => {
                *value = Some(item);
            }
        }
    }

    fn add_atom(&mut self, item: T) -> Result<(),String> {
        let item = self.transformer.make_singleton(item);
        self.add(item);
        Ok(())
    }

    fn pop<F>(&mut self, cb: F) -> Result<(),String> where F: FnOnce(X) -> Result<X,String> {
        match self.stack.pop().expect("struct invariant violated: build stack musused and underflowed") {
            DataStackEntry::Array(entries) => {
                let item = cb(self.transformer.make_array(entries))?;
                self.add(item);
            },
            DataStackEntry::Object(entries) => {
                let item = cb(self.transformer.make_object(entries))?;
                self.add(item);
            },
            DataStackEntry::Node(node) => {
                self.add(cb(node.expect("unset"))?);
            }
        }
        Ok(())
    }
}

impl<X> DataVisitor for DataStack<StructConst,X> {
    fn visit_const(&mut self, input: &StructConst) -> Result<(),String> { self.add_atom(input.clone()) }
    fn visit_separator(&mut self) -> Result<(),String> { Ok(()) }
    fn visit_array_start(&mut self) -> Result<(),String> { self.push_array(); Ok(()) }
    fn visit_array_end(&mut self) -> Result<(),String> { self.pop(|x| Ok(x)) }
    fn visit_object_start(&mut self) -> Result<(),String> { self.push_object(); Ok(()) }
    fn visit_object_end(&mut self) -> Result<(),String> { self.pop(|x| Ok(x)) }
    fn visit_pair_start(&mut self, key: &str) -> Result<(),String> { self.push_key(key); Ok(()) }
    fn visit_pair_end(&mut self, _key: &str) -> Result<(),String> { Ok(()) }
}

pub fn eoestack_run<F,X>(input: &StructBuilt, lates: Option<&LateValues>, transformer: F) -> Result<X,String> where F: DataStackTransformer<StructConst,X> + 'static {
    let mut stack = DataStack::new(transformer);
    input.expand(lates,&mut stack)?;
    Ok(stack.get())
}
