% Reverse the array so the standard increasing relation checks decreasing order.
include "fzn_increasing_bool.mzn";
predicate fzn_decreasing_bool(array[int] of var bool: values) = fzn_increasing_bool(reverse(values));
