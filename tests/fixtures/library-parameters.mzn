annotation parameter_mark(string);
annotation other_mark;
function int: library_value(int, string) = 1;
predicate library_predicate(var int) = true;
test library_test(int) = true;
function int: annotated_value(
    int: value :: other_mark :: parameter_mark("Keep  annotation") = 1 :: other_mark,
) = value;
predicate annotated_bound(
    var int: bound /* Keep  parameter comment */ :: promise_ctx_monotone,
) = bound >= 0;
solve satisfy;
