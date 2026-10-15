int: A = if true then B else C endif;
int: B = D;
int: C = B;
int: D = A;
int: E = 0;
solve satisfy;
