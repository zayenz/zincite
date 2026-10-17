enum Topics;
enum Guests;
enum Genders = {Man, Woman, NA};
type GuestInfo = record(
    string: name,
    string: surname,
    Genders: gender,
    array[Topics] of int: interest
);
array[Guests] of GuestInfo: guests;
array[int] of set of Guests: same_table;
array[int] of set of Guests: different_tables;
enum Tables;
array[Tables] of int: table_capacities;
int: max_gender_imbalance_per_table;
solve satisfy;
