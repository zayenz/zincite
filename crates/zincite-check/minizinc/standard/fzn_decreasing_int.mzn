% Reverse the array so the standard increasing relation checks decreasing order.
include "fzn_increasing_int.mzn";
predicate fzn_decreasing_int(array[int] of var int: values) = fzn_increasing_int(reverse(values));
