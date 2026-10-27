% Reverse the array so the standard increasing relation checks decreasing order.
include "fzn_increasing_float_reif.mzn";
predicate fzn_decreasing_float_reif(array[int] of var float: values, var bool: result) =
    fzn_increasing_float_reif(reverse(values), result);
