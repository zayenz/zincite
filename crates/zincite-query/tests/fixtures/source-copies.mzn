% Data types for the source-copy example.
enum People;
enum Colours = { Red, Blue };
array[People] of record(string: label, Colours: colour, set of People: friends): people;
array[int] of set of People: groups;
array[int] of 1..3: weights;
int: untouched;

% Derived model values are outside data-edit validation.
array[People] of int: derived = [1 | p in People];
solve satisfy;
