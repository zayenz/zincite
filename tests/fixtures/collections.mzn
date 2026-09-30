set of int: Indices={1,2};
set of int: Allowed=Indices union {3};
array[Indices] of opt int: optional=[0x2a,<>];
list of int: values=[
0o7, % Keep  spelling
/* Next  entry */ 2
];
var opt Allowed: choice=<>;
array[1..2,1..2] of int: grid=array2d(1..2,1..2,[1,2,3,4]);
array[int] of int: column=grid[..,2];
array[int] of array[int] of int: nested=[[1,2],[3,4]];
int: picked=nested[1][2]+(values)[1]+array1d(1..2,[5,6])[1];
var 1..values[1]: bounded;
array[int] of int: keyed=[3:7,4:8];
array[int] of int: stepped=[0:5,6,7];
array[int,int] of int: tupled=[(1,2):9,(1,3):10];
array[int,int] of int: plain=[|1,2|3,4|];
array[int,int] of int: rows=[|3:1,2|4:3,4|];
array[int,int] of int: cols=[|5:6:|1,2|3,4|];
array[int,int] of int: both=[|5:6: % Keep  column indices
|3:0x1,2 % Keep  row comment
|4:3,0o4|];
array[c in 1..3] of var set(c) of 1..5: dependent_sets;
array[i in 1..2,j in 1..3] of var 1..(i+j): dependent;
array[i in 1..2] of array[j in 1..2] of var 1..(i+j): dependent_nested;
var set(1..3) of 1..5: cardinality;
set of int: generated={i+j | i,j in Indices where i<j, k=i+j where k>0};
array[int] of int: filtered=[sum(j in {k | k in Indices where k<=i})(j) | i in Indices where i>0];
array[int] of int: indexed_comp=[i:i*2 | i in Indices];
array[int,int] of int: tuple_comp=[(i,j):i+j | i,j in Indices];
array[int] of int: written_bindings=[1 | _, 'written name' in Indices where 'written name'>0];
int: short_sum=sum(i in Indices)(i);
int: expanded_sum=sum(i in Indices)(
i % Keep  body comment
);
int: long_sum=sum(first_index in Indices where first_index>0, second_index in Indices where second_index>first_index, combined_index=first_index+second_index where combined_index>0)(combined_index);
constraint forall(i in Indices)(i>0);
constraint 'forall'(i in Indices /* Keep  binding comment */ where i>0)(
/* Keep  body start */ i>0);
constraint forall(first_index in Indices where first_index>0, second_index in Indices where second_index>first_index)(first_index<second_index);
constraint picked>0;
solve satisfy;
any: uneven=[|1,222,3|55,6 /* Keep  gap */,777 % Keep this deliberately long row comment exactly as written, including  spacing
|];
