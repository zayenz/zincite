% Generic identity retains its written type and default.
function any $T: identity(
    any $T: value, % Keep  parameter comment
    int: count=1,
) :: doc_comment("Identity  docs") = let {int: local_count=count;} in if local_count>0 then value else value endif;
function array[$$Index] of var opt $T: opaque(array[$$Index] of var opt $T: values);
predicate positive(var int: value=1) :: promise_total = forall(i in 1..2)(value+i>0);
test same(int: left, int: right=left) = left=right;
predicate bare;
annotation marker;
annotation note(string: text="Keep  literal") = doc_comment(text);
solve satisfy;
