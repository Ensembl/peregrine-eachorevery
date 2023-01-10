# Inside EoEStruct

The insides of EoEStruct are scary. The representation and algorithms are non-trivial to support the use cases.

To ensure correctness, there is an extensive set of unit tests inside `test/eoestructtest.rs`. This includes a compact, string-based representation for creating test StructTemplates and various methods for running test cases (depending on expected success or failure) driven by json files (`run_*()`). Most test cases are checked by these `.json` files and relatively few rust test methods. A few further rust test methods capture special checks not easily represented as data.

If you modify eoestruct you *must* run and, if necessary, add new unit tests or test cases. There is very little chance of any code being correct first time.

## The Files

Core definitions

* `eoestruct.rs` -- miscellaneous, short core definitions
* `structtemplate.rs` -- basic definitions of `StructTemplate`, `StructVar`, and `StructPair`
* `structbuilt.rs` -- basic definition of `StructBuilt`
* `structvalue/rs` -- basic definition of `StructValue` and the various traits it implements.

Core algorithms

* `build.rs` -- algorithm behind `StructTemplate::build()` and `StructBuilt::unbuild()`
* `expand.rs` -- the two algorithms behind `StructBuilt::select()`, and `StructBuilt::expand()`
* `eoestructdata.rs` -- `eoestack_run`, basic controller for all codecs
* `replace.rs` -- the algorithms behind `replace()`, `extract()` for `StructTemplate` and `StructValue`, `StructTemplate::extract_value()` and `StructTemplate::substitute()`

Codecs

* `eoejson.rs` -- json
* `eoetruthy.rs` -- `StructBuilt::truthy()` is actually a codec converting to a bool!

Debugging

* `eoedebug.rs` -- debug trait implementations for `StructTemplate` and `StructBuilt`

Tests

* `test/eoestructtest.rs` -- all test cases and support functions

## The structure of `StructTemplate`

`StructTemplate` has a simple, direct representation as a tree corresponding very closely to the calls used to construct it and very little processing is done to the arguments. See `structtemplate.rs`

## The representaion of variables

Variables are represented by the externally visible `StructVar`. These are little-used internally, instead its wto fields, its `StructValueId` and `StructVarValue` are used.

`StructValueId` is simply a unique object used for identification. `StructVarValue` is an emum containing one of the permitted EoE variable values, or an identifier for late bindings.

During the build process, `StructValueId`s are done away with (only being used for matching between Alls and Vars) and `StructVarValue` is stored in the relevant `StructBuilt`.

## The structure of `StructBuilt`

`StructBuilt` heavily resembles `StructTemplate`: it is a tree with a one-to-one correspondence to nodes in the template which generated it. However, some of the nodes are heavily processed.

The `Const`, `Array` and `Object` arms are essentially identical with minor tweaks. However, `Var`, `All` and `Condition` differ in the way variables are repesented.

Rather than represent variables symbolically (which would be too slow at render time), `Var` and `Condition` represent them by a pair of integers. These represent, respectively, *which* all statement the variable came from, and the *index* of the variable within that all statement.

The `All` is indexed starting at zero from the root down through to the current position: for example, the outermost All is `0`, the one inside it `1` etc. Note this means that in branches of the tree governed by a different All, an index may be reused. A good way of understanding this is to consider the expansion algorithm which operates as a downward traversal from the root and has a stack for Alls. When it encounters an All "going down" it is pushed onto the All stack, when it later leaves that node "going up" it is popped. The index of the All on the stack is the number used to identify the All.

## The build algorithm

The build algorithm is at heart quite straight-forward, transforming the `StructTemplate` tree into the `StructBuilt` tree, to which it corresponds node-for-node, making the variable index transformations described in the previous section. Variables are moved from their place in the template tree, their point of use, to the point at which they are recursed across.

There's also: 

* a lot of error checking; 

* checking bindings at all nodes are actually used underneathand removing them if not, important both for corretness and speed at expansion time;

* derivation of a flag put on array nodes to indicate there are no condition nodes "below" which greatly speeds up processing of such a node (the usual case) at expand time.

One slight oddity of the build algorithm is in the handling of "bindings" in the `All` arm. `bindings` is an array of type `Binding` which maps the `StructValueId` to an offset. Initially during build, the `All` node doesn't know the value to use, just an id. So it pushes on as many `Binding` objects as necessary, with empty values, and then recurses. Relevant `Var` arms fill in the values. Immediately after return, these bindings objects are then removed from the array. *Importantly*, this code knows that its objects must be the last ones in the array as *only it* creates and removes Binding objects). Without this insight the bindings array manipulation can look very odd.

## The expand algorithm

The expand algorithm, in `expand.rs`, is responsible for driving serializers. It uses a visitor model (`DataVisitor`) to abstract away serialisation details. A `GlobalState` object is created, containing the late bindings and an (initiall empty) stack for All nodes. All objects are not stored on this stack directly, but inside an AllState object which contains not only the value but also `next_index`, a cursor enumerating the expansion at the next index to use, and `first` the position of the first EoE among those inside All which is finite.

Then `split` is called, which recurses through the tree from the root. The only interesting arm is `All`. An `AllState` is pushed onto the stack and then `AllState.row()` is called until it indicates completion, `split` recursing into its child each time.

Expanding lates is really no different to other variables except that they must be pulled from the GlobalState rather than the tree. An internal method in StructVarValue, `resolve()` maps the Late arm to other arms in each of the accessor methods.

## The select algorithm

The select algorithm, also in `expand.rs`, is responsible for finding things at given paths. It uses `GlobalState` and `AllState` objects described in the section above and is also a recursion down the tree, this time by `do_select`. There is also a visitor, but this time just one accepting present and missing values for the output.

However, select is a little more complex than expand, and this time three arms are non-trivial: `All`, `Array` and `Condition`. Other arms basically work like expand but *choose* a child rather than iterating through them all.

The `All` branch behaviour depends on the path. If `"*"`, it works like the expand algorithm, looping through child nodes. If an integer, the `AllState` is created with `next_index` initialised to that value (rather than zero) and the child called just once.

`Condition` recurses into the child if the boolean is truthy, otherwise it is flagged as missing to the visitor.

`Array` is complex because of the possibility of containing conditions. In these case that a numeric offset is given, falsy conditions before that index can shift the required index. Therefore, each index prior to the one specified in the path must be checked as to if it is a false condition. As this is costly, the flag added to `Array` at build time indicating there are no conditions allows such arrays (the vast majority) to sidestep that check.

## The various StructTemplate manipulation functions

`replace`, `extract`, `extract_value`, and `substitute` all work directly on `StructTemplate` and are implemented in `replace.rs`. The reason for manipulations working on `StructTemplate` and not `StructBuilt` is that the built form must be compleete and has consistency checks applied, whereas the template form is designed for easy composition which also makes it suitable for manipulation.

The algorithms in `replace.rs` borrow heavily from a common core (to help reduce bugs).

All use `PathSet`s which are sets of paths, arranged as trees, which lead to a value (which PathSet is parametric in). Values can be set at a particular path and taken from a path. Currently PathSets are only ever created with a single path inside by any of their users,but other algorithms could use its ability to stor multiple paths.

Two core algorithms use PathSets. `do_find_path` recurses through a template and populates indicated paths in the passed PathSet with a function of the value found at those paths (processed via a callback). The function itself returns nothing. This method is called by `extract` and `extract_value`, with different callbacks corresponding to the required functionality.

`do_replace_path` also recurses through the tree with a given PathSet, but replaces any leaf found with a transform of its value, according to the value in the PathSet (again via a callback) and then builds up another tree based on the transformation. It returns a new copy of the passed tree but with the transformed paths replaced. This method is used by `replace` and `substitute`. The callback for `replace` is trivial, that for `substitute` extracts the needed values.
