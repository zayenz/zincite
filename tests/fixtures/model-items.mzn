% Keep  include order
include "alldifferent.mzn"; % Keep  include comment
include "globals.mzn";
var 1..3:choice;
constraint::"Choice  label" choice>=1;
output::"result" ["choice = ",show(choice),"\n"];
output::json_section("details") (choice+1);
output::("extra") ["Keep  literal\n"];
solve::int_search([choice],input_order,indomain_min,complete)::restart_none minimize (choice+0)::output_var;
