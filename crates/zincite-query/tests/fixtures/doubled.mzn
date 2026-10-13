int: entity_count;
int: group_count;
array[1..entity_count] of record(int: weight, int: peer, int: group): entities;
array[1..entity_count, 1..2] of int: capacities;
array[1..group_count] of set of int: groups;
solve satisfy;
