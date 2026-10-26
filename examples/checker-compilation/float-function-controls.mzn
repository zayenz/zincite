include "globals.mzn";
include "checker_annotations.mzn";

var 1.0 .. 4.0: x :: checker_value("float_input", [18]);
var float: root :: is_defined_var;
var float: cube :: is_defined_var;
var float: exponential :: is_defined_var;
var float: hyperbolic :: is_defined_var;
var float: logarithm :: is_defined_var;

constraint :: "Square root"
float_sqrt(x, root) :: defines_var(root) :: checker_constraint("root", [18]);
constraint :: "Integer power"
float_pow(x, 3.0, cube) :: defines_var(cube) :: checker_constraint("cube", [18]);
constraint :: "Fixed base power"
float_pow(2.0, x, exponential) :: defines_var(exponential) :: checker_constraint("exponential", [18]);
constraint :: "Hyperbolic cosine"
float_cosh(x, hyperbolic) :: defines_var(hyperbolic) :: checker_constraint("hyperbolic", [18]);
constraint :: "Natural logarithm"
float_ln(x, logarithm) :: defines_var(logarithm) :: checker_constraint("logarithm", [18]);
constraint :: "Array maximum"
max([root, cube, exponential]) <= 64.0 :: checker_constraint("array_maximum", [18]);

solve satisfy;
