% Scalar atoms: café
/* Keep  this diagram:
   x --> y
*/
par /* type  comment */ int : count=0x2a; % hexadecimal spelling
int: extra ; extra=0o7;
var int: choice =count;
float: ratio=1.25e-2;
float: upper = infinity;
bool : ready=true;
string: message="café \"quote\" \\(literal) \n \t \101 \x41 first \(count+1::doc_comment("hello")) nested \("inner \(extra*2)") last \(count) end";
int: 'quoted name'=extra;


constraint::"Scalar  label" % keep  label comment
 true;
constraint ready;
solve satisfy
