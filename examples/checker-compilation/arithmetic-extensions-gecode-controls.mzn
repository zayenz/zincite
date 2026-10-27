include "globals.mzn";
include "checker_annotations.mzn";
include "gecode.mzn";
include "gecode_fzn.mzn";

% Decisions and marked results keep native argument roles visible after flattening.
var -20 .. 20: left_value :: checker_value("left_value", [1]);
var -20 .. 20: right_value;
var 1 .. 7: residue_modulus;
array[1 .. 2] of var 0 .. 1: chosen_sites;
var 0 .. 20: gcd_result :: is_defined_var;
var -400 .. 400: product_result :: is_defined_var;
var 0 .. 4: fixed_residue :: is_defined_var;
var 0 .. 6: variable_residue :: is_defined_var;
var -3 .. 2: root_result :: is_defined_var;
var 0 .. 7: minimum :: is_defined_var;
var bool: divisible :: is_defined_var;

% Explicit native calls preserve the intended forward definitions.
constraint :: "Signed gcd"
gecode_gcd_int(left_value, right_value, gcd_result) :: defines_var(gcd_result);
constraint :: "Signed product"
gecode_product_int([left_value, right_value], product_result) :: defines_var(product_result);
constraint :: "Fixed Euclidean residue"
gecode_product_mod_fixed_fzn([left_value, right_value], 5, fixed_residue) :: defines_var(fixed_residue);
constraint :: "Variable Euclidean residue"
gecode_product_mod_var_fzn([left_value, right_value], residue_modulus, variable_residue) :: defines_var(
    variable_residue,
);
constraint :: "Floor cube root"
gecode_int_nroot(left_value, 3, root_result) :: defines_var(root_result);
constraint :: "Exact minimum and pair requirement"
gecode_minimum_distance_required_fzn(chosen_sites, 2, [0, 7, 7, 0], minimum, [0, 7, 7, 0]) :: defines_var(minimum);
constraint :: "Divisibility truth"
gecode_divides_int_reif(gcd_result, left_value, divisible) :: defines_var(divisible);
constraint :: "Separate selected sites"
gecode_inter_distance_fzn(chosen_sites, 1) :: checker_constraint("sites", [1, 2]);
solve satisfy;
