include "globals.mzn";
include "checker_annotations.mzn";
var 0.0 .. 2.0: x :: checker_value("primary", [1]);
var 0.0 .. 4.0: total;
var int: rounded;
var bool: small;
% Derived float and conversion outputs have exact forward directions.
constraint :: "Float addition"
float_plus(x, 0.25, total);
constraint :: "Ceiling"
float_ceil(total, rounded);
constraint :: "Small total"
float_lt_reif(total, 2.0, small);
constraint :: "Known upper bound"
float_le(total, 3.0);
solve :: float_search([x], 0.000001, input_order, indomain_split, complete) minimize total;
