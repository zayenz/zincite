include "alldifferent.mzn";
enum Colour={Red,Blue};
enum Tagged={Empty}++Wrap(Colour);
type Payload=record(tuple(int,int): bounds,array[Colour] of int: values);
annotation note(string: text)=doc_comment(text);
function any $T: identity(any $T: value)=value;
function int: total(Payload: payload, int: extra=1)::note("Keep  docs")=
 let {int: offset=if extra>0 then extra else 0 endif; constraint offset>=0;}
 in sum(c in Colour where payload.values[c]>0)(payload.values[c])+offset;
Payload: payload=(bounds:(1,3),values:[Red:1,Blue:2]);
array[Colour] of int: doubled=[c:identity(payload.values[c])*2|c in Colour];
array[1..2,1..2] of int: matrix=[|1,2|3,4|];
set of int: selected={matrix[i,1]|i in 1..2 where matrix[i,1]>1};
Tagged: tagged=Wrap(Red);
var set(1) of 1..3: chosen={1};
array[i in 1..2] of var 1..(i+2): decision=[1,2];
constraint::"Range \(payload.bounds.1)" forall(c in Colour)(doubled[c]>=payload.bounds.1);
constraint alldifferent(decision) /\ total(payload)=4 /\ tagged=Wrap(Red) /\ selected={3};
output::"result" ["total \(total(payload))\n"];
solve::int_search(decision,input_order,indomain_min,complete) minimize sum(i in 1..2)(decision[i]);
