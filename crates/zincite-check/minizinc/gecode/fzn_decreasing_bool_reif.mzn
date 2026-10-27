% Reverse the array so the standard increasing relation checks decreasing order.
include "fzn_increasing_bool_reif.mzn";
predicate fzn_decreasing_bool_reif(array[int] of var bool: values, var bool: result) =
    fzn_increasing_bool_reif(reverse(values), result);
