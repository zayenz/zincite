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
string: message="raw \\ slash and \n";
int: 'quoted name'=extra;


constraint::"Scalar  label" % keep  label comment
 true;
constraint ready;
solve satisfy
