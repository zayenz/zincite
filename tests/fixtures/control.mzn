int: count=3;
int: value=(if count>0 then
    let { int: local_value=count+1; % Keep  local spelling
        array[1..2] of int: offsets=[1,2],
        constraint :: "Local bound" local_value>0;
    } in (if local_value>2 then local_value+offsets[1] elseif local_value=2 then 2 else 0 endif)
elseif count=0 then 0 else -1 endif);
constraint :: "Nested local scope"
let {var 0..10: local_value=value; constraint local_value>0;} in
    if count>0 then let {int: count=1;} in local_value>=count endif;
solve satisfy;
