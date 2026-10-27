% Zincite implements these standard FlatZinc primitive signatures.
predicate bool_clause_reif(array[int] of var bool: positive, array[int] of var bool: negative, var bool: result);
predicate array_int_minimum(var int: result, array[int] of var int: values);
predicate array_int_maximum(var int: result, array[int] of var int: values);
predicate array_float_minimum(var float: result, array[int] of var float: values);
predicate array_float_maximum(var float: result, array[int] of var float: values);
