enum Colour::doc_comment("Keep  enum docs")={Red,'Ocean blue'};
enum Entry={
None, % Keep  member comment
/* Next  member */ Extra
}++FromColour(Colour)++_(1..2);
type ColourAlias::doc_comment("Keep  alias docs")=Colour;
type Counts=array[Colour] of int;
ColourAlias: colour=Red;
Entry: entry=FromColour(colour);
Counts: counts=[1,2];
Colour: original=FromColour⁻¹(entry);
constraint::"Constructor round trip" original=colour;
solve satisfy;
