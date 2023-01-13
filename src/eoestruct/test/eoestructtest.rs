
use std::{str::FromStr, collections::hash_map::DefaultHasher, hash::{Hash, Hasher}, fmt::Debug};
use crate::{eoestruct::{eoejson::{struct_to_json, struct_from_json, array_to_var, select_to_json }, structtemplate::{StructVar, StructPair}, StructTemplate, eoestructdata::{DataVisitor}, StructBuilt, structvalue::StructValue}, EachOrEvery};
use serde_json::{Value as JsonValue, Number, Map as JsonMap };

use super::super::eoestruct::{ StructConst, StructVarGroup, LateValues, StructVarValue };

fn json_fix_numbers(json: &JsonValue) -> JsonValue {
    match json {
        JsonValue::Null => JsonValue::Null,
        JsonValue::Bool(x) => JsonValue::Bool(*x),
        JsonValue::Number(n) => JsonValue::Number(Number::from_f64(n.as_f64().unwrap()).unwrap()),
        JsonValue::String(s) => JsonValue::String(s.to_string()),
        JsonValue::Array(x) => JsonValue::Array(x.iter().map(|x| json_fix_numbers(x)).collect()),
        JsonValue::Object(x) => JsonValue::Object(x.iter().map(|(k,v)| (k.to_string(),json_fix_numbers(v))).collect()),
    }
}

macro_rules! json_get {
    ($name:ident,$var:tt,$typ:ty) => {
        fn $name(value: &JsonValue) -> $typ {
            match value {
                JsonValue::$var(v) => v.clone(),
                _ => panic!("malformed test data")
            }
        }
                
    };
}

json_get!(json_array,Array,Vec<JsonValue>);
json_get!(json_string,String,String);
json_get!(json_object,Object,JsonMap<String,JsonValue>);

fn varval_hash(data: &StructVarValue) -> u64 {
    let mut state = DefaultHasher::new();
    data.hash(&mut state);
    state.finish()
}

fn to_str_vec<'a>(a: &'a Vec<String>) -> Vec<&'a str> {
    a.iter().map(|x| x.as_str()).collect()
}

fn build_json(vars: Vec<String>,ifs: Vec<String>, template: &JsonValue, late_data: Option<&JsonValue>) -> (StructTemplate,LateValues) {
    let late_data = late_data.map(|late_data | json_object(late_data));
    let (template,late_names) = struct_from_json(vars,ifs,&template).ok().unwrap();
    let mut lates = LateValues::new();
    let mut group = StructVarGroup::new();
    if let Some(late_data) = late_data {
        for (name,var) in late_names.iter() {
            let value =  array_to_var(&mut group,&json_array(late_data.get(name).unwrap())).ok().unwrap();
            lates.add(var,&value).ok().unwrap();
        }
    }
    (template,lates)
}

fn build_json_123(parts: &[JsonValue], from: usize, lates: Option<&JsonValue>) -> (StructTemplate,LateValues) {
    let vars = json_array_of_strings(&parts[1]);
    let ifs = json_array_of_strings(&parts[2]);
    build_json(vars,ifs,&parts[from],lates)
}

fn json_array_of_strings(data: &JsonValue) -> Vec<String> {
    json_array(data).iter().map(|x| json_string(x)).collect::<Vec<_>>()
}

fn debug_matches<X>(data: X, cmp: &JsonValue) where X: Debug {
    let debug = format!("{:?}",data);
    if !cmp.is_null() {
        assert_eq!(debug,json_string(cmp));
    }
}

/* A case comprises, in an array:
    *   0: a name for the case
    *   1: a set of strings to recognise as variables
    *   2: a set of strings to recognise as conditions
    *   3: a template (see struct_from_json() for details)
    *   4: an expected debug form of the template (or null if none specified)
    *   5: an expected output from expansion
    */
fn run_case(value: &JsonValue, rebuild: bool) {
    let parts = json_array(value);
    println!("running {}\n",json_string(&parts[0]));
    let (mut template,lates) = build_json_123(&parts,3,Some(&parts[6]));
    debug_matches(&template,&parts[4]);
    if rebuild {
        let built = template.build().ok().expect("unexpected error");
        template = built.unbuild().expect("unexpected error");
    }
    let output = struct_to_json(&template.build().ok().expect("unexpected error"),Some(&lates)).ok().unwrap();
    let output = JsonValue::from_str(&output.to_string()).ok().unwrap();
    assert_eq!(json_fix_numbers(&output),json_fix_numbers(&parts[5]));
}

/* A substitute case comprises, in an array:
    *   0: a name for the case
    *   1: a set of strings to recognise as variables
    *   2: a set of strings to recognise as conditions
    *   3: a template (see struct_from_json() for details)
    *   4: an expected debug form of the template before modification (or null if none specified)
    *   5: a path to a replacement
    *   6: the value of the replacement
    *   7: an expected debug form of the template after modification
    *   8: the expected expanded form
    * 
    * For an expected failure the array has no (8) and (7)) is the error
    */
fn run_substitute_case(value: &JsonValue) {
    let parts = json_array(value);
    println!("running {}\n",json_string(&parts[0]));
    let (template_pre,lates) = build_json_123(&parts,3,None);
    debug_matches(&template_pre,&parts[4]);
    let path = json_array_of_strings(&parts[5]);
    let path = to_str_vec(&path);
    let var = array_to_var(&mut StructVarGroup::new(),&json_array(&parts[6])).expect("error building value");
    if parts.len() == 9 {
        /* success */
        let template_post = template_pre.substitute(&path,var).expect("failed substitute");
        debug_matches(&template_post,&parts[7]);
        let output = struct_to_json(&template_post.build().ok().expect("unexpected error"),Some(&lates)).ok().unwrap();
        let output = JsonValue::from_str(&output.to_string()).ok().unwrap();
        assert_eq!(json_fix_numbers(&output),json_fix_numbers(&parts[8]));
    } else {
        /* failure */
        let err = template_pre.substitute(&path,var).err().expect("unexpceted success");
        assert_eq!(err,json_string(&parts[7]));
    }
}

/* A replace case comprises, in an array:
    *   0: a name for the case
    *   1: a set of strings to recognise as variables
    *   2: a set of strings to recognise as conditions
    *   3: a template (see struct_from_json() for details)
    *   4: an expected debug form of the pre-replacement template (or null if none specified)
    *   5: a path for replacement
    *   6: the replacement
    *   7: any path copies
    *   8: an expected debug form of the post-replacement template
    *   9: an expected output from expansion (or null if none specified)
    * 
    * if an error, (9) is missing and (8) is the error string
    */
fn run_replace_case(value: &JsonValue) {
    /* initial setup */
    let parts = json_array(value);
    println!("running replace {}\n",json_string(&parts[0]));
    let (template_pre,_) = build_json_123(&parts,3,None);
    debug_matches(&template_pre,&parts[4]);
    let built_pre = template_pre.build().ok().expect("unexpected error");
    /* do the replacement/substitution */
    let path = json_array_of_strings(&parts[5]);
    let path = to_str_vec(&path);
    /* replace */
    let template_pre = built_pre.unbuild().expect("error in unbuild");
    let replacement = build_json_123(&parts,6,None).0;
    let mut copies = vec![];
    for items in json_array(&parts[7]) {
        let items = json_array(&items);
        let src = json_array_of_strings(&items[0]);
        let dst = json_array_of_strings(&items[1]);
        copies.push((src,dst));
    }
    let copy_refs = copies.iter().map(|(a,b)| (to_str_vec(a),to_str_vec(b))).collect::<Vec<_>>();
    let copy_refs = copy_refs.iter().map(|(a,b)| (a.as_slice(),b.as_slice())).collect::<Vec<_>>();
    if parts.len() == 10 {
        /* success */
        let template_post = template_pre.replace(&path,replacement,&copy_refs).expect("error in replace");
        let built_post = template_post.build().ok().expect("unexpected error rebuilding");
        /* check result */
        debug_matches(&template_post,&parts[8]);
        let output = struct_to_json(&built_post,None).ok().unwrap();
        let output = JsonValue::from_str(&output.to_string()).ok().unwrap();
        assert_eq!(json_fix_numbers(&output),json_fix_numbers(&parts[9]));
    } else {
        /* failure */
        let err = template_pre.replace(&path,replacement,&copy_refs).err().expect("unexpected success");
        assert_eq!(err,parts[8]);
    }

}

/* An extract/extract_value case comprises, in an array:
    *   0: a name for the case
    *   1: a set of strings to recognise as variables
    *   2: a set of strings to recognise as conditions
    *   3: a template (see struct_from_json() for details)
    *   4: an expected debug form of the template (or null if none specified)
    *   5: a path for extraction
    *   6: extract: the expected debug form of the template after extraction (string)
    *      extract_value: the expected value (array)
    * 
    * When testing for failures the string (or first element in array in arg 6) is the
    * error string
    */
fn run_extract_case(value: &JsonValue, fail: bool) {
    let parts = json_array(value);
    println!("running {}\n",json_string(&parts[0]));
    let (template_pre,_) = build_json_123(&parts,3,None);
    debug_matches(&template_pre,&parts[4]);
    let path = json_array_of_strings(&parts[5]);
    let path = to_str_vec(&path);
    if let JsonValue::String(_) = &parts[6] {
        if fail {
            /* extract fail */
            let err = template_pre.extract(&path).err().expect("unexpected success");
            assert_eq!(err,json_string(&parts[6]));
        } else {
            /* extract */
            let template_post = template_pre.extract(&path).expect("extraction");
            debug_matches(&template_post,&parts[6]);
        }
    } else {
        if fail {
            /* extract_value fail */
            let err = template_pre.extract_value(&path).err().expect("extraction");
            assert_eq!(err,json_string(&json_array(&parts[6])[0]));
        } else {
            /* extract_value */
            let got = template_pre.extract_value(&path).expect("extraction");
            let expected = array_to_var(&mut StructVarGroup::new(), &json_array(&parts[6])).expect("parsing expected");
            assert_eq!(varval_hash(&expected.value),varval_hash(&got));
        }
    }
}

fn run_value_cases(value: &JsonValue) {
    let data = StructValue::new_json_value(&value[0]);
    for success in json_array(&value[1]) {
        let parts = json_array(&success);
        let path = json_array_of_strings(&parts[0]);
        let path = to_str_vec(&path);
        /* extract test */
        let got = data.extract(&path).expect("path failed");
        assert_eq!(json_fix_numbers(&got.to_json_value()),json_fix_numbers(&parts[1]));
        /* replace test */
        for repl in json_array(&value[3]) {
            let data = StructValue::new_json_value(&value[0]);
            let repl = StructValue::new_json_value(&repl);
            let modified = data.replace(&path, repl.clone(), &vec![]).expect("replace failed");
            let got = modified.extract(&path).expect("path failed");
            assert_eq!(&got,&repl);
            let data = StructValue::new_json_value(&value[0]);
            let repl = StructValue::new_number(23.1);
            let modified = data.replace(&path, repl.clone(), &vec![]).expect("replace failed");
            let got = modified.extract(&path).expect("path failed");
            assert_eq!(&got,&repl);
        }
    }
    for failure in json_array(&value[2]) {
        let path = json_array_of_strings(&failure);
        let path = to_str_vec(&path);
        assert!(data.extract(&path).is_err());
    }
    /* build test */
    let built = data.build();
    let built_json = struct_to_json(&built,None).expect("unbuildable");
    assert_eq!(json_fix_numbers(&value[0]),json_fix_numbers(&built_json));
    /* serialise test */
    let expect_str = serde_json::to_string(&json_fix_numbers(&value[0])).expect("unserialisable A");
    let got_str = serde_json::to_string(&data).expect("unserialisable B");
    assert_eq!(expect_str,got_str);
}

fn run_value_sort_cases(value: &JsonValue) {
    let parts = json_array(value);
    println!("ruuning {}\n",json_string(&parts[0]));
    let mut got = json_array(&parts[1]).iter().map(|x| StructValue::new_json_value(x)).collect::<Vec<_>>();
    got.sort();
    let expect = json_array(&parts[2]).iter().map(|x| StructValue::new_json_value(x)).collect::<Vec<_>>();
    for (got,expect) in got.iter().zip(expect.iter()) {
        assert_eq!(got,expect);
    }
}

fn run_case_buildfail(value: &JsonValue) {
    let parts = json_array(value);
    println!("ruuning {}\n",json_string(&parts[0]));
    let (template,_) = build_json_123(&parts,3,None);
    assert_eq!(template.build().err().expect("unexpected success"),json_string(&parts[4]));
}

fn run_case_expandfail(value: &JsonValue, rebuild: bool) {
    let parts = json_array(value);
    println!("ruuning {}\n",json_string(&parts[0]));
    let (mut template,lates) = build_json_123(&parts,3,Some(&parts[6]));
    debug_matches(&template,&parts[4]);
    if rebuild {
        let built = template.build().ok().expect("unexpected error");
        template = built.unbuild().expect("unexpected error");
    }
    let output = struct_to_json(&template.build().ok().expect("unexpected error"),Some(&lates));
    assert_eq!(output.err().expect("unexpected success"),json_string(&parts[5]));
}

fn run_case_parsefail(value: &JsonValue) {
    let parts = json_array(value);
    println!("ruuning {}\n",json_string(&parts[0]));
    let vars = json_array_of_strings(&parts[1]);
    let ifs = json_array_of_strings(&parts[2]);
    let output = struct_from_json(vars,ifs,&parts[3]);
    assert_eq!(output.err().expect("unexpected success"),json_string(&parts[4]));
}

macro_rules! run_cases {
    ($name:ident,$path:expr,$fn:ident) => {
        #[test]
        fn $name() {
            let data = JsonValue::from_str(include_str!($path)).ok().unwrap();
            for testcase in json_array(&data).iter() {
                $fn(&testcase);
            }
        }
    };

    ($name:ident,$path:expr,$fn:ident,$($extra:expr),*) => {
        #[test]
        fn $name() {
            let data = JsonValue::from_str(include_str!($path)).ok().unwrap();
            for testcase in json_array(&data).iter() {
                $fn(&testcase,$($extra),*);
            }
        }
    };
}

run_cases!(test_smoke,"test-eoe-smoke.json",run_case,false);
run_cases!(test_replace_smoke,"test-eoe-replace.json",run_replace_case);
run_cases!(test_substitute_smoke,"test-eoe-substitute.json",run_substitute_case);
run_cases!(test_extract,"test-eoe-extract.json",run_extract_case,false);
run_cases!(test_extract_fail,"test-eoe-extract-fail.json",run_extract_case,true);
run_cases!(test_rebuild_smoke,"test-eoe-smoke.json",run_case,true);
run_cases!(test_buildfail,"test-eoe-buildfail.json",run_case_buildfail);
run_cases!(test_expandfail,"test-eoe-expandfail.json",run_case_expandfail,false);
run_cases!(test_rebuild_expandfail,"test-eoe-expandfail.json",run_case_expandfail,true);
run_cases!(test_parsefail,"test-eoe-parsefail.json",run_case_parsefail);
run_cases!(test_visitor,"test-visitor.json",visitor_case);
run_cases!(test_select,"test-select.json",select_case);
run_cases!(test_value_smoke,"test-eoe-value.json",run_value_cases);
run_cases!(test_value_ordering_smoke,"test-eoe-value-ordering.json",run_value_sort_cases);
run_cases!(test_top_replace,"test-eoe-top-replace.json",top_replace_case);

#[test]
fn test_eoestruct_free() {
    /* corner case not testable with the available harnesses */
    let mut group = StructVarGroup::new();
    let template = StructTemplate::new_array(vec![
        StructTemplate::new_boolean(true),
        StructTemplate::new_var(&StructVar::new_boolean(&mut group,EachOrEvery::each(vec![false,true])))
    ]);
    assert_eq!(template.build().err().expect("unexpected success"),"free variable in template");
}

#[test]
fn test_eoestruct_every() {
    let mut group = StructVarGroup::new();
    let every = StructVar::new_boolean(&mut group,EachOrEvery::every(false));
    let each = StructVar::new_number(&mut group,EachOrEvery::each(vec![1.,2.]));
    let template = StructTemplate::new_all(&mut group,
    StructTemplate::new_array(vec![
        StructTemplate::new_boolean(true),
        StructTemplate::new_var(&every),
        StructTemplate::new_var(&each)
    ]));
    let debug = format!("{:?}",template);
    assert_eq!("Aab.( [true,false,b=<1.0,2.0>] )",debug);
    let output = struct_to_json(&template.build().ok().expect("unexpected error"),None).ok().unwrap();
    let wanted = JsonValue::from_str("[[true,false,1],[true,false,2]]").ok().unwrap();
    assert_eq!(&json_fix_numbers(&wanted),&json_fix_numbers(&output));
}

#[test]
fn test_late_infinite_array() {
    let mut group = StructVarGroup::new();
    let late = StructVar::new_late(&mut group);
    let infinite = StructVar::new_number(&mut group,EachOrEvery::every(77.));
    let template = 
        StructTemplate::new_all(&mut group,
            StructTemplate::new_object(vec![
                StructPair::new("a",StructTemplate::new_number(42.)),
                StructPair::new("b",StructTemplate::new_var(&late))
            ]));
    let mut lates = LateValues::new();
    lates.add(&late,&infinite).ok().unwrap();
    let output = struct_to_json(&template.build().ok().expect("unexpected error"),Some(&lates));
    assert_eq!(output.err().expect("unexpected success"),"no infinite recursion allowed");
}

#[test]
fn test_infinite_all() {
    let mut group = StructVarGroup::new();
    let late = StructVar::new_late(&mut group);
    let template = StructTemplate::new_all(&mut group,
        StructTemplate::new_var(&late)
    );
    let mut lates = LateValues::new();
    lates.add(&late, &StructVar::new_boolean(&mut group,EachOrEvery::every(false))).ok().unwrap();
    let output = struct_to_json(&template.build().ok().expect("unexpected error"),Some(&lates)).err().unwrap();
    assert_eq!("no infinite recursion allowed",output);
}

#[test]
fn test_eoe_smoke_array() {
    let pattern = vec![0,1,2,3,1,2,3,1,2,1];
    let start = pattern.clone();
    let options = vec![
        StructTemplate::new_number(0.),
        StructTemplate::new_string("1".to_string()),
        StructTemplate::new_boolean(true),
        StructTemplate::new_null(),
    ];
    let output_options = vec![
        JsonValue::Number(Number::from_f64(0.).unwrap()),
        JsonValue::String("1".to_string()),
        JsonValue::Bool(true),
        JsonValue::Null
    ];
    let cmp = JsonValue::Array(
        pattern.iter().map(|x| output_options[*x].clone()).collect::<Vec<_>>()
    );
    let template = StructTemplate::new_array(start.iter().map(|x| { options[*x].clone() }).collect());
    let output = struct_to_json(&template.build().ok().expect("unexpected error"),None).ok().unwrap();
    assert_eq!(json_fix_numbers(&output),json_fix_numbers(&cmp));
}

#[test]
fn test_eoestruct_notopcond() {
    let mut group = StructVarGroup::new();
    let template = StructTemplate::new_condition(StructVar::new_boolean(&mut group,EachOrEvery::each(vec![true])),
        StructTemplate::new_number(42.)
    );
    assert_eq!(template.build().err().expect("unexpected success"),"conditionals banned at top level");
}

#[test]
fn test_bind_late_to_late() {
    let mut lates = LateValues::new();
    let mut group = StructVarGroup::new();
    let late1 = StructVar::new_late(&mut group);
    let late2 = StructVar::new_late(&mut group);
    assert_eq!("cannot bind late variables to late variables",lates.add(&late1,&late2).err().unwrap());
}

#[test]
fn test_bind_to_early() {
    let mut lates = LateValues::new();
    let mut group = StructVarGroup::new();
    let early = StructVar::new_boolean(&mut group,EachOrEvery::every(false));
    let late = StructVar::new_late(&mut group);
    assert_eq!("can only bind to late variables",lates.add(&early,&late).err().unwrap());
}

#[test]
fn test_missing_late() {
    let mut group = StructVarGroup::new();
    let template = StructTemplate::new_array(vec![
        StructTemplate::new_var(&StructVar::new_late(&mut group))
    ]);
    assert_eq!(template.build().err().expect("unexpected success"),"free variable in template");
}

struct TestVisitor(String);

impl DataVisitor for TestVisitor {
    fn visit_const(&mut self, _input: &StructConst) -> Result<(),String> { self.0.push('c'); Ok(()) }
    fn visit_separator(&mut self) -> Result<(),String> { self.0.push(','); Ok(())}
    fn visit_array_start(&mut self) -> Result<(),String> { self.0.push('['); Ok(()) }
    fn visit_array_end(&mut self) -> Result<(),String> { self.0.push(']'); Ok(()) }
    fn visit_object_start(&mut self) -> Result<(),String> { self.0.push('{'); Ok(()) }
    fn visit_object_end(&mut self) -> Result<(),String> { self.0.push('}'); Ok(()) }
    fn visit_pair_start(&mut self, key: &str) -> Result<(),String> { self.0.push_str(&format!("<{}>",key)); Ok(()) }
    fn visit_pair_end(&mut self, key: &str) -> Result<(),String> { self.0.push_str(&format!("</{}>",key)); Ok(()) }
}

fn top_replace_case(value: &JsonValue) {
    let parts = json_array(value);
    println!("ruuning {}\n",json_string(&parts[0]));
    let (template,_) = build_json_123(&parts,3,None);
    let new_template = template.set_index(&[],2).expect("set failed");
    eprintln!("{:?}",new_template);
    assert_eq!(json_string(&parts[4]),format!("{:?}",new_template));
}

fn visitor_case(value: &JsonValue) {
    let parts = json_array(value);
    println!("ruuning {}\n",json_string(&parts[0]));
    let (template,lates) = build_json_123(&parts,3,None);
    debug_matches(&template,&parts[4]);
    let mut visitor = TestVisitor(String::new());
    template.build().ok().expect("unexpected error").expand(Some(&lates),&mut visitor).ok().expect("visitor failed");
    assert_eq!(&parts[5],&visitor.0)
}

fn json_number_or_null(value: &JsonValue) -> Option<f64> {
    match value {
        JsonValue::Number(n) => { n.as_f64() },
        _ => { None }
    }
}

fn select_subcase(data: &StructBuilt, path: &[String], values: &[Option<f64>]) {
    let output = json_fix_numbers(&select_to_json(data, path,None).expect("bad select"));
    let output = json_array(&output).iter().map(|x| json_number_or_null(x)).collect::<Vec<_>>();
    assert_eq!(output,values);
}

fn select_case(value: &JsonValue) {
    let parts = json_array(value);
    println!("running {}",json_string(&parts[0]));
    let (template,lates) = build_json_123(&parts,3,None);
    let build = template.build().ok().expect("unexpected error");
    let output = struct_to_json(&build,Some(&lates)).ok().unwrap();
    let output = JsonValue::from_str(&output.to_string()).ok().unwrap();
    assert_eq!(json_fix_numbers(&output),json_fix_numbers(&parts[4]));
    for subtests in json_array(&parts[5]) {
        let parts = json_array(&subtests);
        let path = json_array_of_strings(&parts[0]);
        let values = json_array(&json_fix_numbers(&parts[1])).iter().map(|x| json_number_or_null(x)).collect::<Vec<_>>();
        select_subcase(&build,&path,&values);
    }
}
