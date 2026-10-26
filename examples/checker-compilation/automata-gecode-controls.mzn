include "globals.mzn";
include "checker_annotations.mzn";

% Direct declarations expose the registry's fixed row-major encodings.
predicate gecode_regular(
    array[int] of var int: entries,
    int: states,
    int: symbols,
    array[int] of int: transitions,
    int: initial,
    set of int: accepting,
);
predicate gecode_regular_set(
    array[int] of var int: entries,
    int: states,
    int: minimum,
    int: maximum,
    array[int] of int: transitions,
    int: initial,
    set of int: accepting,
);
predicate gecode_regular_nfa_int_fzn(
    array[int] of var int: entries,
    int: states,
    int: first,
    int: symbols,
    array[int] of set of int: transitions,
    int: initial,
    set of int: accepting,
);

array[1 .. 2] of var 1 .. 2: word :: checker_value("word", [1, 2]);
array[1 .. 2] of var -1 .. 0: shifted_word :: checker_value("shifted_word", [3, 4]);
array[1 .. 2] of var -1 .. 0: nfa_word :: checker_value("nfa_word", [5, 6]);

% The fourth shifted-DFA argument is an inclusive endpoint; NFA uses a count.
constraint :: "DFA word"
gecode_regular(word, 2, 2, [2, 0, 2, 1], 1, {1}) :: checker_constraint("dfa_word", [1, 2]);
constraint :: "Shifted DFA word"
gecode_regular_set(shifted_word, 2, -1, 0, [2, 0, 0, 1], 1, {1}) :: checker_constraint("shifted_word", [3, 4]);
% The target sets represent branching paths, with every transition consuming a symbol.
constraint :: "NFA word"
gecode_regular_nfa_int_fzn(nfa_word, 3, -1, 2, [{1, 2}, {}, {}, {3}, {}, {3}], 1, {3}) :: checker_constraint(
    "nfa_word",
    [5, 6],
);
solve satisfy;
