include "globals.mzn";
include "checker_annotations.mzn";

% Each word uses the alphabet exposed by its standard wrapper.
array[1 .. 2] of var 1 .. 2: word :: checker_value("word", [1, 2]);
array[1 .. 2] of var -1 .. 0: shifted_word :: checker_value("shifted_word", [3, 4]);
array[1 .. 2] of var -1 .. 0: nfa_word :: checker_value("nfa_word", [5, 6]);

% The first two automata return to state one after the intended two symbols.
constraint :: "DFA word"
regular(word, 2, 2, [|
      2, 0
    | 2, 1
|], 1, {1}) :: checker_constraint("dfa_word", [1, 2]);
constraint :: "Shifted DFA word"
regular(shifted_word, 2, -1 .. 0, array2d(1 .. 2, -1 .. 0, [2, 0, 0, 1]), 1, {1}) :: checker_constraint(
    "shifted_word",
    [3, 4],
);
% Only one branch from state one can reach accepting state three.
constraint :: "NFA word"
regular_nfa(nfa_word, 3, -1 .. 0, array2d(1 .. 3, -1 .. 0, [{1, 2}, {}, {}, {3}, {}, {3}]), 1, {3})
    :: checker_constraint("nfa_word", [5, 6]);
solve satisfy;
