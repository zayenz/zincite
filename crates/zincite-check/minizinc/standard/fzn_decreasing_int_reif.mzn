% Reverse the array so the standard increasing relation checks decreasing order.
include "fzn_increasing_int_reif.mzn";
predicate fzn_decreasing_int_reif(array[int] of var int: values, var bool: result) =
    fzn_increasing_int_reif(reverse(values), result);
