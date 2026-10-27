% Reverse the array so the standard increasing relation checks decreasing order.
include "fzn_increasing_set_reif.mzn";
predicate fzn_decreasing_set_reif(array[int] of var set of int: values, var bool: result) =
    fzn_increasing_set_reif(reverse(values), result);
