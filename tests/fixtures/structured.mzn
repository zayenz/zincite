% Fields stay in written order, including their comments.
type Payload=record(tuple(int,int): coordinates, array[1..2] of int: samples);
record(Payload: details, int: z): entry=(
 details:(coordinates:(1,2), samples:[3,4]), % Keep  field comment
 z:5
);
tuple(tuple(int,int),Payload): nested=((6,7),entry.details);
int: selected=nested.1.2+entry.details.samples[1]+nested.2.coordinates.1;
opt record(int: value): missing=<>;
opt tuple(int,int): optional_pair=<>;
record(var 1..3: choice, opt int: extra): decision=(choice:2,extra:<>);
record(int: 'quoted field'): quoted=('quoted field':8,);
function tuple(int,int): pick(record(Payload: details,int: z): argument)=
 let {tuple(int,int): local=argument.details.coordinates;} in local;
constraint selected=11 /\ quoted.'quoted field'=8 /\ pick(entry).1=1;
solve satisfy;
