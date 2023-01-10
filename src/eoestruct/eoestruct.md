# EoEStruct

## Overview

The fundamental structure for data in the genome browser is the "EachOrEvery" (EoE). Every EachOrEvery is one of two things:

1. a finite sequence of values, resembling an array ("each")
2. a conceptually-infinitely long repeating sequence of a single value ("every").

These cannot be nested, and so are flat-sequence data-structures representing, say, a list of start co-ordinates, biotypes, colours, etc. A group of EoEs can be iterated through together. The iteration terminates when the finite EoEs end (of which there must be at least one, and all must be the same length).

The question arises as to map this to more conventional data-structures such as represented in JSON, transforming in "both directions", ie:

1. how do we take a more conventional data source and read and manipulate it as EoEs?
2. how do we combine EoEs and use them to create a more conventional data source?

The answer is: with EoEStruct.

EoEStruct is well covered by very comprehensive unit tests.

The most important type in EoEStruct is `StructBuilt`. This compactly represents conventionally-structured data. This document is divided into three parts of increasing complexity:

1. Getting EoEs out of conventional data
2. Generating more conventional datafrom EoEs
3. (Limited) modification of EoEStructs.

EoEStruct also implements StructValue, a much simpler data-structure representing a JSON "constant", ie an arbitrary json data-structure which doesn't use EoEs. This resembles json_serde::Value, but has some nice properties like efficient copies by using Arcs internally and ordering and equality and areserializable via serde (not only into JSON!).

## StructValue Basics

When all you want to do is represent some JSON data without reference to EoEs, use StructValues. There are constructors and a way of going forward and backward to the serde form.

```
    fn new_number(input: f64) -> StructValue;
    fn new_string(input: String) -> StructValue;
    fn new_boolean(input: bool) -> StructValue;
    fn new_null() -> StructValue;
    fn new_array(input: Vec<StructValue>) -> StructValue;
    fn new_object(mut input: Vec<(String,StructValue)>) -> StructValue;
    fn new_expand(input: &StructBuilt, lates: Option<&LateValues>) -> Result<StructValue,StructError>;
    fn new_json_value(value: &JsonValue) -> StructValue;
    fn to_json_value(&self) -> JsonValue;
```

As well as implementing Serialize, Deseriailize, PartialOrd, Ord, PartialEq, Eq, and Hash you can also extract and replace parts of values just like with StructTemplates. See the relevant sections later in this document. You can convert StructValues into StructBuilts for the use of other operations described in this document. Unlike with StructTemplate, this is guaranteed to succeed.

```
    pub fn build(&self) -> StructBuilt;
```

## Getting EoEs out of conventional data

Sometimes you have some conventional data (in a `StructBuilt`) and you want to extract some EoEs for inside your style program.

To extract such data you need to specify the path (an array of strings describing which bit of the data you want) and pass it to `struct_select`. You will get a vector of optional `StructConst`s out. A `StructConst` is one of the leaf types in the JSON data-model (number, boolean, string, null). It is optional because it may be missing for any given object. For example, you may have a structure comprising an array of objects and the key you want simply not be present in some of those objects. You can then (hopefully!) build this result into your EoE.

What remains is the syntax for a path.

### Path syntax

A path is an array of strings. Those strings can be

1. a string representing a key in an object;
2. a stringified integer representing the index in an array;
3. `*` representing all instances in an array.

For example, if you want all the start co-ordinates, `[w0,w1,...]` from a structure like this:

```
[
    { "start": w0, "end", x0, "height": y0, "colour": z0 },
    { "start": w1, "end", x1, "height": y1, "colour": z1 }
]
```

then the path is `["*","start"]`.

But for the end-coordinate `[x0,x1,...]` of data like this:

```
{
    "objects": [
        { "range": [w0,x0] },
        { "range": [w1,x1] },
        { "range": [w2,x2] },
        { "range": [w3,x3] },
        { "range": [w4,x4] }
    ]
}
```

then the path is `["objects","*","1"]`.

That's all there is to it! (For late values, see the later section in this document).

## Generating more conventional datafrom EoEs

Sometimes you have some EoEs and want conventional data to serialise. `StructBuilt`s can be serialised into json, but how do you get one for your data and template? You build a `StructTemplate` and then call `build()` on it. This is much more fiddly than the other way around because you need to specify how you want the data strcutured.

Say you have a bunch of EoEs representing some data, for example, one contains start coordinates, one end coordinates, one height, one colours, and so on, and you with to create some conventional data-structure for reporting, with the schema below:

```
[
    { "start": w0, "end", x0, "height": y0, "colour": z0 },
    { "start": w1, "end", x1, "height": y1, "colour": z1 }
]
```

where `w, x, y, z` etc are EoEs. To do this, you use a `StructTemplate`.

A StructTemplate is a tree which works a bit like a schema for this data. You build leaf StructTemplates out of constants and EoEs, and combine them to build your single template. You can then, for example, serialize this finished StructTemplate and you will get your nicely formatted JSON out. The data remains as compact as it is in the EoEs: nowhere is it expanded out into a very long string nor to giant data structure.

This is directly analagous to the way you would build such an object programatically in any other language, first out of your constants and variables, later built into arrays, objects etc, and when finished emit it. However, EoEStruct has some special types to make things more useful in our context, without looping and creating messy intermediate objects internally. But it is basically the same process.

### Constant StructTemplates

The simplest StructTemplate is a constant. This always emits that exact constant. You can work out what these do by yourself!

```
    pub fn new_number(input: f64) -> StructTemplate;
    pub fn new_string(input: String) -> StructTemplate;
    pub fn new_boolean(input: bool) -> StructTemplate;
    pub fn new_null() -> StructTemplate;
```

### Adding variables

Chances are your data-structure isn't a constant, so you'll need to introduce some variables, in the form of EoEs. To do this you need a `StructVarGroup`. You can just create these whenever you wish:

```
    pub fn new() -> StructVarGroup;
```

All the EoEs in a StructVarGroup are iterated through together (for example, you'd have a single group for your start and end co-ordinates, biotypes, etc).

Once you have a StructVarGroup, you can start putting each of your EoEs into `StructVar`s.

```
    pub fn new_number(group:&mut StructVarGroup, input: EachOrEvery<f64>) -> StructVar;
    pub fn new_string(group:&mut StructVarGroup, input: EachOrEvery<String>) -> StructVar;
    pub fn new_boolean(group:&mut StructVarGroup, input: EachOrEvery<bool>) -> StructVar;
```

### Iterating through groups of EoEs

Now you've created a `StructVarGroup` with some values in it, there will be a point in your template where you want one entry per element of the EoEs in that group. For example, in our motivating example of the array of objects, this will be the array at the very top level of the template (but need not be, in general).

```
    pub fn new_all(vars: &mut StructVarGroup, expr: StructTemplate) -> StructTemplate;
```

`expr` is the sub-template for each element.  If you like, this template node works a bit like a "for" loop.

At the moment we don't have any way of accessing the values of our EoE in that sub-template, which we certainly need for it to be useful! Do it like this:

```
    pub fn new_var(input: &StructVar) -> StructTemplate
```

Of course, this template must be inside a relevant `new_all` for that variable.

### Simple Example

At least we're at the stage where we can illustrate a very simle realistic use case. Say you have a single EoE called `a` containing the values `[1,2,3,4,5]`. And you just want to serialise it as an array, just like that.

```
  let group = StructVarGroup::new();            // create group
  let var_a = StructVar::new_number(group,a);   // add a to group

  let element = StructTemplate::new_var(var_a); // each element of the array is just our value
  let template = StructTemplate::new_all(group,element); // create array by repeating element
```

### Adding objects (ie "maps", "dicts") to templates

You can add objects to a StructTemplate. Such objects are made out of pairs of keys and values. The key must be a constant string, but the value can be any StructTemplate. 

```
    pub fn new(key: &str, value: StructTemplate) -> StructPair;
```

An EoE is created of these pairs to make an object StructTemplate.

```
    pub fn new_object(input: Vec<StructPair>) -> StructTemplate;
```

### More realistic example

We're now in a position to create a more realistic camples. Say you have two EoEs called `start` and `end`, of the same length and you want to put them into a structure of the form

```
  [ { "start": s0, "end": e0 }, { "start": s1, "end": e1 }, { "start": s2, "end": e2 }, ... ]
```

You could do it like this:

```
  let group = StructVarGroup::new();
  let var_start = StructVar::new_number(group,start);
  let var_end = StructVar::new_number(group,end);

  let element = StructTemplate::new_object(vec![
      StructPair::new("start",StructTemplate::new_var(var_start)),
      StructPair::new("end",StructTemplate::new_var(var_end))
  ]);
  let template = StructTemplate::new_all(group,element);
```

### Adding fixed arrays

Sometimes you want to add arrays of known length, working as tuples, rather than objects. Note that this is fundamentally a different case from the more common `new_all` described above, where the array iterates through your EoEs. To continue the previous example, rather than the serialisation described there, you might want it in the form.

```
  [ { "range": [s0,e0] }, { "range": [s1,e1] }, { "range": [s2,e2] }, ... ]
```

For which `new_array` is provided and which you can do like this:

```
  let group = StructVarGroup::new();
  let var_start = StructVar::new_number(group,start);
  let var_end = StructVar::new_number(group,end);

  let range_tuple = StructTemplate::new_array(vec![
      StructTemplate::new_var(var_start),
      StructTemplate::new_var(var_end)
  ]);
  let element = StructTemplate::new_objectvec![
      StructPair::new("range",range_tuple)
  ]);
  let template = StructTemplate::new_all(group,element);
```

### Advanced: Conditions

Templates can be wrapped in a condition template, which also takes a variable (as a `StructVar`). As the group is iterated through, if the variable is truthy then the condition has no effect and the subtemplate is rendered; otherwise it is as if the subtemplate isn't there at all. Conditions can only go inside arrays and pair values (for objects), wherein a falsy value means that the element and pair are skipped (repsectively) as the array/object is built.

For example, it could be that you are building our standard "array of objects" structure and there's a key in the object called "protein" with an id value from the EoE `protein` which should only be present when another EoE, `protein_present`, is true, otherwise the key be missing entirely.

```
  let group = StructVarGroup::new();
  let var_protein = StructVar::new_number(group,protein);
  let var_protein_present = StructVar::new_number(group,protein_present);
  ...
  let element = StructTemplate::new_object(EachOrEvery::each([
      ...
      StructPair::new("protein",StructTemplate::condition(
          var_protein_present,
          StructTemplate::new_var(var_protein)
      )
  ]));
  let template = StructTemplate::new_all(group,element);
```

### Advanced: late binding

Sometimes you want to add an EoE *after* a template has been generated, it's just not known at the time of template generation. These are known as "late" bindings and are passed at serialisation time. A placeholder "late" variable is passed when the template is generated and then when it is serialised, a `LateValues` object is passed containing the values of any "late" variables.

Template generation takes a non-trivial amount of time, so it makes sense not to do it too often, but "late" variables are a little slower and more awkward than regular variables if there is no need to use them.

```
    pub fn new_late(group:&mut StructVarGroup) -> StructVar
    pub fn new() -> LateValues
    pub fn add(&mut self, var: &StructVar, val: &StructVar) -> StructResult // in LateValues
```

### Actually serialising

The actual method to serialise is `eoestack_run` but you will usually use a convenience function for the serialisation format of your choice, for exmaple `struct_to_json`.

To use these you will need a `StructBuilt` not your `StructTemplate`. Use `StructTemplate::build()` to acheive create it.

### Advanced: Building templates from JSON

Building templates programatically can be painful, so the library a way of specifying them in JSON. This isn't as useful as it seems, as it turns out building templates is usually best hardwired, but is useful for unit-tests etc and probably has further niche cases.

Building templates uses `struct_from_json` which takes a list of "all" strings, a list of "if" strings, and a JSON template. The "all" and "if" strings are strings with special meaning in the template, and the list could come from the same source as the template or be hardwired in the call, as appropriate for the use case.

Every type in the supplied json template is copied across verbatim to the `StructTemplate` generated except for objects and strings, and even objects and strings are copied across verbatim *unless* they meet certainconditions:
* Objects are only special if they conatain a key in the "all" or "if" list supplied. 
* Strings are only special if the match a variable name established in an All (see below)

If the object contains a key in the "all" list then an `All` is generated in the template. The matching key is taken to be the sub-expression of the `All` and all other keys the names of variables with the given value. For example, if `!` is in the Alls list, then

```
{
    "!": ["var","z"],
    "var": ["a","b","c"]
}
```

Will expand to `[["a","z"],["b","z"],["c","z"]]`. The `!` in the outer object established it as an `All` node and that meant the other key `var` was made a variable with contents `["a","b","c"]`. The inner template was set to `["var","z"]` where the `"var"` was mapped to the newly-established variable and the `"z"`, not matching any variable became a literal `"z"`. A more realistic example might be (again with `!` as our all string):

```
{
    "!": {
        "start": "start",
        "end": "end",
        "colour": "blue"
    },
    "start": [1000000,1100000,1200000],
    "end": [1000100,1100100,1200100]
}
```

If an object has no keys matching an all, it is tested as to whether it matches an if. If it does then a `Condition` node is created at this point. The if key used should also, by this point in the tree, have been bound to a variable by an `All` node. The contents of the key are used as the subtemplate and the value of the key as a variable used as the condition. This is probably best explained by example. Let's extend the above example. Say we have an EoE of booleans and when that boolean is true want to have the key `"very_special": true` added to our object, otherwise no key being present. Let's call this array `&special`. We would add the string `"&special"` to the ifs list and modify our template above to say:

```
{
    "!": {
        "start": "start",
        "end": "end",
        "colour": "blue",
        "&special": { "very_special": true }
    },
    "start": [1000000,1100000,1200000],
    "end": [1000100,1100100,1200100],
    "&special": [true,false,false]
}
```

Note that we use `&special` as a variable in the outer all and then use that same string within its template (the `!` key) to establish the condition node. We get:

```
[
    { "start": 1000000, "end": 1000100, "very_special": true },
    { "start": 1100000, "end": 1100100 },
    { "start": 1200000, "end": 1200100 }
]
```

## (Limited) modification of EoEStructs

StructTemplates can be modified with replace(). This takes a path to a part of the template and a replacement value. Note that a "path" here is a little different here to the general seletion case from `StructBuilt`, as you are modifying the whole template, without reference to the data. For example, when selecting from an `All` element, it makes sense to talk about the 1st, 2nd, etc element. When accessing or modifying the template itself, this is meaningless.

* All nodes *must* have `*` in the path.
* Array nodes *must* have an integer in the path.
* Condition nodes must have a `&` in the path to go into their contents.
* Condition nodes always count in an array index.

```
    fn replace(&self, path: &[&str], value: StructTemplate, copy: &[(&[&str],&[&str])]) -> Result<StructTemplate,StructError>

```

In some rare cases you may wish to refer to parts of the old sub-template in the replacement (especially its variables). For example, you might emit variables A and B as [A,B] and want to replace it with [B,A] without replacing any of the machinery higher up in the tree which enumerates through A and B. This effectively means "copying" some nodes from one template to another. To do this you pass a list of "path pairs" mapping the path to the variable in the replaced template to the path in the new. The old path is rooted at the entire template level (and so can actually validly refer to variables beyond the replaced part) whereas the target path is relative to the replacement part only. If you don't know that you need this, the `copy` argument can be blznk.

You can extract parts of a template using a path with `extract` and `extract_value`.

```
    fn extract(&self, path: &[&str]) -> Result<StructTemplate,StructError>
    fn extract_value(&self, path: &[&str]) -> Result<StructVarValue,StructError>
```

You can also replace the EoE value used in a `Var` or a `Condition` using the method:

```
    fn substitute(&self, path: &[&str], value: StructVar) -> Result<StructTemplate,StructError>
```

Note that the group of the replacement is ignored, only the value is copied, using the group of the old `Var` or `Condition`node; indeed, this is the value of this method. If you try to use `replace()` with free variables they will be of the wrong group and so the template won't build, so use `substitute()` instead.

It may seem a litte mysterious as to why you would want to do any of this. Note, however that `StructBuilt`s are immutable objects and are often used for storing arbitrary json-modelled values. If you *do* want to modify them, you can convert them *back* to a `StructTemplate`, replace somde contents and then rebuild. You "unbuild" with this method.

```
    fn unbuild(&self) -> Result<StructTemplate,StructError>
```

But essentially any need for modification (rather than selecting andremaking) should be seen as an unfortunate hack, as it requires too much knowledge of how the `StructBuilt` was made.
