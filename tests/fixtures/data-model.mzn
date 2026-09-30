% The data file supplies these parameters.
int: count;
array[1..count] of int: values;
record(tuple(int,int): bounds, string: name): metadata;
constraint forall(i in 1..count)(values[i]>=metadata.bounds.1 /\ values[i]<=metadata.bounds.2);
output ["\(metadata.name): \(values)\n"];
solve satisfy;
