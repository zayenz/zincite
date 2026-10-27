include "globals.mzn";
include "checker_annotations.mzn";
include "gecode.mzn";
include "gecode_fzn.mzn";

predicate gecode_array_set_element_union(
    var set of int: indices,
    array[int] of var set of int: sets,
    var set of int: result,
);

% Native calls expose the registry's offsets and marked scalar results.
var set of 1 .. 3: first :: checker_value("first", [1]);
var set of 1 .. 3: second;
var set of 1 .. 2: selected;
var set of 1 .. 3: union_result :: is_defined_var;
var set of 1 .. 3: combined :: is_defined_var;
var set of 1 .. 3: common :: is_defined_var;
var set of 1 .. 3: bounded_common :: is_defined_var;
var set of 1 .. 3: partitioned :: is_defined_var;
var set of 1 .. 3: image :: is_defined_var;
var 1 .. 3: integer;
array[1 .. 3] of var bool: flags;
array[1 .. 2] of var 1 .. 2: integers;
array[1 .. 2] of var set of 1 .. 2: buckets;
array[1 .. 2] of var set of 3 .. 4: forward;
array[1 .. 2] of var set of 1 .. 2: inverse;
array[1 .. 2] of var 1 .. 3: ordered;

constraint :: "Selected union"
gecode_array_set_element_union(selected, [first, second], union_result) :: defines_var(union_result);
constraint :: "Selected intersection"
gecode_array_set_element_intersect(selected, [first, second], common) :: defines_var(common);
constraint :: "Selected bounded intersection"
gecode_array_set_element_intersect_in(selected, [first, second], bounded_common, 1 .. 3) :: defines_var(bounded_common);
constraint :: "Selected partition"
gecode_array_set_element_partition(selected, [first, second], partitioned) :: defines_var(partitioned);
constraint :: "Native sorted channel"
gecode_set_channel_sorted(ordered, combined) :: defines_var(combined);
constraint :: "Native Boolean channel"
gecode_link_set_to_booleans(combined, flags, 1) :: checker_constraint("flags", [1, 3]);
constraint :: "Native integer set channel"
gecode_int_set_channel(integers, 1, buckets, 1) :: checker_constraint("buckets", [1, 2]);
constraint :: "Native inverse sets"
gecode_inverse_set(forward, inverse, 1, 3) :: checker_constraint("inverse", [1, 2]);
constraint :: "Native range"
gecode_range([integer], 1, {1}, image) :: defines_var(image);
constraint :: "Native set precedence"
gecode_precede_set([first, second], 1, 3) :: checker_constraint("precede", [1, 2]);
solve satisfy;
