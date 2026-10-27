include "globals.mzn";
include "checker_annotations.mzn";
include "gecode.mzn";

% Application edits determine results without choosing missing decisions.
var -3 .. 3: budget_offset :: checker_value("budget_offset", [9007199254740993]);
var 0 .. 4: tool_capacity :: checker_value("tool_capacity", [9007199254740993]) = (budget_offset + 2)
    :: checker_constraint("offset", [9007199254740993]);
var set of 1 .. 3: chosen :: checker_value("chosen", [7]);
var set of 1 .. 3: combined :: checker_value("combined", [7]) = chosen union {1};
var 0 .. 6: gcd_result :: is_defined_var;
var -2.0 .. 2.0: amount :: checker_value("amount", [8]);
var 0.0 .. 4.0: squared :: is_defined_var;
var bool: enabled :: checker_value("enabled", [9]);
var bool: allowed = budget_offset <= 1;

% Each forward relation names its output explicitly.
constraint :: "Compute common divisor"
gecode_gcd_int(budget_offset, 6, gcd_result) :: defines_var(gcd_result) :: checker_constraint("gcd", [7]);
constraint :: "Square editable amount"
gecode_float_pow(amount, 2, squared) :: defines_var(squared) :: checker_constraint("square", [8]);

% Labels and typed application IDs drive feedback for independent failures.
constraint :: "Selected tools fit the limit"
card(combined) <= tool_capacity :: checker_constraint("capacity", [7]);
constraint :: "Enable permitted values"
(enabled -> allowed) :: checker_constraint("enabled", [9]);
constraint :: "Limit squared amount"
squared <= 1.0 :: checker_constraint("limit", [8]);

% Compile flags control these source constraints before checking begins.
constraint :: "Optional symmetry representative"
symmetry_breaking_constraint(budget_offset <= 0) :: checker_constraint("symmetry", [9]);
constraint :: "Implied output lower bound"
implied_constraint(tool_capacity >= 0) :: checker_constraint("implied", [7]);

solve :: int_search([budget_offset], first_fail, indomain_min, complete) minimize tool_capacity;
