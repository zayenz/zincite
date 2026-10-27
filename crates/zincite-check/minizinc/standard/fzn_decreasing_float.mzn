% Reverse the array so the standard increasing relation checks decreasing order.
include "fzn_increasing_float.mzn";
predicate fzn_decreasing_float(array[int] of var float: values) = fzn_increasing_float(reverse(values));
