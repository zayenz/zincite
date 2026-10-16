# Formatting style

Zincite formats MiniZinc layout while keeping identifier and literal spelling,
comments and expression structure. The default is four spaces and a 120-column
limit. The examples below show actual formatter output; they are syntax excerpts,
so names such as `Position` need declarations in a complete model.

Use the [format-on-save recipe](format-on-save.md) to apply this layout in your
editor. The examples use default settings unless an option is shown.

## Let bodies and quantified conditions

The expression after `} in` starts at the same indentation as `let`. A callable's
body starts below its signature.

Before:

```minizinc
predicate positive(array[int] of var int: xs)=let {int: n=length(xs);} in if n=0 then true else xs[1]>0 endif;
```

After:

```minizinc
predicate positive(array[int] of var int: xs) =
    let {
        int: n = length(xs);
    } in
    if n = 0 then
        true
    else
        xs[1] > 0
    endif;
```

A short `forall` directly in an `if` or `elseif` condition can remain inline when
the complete condition fits. Comments and genuinely multiline bodies keep their
expanded layout.

Before:

```minizinc
predicate bounded(array[int] of var int: xs)=if forall(i in index_set(xs))(xs[i]>0) then true else false endif;
```

After:

```minizinc
predicate bounded(array[int] of var int: xs) =
    if forall (i in index_set(xs)) (xs[i] > 0) then
        true
    else
        false
    endif;
```

Other `forall` bodies expand. A string label always gets a line break, with the
constraint expression starting at the same indentation as `constraint`.
Generator calls have a space before their arguments; ordinary calls do not.
Broken binary operators stay at the end of the preceding line.

Before:

```minizinc
constraint :: "Keep every value positive" forall(i in Position)(value[i]>0 /\ value[i]<10);
constraint forall(i in Position)(value[i]>0);
```

After:

```minizinc
constraint :: "Keep every value positive"
forall (i in Position) (
    value[i] > 0 /\
    value[i] < 10
);
constraint forall (i in Position) (
    value[i] > 0
);
```

## Implicit and explicit comprehension indices

An expanded comprehension keeps its scalar or tuple index prefix immediately
after `[`. Its body and generators use the same indentation as an implicit
comprehension. The separator `|` starts the generator line, with generator text
aligned with the body. The final generator has no trailing comma.

These examples differ only in the written index prefix. Formatting preserves
that difference; it never invents or changes an index expression.

Before:

```minizinc
any: shards = [
let {var 0..9: value;} in value
| p, q in Position,
];
any: shards = [p:
let {var 0..9: value;} in value
| p, q in Position,
];
any: shards = [(p,q):
let {var 0..9: value;} in value
| p, q in Position,
];
```

After:

```minizinc
any: shards = [
    let {
        var 0 .. 9: value;
    } in
    value
  | p, q in Position
];
any: shards = [ p:
    let {
        var 0 .. 9: value;
    } in
    value
  | p, q in Position
];
any: shards = [ (p, q):
    let {
        var 0 .. 9: value;
    } in
    value
  | p, q in Position
];
```

Short, simple comprehensions remain compact when they fit. A long generator
header expands to one generator per line; a `where` filter goes below its own
generator. Generator order and filter attachment stay intact.

## Nested arrays and parentheses

Nested opening and closing brackets stay together when the expression fits.
The formatter preserves each array layer and pair of parentheses. A short
conditional inside those arrays can stay on one line:

Before:

```minizinc
any: value=[[[if p then 1 else 0 endif]]];
any: choices=[[if p then [i|i in Position] else [0] endif]];
```

After:

```minizinc
any: value = [[[if p then 1 else 0 endif]]];
any: choices = [[if p then [i | i in Position] else [0] endif]];
```

A longer body wraps its operands or branches while keeping adjacent brackets
together. With `--max-line-length 60`:

Before:

```minizinc
any: value=[[[first_operand+second_operand+third_operand+fourth_operand+fifth_operand]]];
```

After:

```minizinc
any: value = [[[first_operand + second_operand +
    third_operand + fourth_operand + fifth_operand]]];
```

With `--max-line-length 40`, an outer conditional expands while its short inner
conditional remains compact:

Before:

```minizinc
any: value=[[[if p then (if q then 1 else 2 endif) else 0 endif]]];
```

After:

```minizinc
any: value = [[[if p then
    (if q then 1 else 2 endif)
else
    0
endif]]];
```

Comments, blank lines and explicitly expanded lists can force line breaks.
Long chains of brackets also split when they exceed the width limit. Nested
conditional branches keep their indentation, even when this makes a deeply
nested expression wide.

## Matrices

Two-dimensional literals use one logical row per line and aligned columns.
Alignment makes tables easier to read, but changing a cell's width can realign
other rows. When rows exceed the width limit, every row wraps at the same column
boundaries. An indivisible cell may exceed the limit.

Before:

```minizinc
array[1..2,1..3] of int: costs=[|1,200,3|40,5,600|];
```

After:

```minizinc
array[1 .. 2, 1 .. 3] of int: costs = [|
      1,  200, 3
    | 40, 5,   600
|];
```

## Comments and expanded lists

Comment text, internal spaces and line breaks stay intact, including diagrams.
The formatter adjusts surrounding indentation without reflowing prose.
Explicitly multiline comma-separated lists remain expanded even when they would
fit on one line. Expanded lists use trailing commas where the grammar permits.

Before:

```minizinc
% Diagram: A --> B
%          |     |
array[1..2] of int: codes=[
0x2a, % Keep  hexadecimal spelling
/* Second  entry */ 0o7
];
```

After:

```minizinc
% Diagram: A --> B
%          |     |
array[1 .. 2] of int: codes = [
    0x2a, % Keep  hexadecimal spelling
    /* Second  entry */ 0o7,
];
```

Adjacent include groups sort by path, with `globals.mzn` first. Attached comments
move with their include. Blank lines, section comments, suppression directives
and protected spans act as sorting boundaries. Other top-level items keep their
order. Formatting never renames identifiers, inserts labels or includes, or
rewrites the model.

## Formatting directives

A standalone `% zincite-fmt: skip` preserves the next complete top-level item.
Standalone `off` and `on` comments preserve the complete items between them,
including their whitespace.

Before:

```minizinc
% zincite-fmt: skip
int: untouched=  1;
% zincite-fmt: off
int: also_untouched=  2;
% zincite-fmt: on
int: formatted=3;
```

After:

```minizinc
% zincite-fmt: skip
int: untouched=  1;
% zincite-fmt: off
int: also_untouched=  2;
% zincite-fmt: on
int: formatted = 3;
```

Protected source must still parse. Nested or unmatched markers, a skip with no
following item, and off/on boundaries inside an expression are errors. Syntax
or directive errors fail formatting for the whole file and leave it untouched.

## Width, indentation and line endings

EditorConfig supplies indentation, tab width, editable line endings, width,
final-newline and trailing-whitespace settings, and UTF-8/BOM selection.
Command-line overrides take precedence. For example:

```sh
zincite-fmt --indent-size 2 --max-line-length 40 model.mzn
zincite-fmt --indent-style tab --indent-size 4 --tab-width 4 model.mzn
zincite-fmt --max-line-length off --end-of-line crlf model.mzn
```

With `--indent-size 2 --max-line-length 40`, this compact comprehension expands.
The `|` indentation changes with the body indentation, while its following
text remains aligned with the body.

Before:

```minizinc
any: result = [(p,q):p+q | p in Position, q in Position];
```

After:

```minizinc
any: result = [ (p, q):
  p + q
| p in Position,
  q in Position
];
```

Width counts Unicode scalar values as one column and expands tabs to their next
configured stop. Unbreakable identifiers, strings, URLs and preserved comments
can exceed the limit without a warning or failure. Protected comments, literal
contents and skipped spans take precedence over newline conversion and trimming.
See `zincite-fmt --help` for all accepted option values.
