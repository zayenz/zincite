% Reverse the array so the standard increasing relation checks decreasing order.
include "fzn_increasing_set.mzn";
predicate fzn_decreasing_set(array[int] of var set of int: values) = fzn_increasing_set(reverse(values));
