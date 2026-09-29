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
constraint picked>0;
solve satisfy;
