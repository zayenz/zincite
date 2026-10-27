include "globals.mzn";
include "checker_annotations.mzn";

% Public views use their original index sets, including nonzero offsets.
array[2 .. 3] of var 1 .. 2: integers;
array[1 .. 2] of var set of 2 .. 3: buckets;
array[2 .. 3] of var bool: flags;
var set of 2 .. 3: selected :: checker_value("selection", [2, 3]);
var set of 1 .. 2: image;
array[1 .. 2] of var set of 2 .. 3: forward;
array[2 .. 3] of var set of 1 .. 2: inverse;
array[1 .. 2] of var set of 1 .. 3: sequence;

% Each channel and selection has a separate application label.
constraint :: "Public Boolean channel"
link_set_to_booleans(selected, flags) :: checker_constraint("flags", [2, 3]);
constraint :: "Public integer set channel"
int_set_channel(integers, buckets) :: checker_constraint("buckets", [2, 3]);
constraint :: "Public inverse sets"
inverse_set(forward, inverse) :: checker_constraint("inverse", [1, 2]);
constraint :: "Public range"
range(integers, selected, image) :: checker_constraint("range", [2, 3]);
constraint :: "Public roots"
roots(integers, selected, image) :: checker_constraint("roots", [2, 3]);
constraint :: "Public set member"
member(buckets, selected) :: checker_constraint("member", [1, 2]);
constraint :: "Public set precedence"
value_precede(2, 3, buckets) :: checker_constraint("precede", [1, 2]);
constraint :: "Public set sequence"
seq_precede_chain(sequence) :: checker_constraint("sequence", [1, 2]);
solve satisfy;
