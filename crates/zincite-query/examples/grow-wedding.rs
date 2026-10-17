//! Grow the documented seating data shape without changing the input file.
//!
//! Run from a checkout: cargo run -p zincite-query --example grow-wedding -- INPUT.dzn FACTOR
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::ops::Range;

use zincite_lint::{EditPart, SourceSnapshot, TextEdit, prepare_text_edits};
use zincite_query::{
    ArrayIndexing, Input, ItemKind, Limits, LiteralKind, LiteralValue, Query, QueryResult,
};
use zincite_syntax::{FileMode, TokenKind};

const MAX_INPUT: usize = 1024 * 1024;
const MAX_OUTPUT: usize = 16 * 1024 * 1024;
const MAX_TOKENS: usize = 200_000;
const MAX_FACTOR: usize = 1000;
const ASSIGNMENTS: [&str; 8] = [
    "Topics",
    "Guests",
    "guests",
    "same_table",
    "different_tables",
    "Tables",
    "table_capacities",
    "max_gender_imbalance_per_table",
];

type Result<T> = std::result::Result<T, String>;

fn main() {
    if let Err(message) = run() {
        eprintln!("grow-wedding: {message}");
        std::process::exit(2);
    }
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(
            "usage: grow-wedding INPUT.dzn FACTOR (total copies, including the original)".into(),
        );
    }
    let factor = args[1]
        .to_str()
        .and_then(|text| text.parse::<usize>().ok())
        .ok_or("FACTOR must be a positive integer")?;
    let mut bytes = Vec::new();
    File::open(&args[0])
        .map_err(|error| error.to_string())?
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let candidate = grow(bytes, factor)?;
    std::io::stdout()
        .lock()
        .write_all(&candidate)
        .map_err(|error| error.to_string())
}

fn grow(bytes: Vec<u8>, factor: usize) -> Result<Vec<u8>> {
    if !(1..=MAX_FACTOR).contains(&factor) {
        return Err(format!("FACTOR must be in 1..={MAX_FACTOR}"));
    }
    if bytes.len() > MAX_INPUT {
        return Err("input exceeds 1 MiB".into());
    }
    let input = Input::parse_named(bytes, FileMode::Data, "wedding.dzn")
        .map_err(|error| format!("invalid data: {error:?}"))?;
    if input
        .syntax()
        .tokens()
        .len()
        .checked_mul(factor)
        .is_none_or(|n| n > MAX_TOKENS)
    {
        return Err("grown input exceeds the 200,000-token allowance".into());
    }
    // This example understands these eight assignments, not arbitrary dependencies.
    let QueryResult::Selection(items) = Query::parse("items", Limits::default())
        .map_err(|error| error.message)?
        .evaluate(&input, Limits::default())
        .map_err(|error| error.message)?
    else {
        unreachable!()
    };
    let mut names = BTreeSet::new();
    for item in items.items() {
        if item.kind != ItemKind::Assignment
            || !item
                .name
                .is_some_and(|name| ASSIGNMENTS.contains(&name) && names.insert(name))
        {
            return Err("expected only the eight distinct wedding data assignments".into());
        }
    }
    if names.len() != ASSIGNMENTS.len() {
        return Err("all eight wedding data assignments are required".into());
    }
    // Refuse formatter directives: copying their text could change directive scopes.
    for (index, token) in input.syntax().tokens().iter().enumerate() {
        if matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment)
            && input
                .token_bytes(index)
                .windows(b"zincite-fmt:".len())
                .any(|w| w == b"zincite-fmt:")
        {
            return Err("comments containing zincite directives are unsupported".into());
        }
    }
    let topics = literal(&input, "Topics")?;
    let guests = literal(&input, "Guests")?;
    let records = literal(&input, "guests")?;
    let same = literal(&input, "same_table")?;
    let different = literal(&input, "different_tables")?;
    let capacities = literal(&input, "table_capacities")?;
    let imbalance = literal(&input, "max_gender_imbalance_per_table")?;
    let topic_members = members(&topics)?;
    let guest_members = members(&guests)?;
    if topic_members.is_empty() || guest_members.is_empty() {
        return Err("Topics and Guests must be nonempty literal member sets".into());
    }
    let topic_names: BTreeSet<_> = topic_members.iter().map(|(name, _)| *name).collect();
    let guest_names: BTreeSet<_> = guest_members.iter().map(|(name, _)| *name).collect();
    if !topic_names.is_disjoint(&guest_names)
        || topic_names
            .iter()
            .chain(guest_names.iter())
            .any(|name| ASSIGNMENTS.contains(name) || ["Man", "Woman", "NA", "T"].contains(name))
    {
        return Err("enum members collide with the wedding model's identifiers".into());
    }
    validate_records(&records, &guest_names, topic_members.len())?;
    let same_members = group_members(&same, &guest_names)?;
    let different_members = group_members(&different, &guest_names)?;
    let table_item = items
        .items()
        .iter()
        .find(|item| item.name == Some("Tables"))
        .unwrap();
    let (table_count, table_range) = table_constructor(&input, &table_item.range)?;
    let capacity_values = positional(&capacities)?;
    if capacity_values.len() != table_count
        || capacity_values
            .iter()
            .any(|value| !matches!(value.kind, LiteralKind::Integer))
        || !matches!(imbalance.kind, LiteralKind::Integer)
    {
        return Err("table_capacities must contain one literal integer per table; imbalance must be a literal integer".into());
    }
    let grown_tables = table_count
        .checked_mul(factor)
        .filter(|count| i64::try_from(*count).is_ok())
        .ok_or("table count overflow")?;
    if factor == 1 {
        return Ok(input.source_bytes().to_vec());
    }
    let mut occupied: BTreeSet<String> = input
        .syntax()
        .tokens()
        .iter()
        .enumerate()
        .filter(|(_, token)| {
            matches!(
                token.kind,
                TokenKind::Identifier | TokenKind::QuotedIdentifier
            )
        })
        .map(|(index, _)| {
            String::from_utf8_lossy(input.token_bytes(index))
                .trim_matches('\'')
                .to_owned()
        })
        .collect();
    let mut mappings = Vec::new();
    for copy in 1..factor {
        let mut mapping = BTreeMap::new();
        for (index, (name, _)) in guest_members.iter().enumerate() {
            let generated = format!("GrownGuest_{copy}_{}", index + 1);
            if !occupied.insert(generated.clone()) {
                return Err(format!("generated guest identifier collides: {generated}"));
            }
            mapping.insert(*name, generated);
        }
        mappings.push(mapping);
    }
    let LiteralKind::Array { entries, .. } = &records.kind else {
        unreachable!()
    };
    let record_keys: Vec<_> = entries
        .iter()
        .map(|entry| {
            let key = entry.key.as_ref().unwrap();
            let LiteralKind::Member(name) = key.kind else {
                unreachable!()
            };
            (name, key.range.clone())
        })
        .collect();
    let mut edits = Vec::new();
    for (value, replacements) in [
        (&guests, guest_members),
        (&records, record_keys),
        (&same, same_members),
        (&different, different_members),
        (&capacities, Vec::new()),
    ] {
        edits.push(copy_collection(&input, value, &replacements, &mappings)?);
    }
    edits.push(TextEdit {
        range: table_range,
        replacement: vec![EditPart::Text(grown_tables.to_string())],
    });
    // Charge the exact final byte count before the public edit API allocates it.
    let mut output_len = input.source_bytes().len();
    for edit in &edits {
        output_len -= edit.range.len();
        for part in &edit.replacement {
            let len = match part {
                EditPart::Text(text) => text.len(),
                EditPart::Original(range) => range.len(),
            };
            output_len = output_len
                .checked_add(len)
                .filter(|n| *n <= MAX_OUTPUT)
                .ok_or("candidate exceeds 16 MiB")?;
        }
    }
    let snapshot = SourceSnapshot::new("wedding.dzn", input.source_bytes());
    let candidate = prepare_text_edits(&snapshot, input.source_bytes(), &edits)
        .map_err(|error| error.to_string())?;
    Input::parse_named(candidate.clone(), FileMode::Data, "grown-wedding.dzn")
        .map_err(|error| format!("invalid candidate: {error:?}"))?;
    Ok(candidate)
}

fn literal<'a>(input: &'a Input, name: &str) -> Result<LiteralValue<'a>> {
    let query = Query::parse(
        &format!("filter(name(\"{name}\")) | values"),
        Limits::default(),
    )
    .map_err(|error| error.message)?;
    let QueryResult::Literals(values) = query
        .evaluate(input, Limits::default())
        .map_err(|error| error.message)?
    else {
        unreachable!()
    };
    match values.values() {
        [value] => Ok(value.clone()),
        _ => Err(format!("expected one {name} assignment")),
    }
}

fn members<'a>(value: &LiteralValue<'a>) -> Result<Vec<(&'a str, Range<usize>)>> {
    let LiteralKind::Set(values) = &value.kind else {
        return Err("expected a literal enum member set".into());
    };
    let mut seen = BTreeSet::new();
    values
        .iter()
        .map(|value| match value.kind {
            LiteralKind::Member(name) if plain_member(value) && seen.insert(name) => {
                Ok((name, value.range.clone()))
            }
            _ => Err("expected distinct literal enum members".into()),
        })
        .collect()
}

// Remapping replaces one identifier token, never parentheses or attached comments.
fn plain_member(value: &LiteralValue<'_>) -> bool {
    let LiteralKind::Member(name) = value.kind else {
        return false;
    };
    value.spelling == name.as_bytes()
        || value
            .spelling
            .strip_prefix(b"'")
            .and_then(|s| s.strip_suffix(b"'"))
            == Some(name.as_bytes())
}

fn positional<'a, 'b>(value: &'b LiteralValue<'a>) -> Result<Vec<&'b LiteralValue<'a>>> {
    match &value.kind {
        LiteralKind::Array {
            indexing: ArrayIndexing::Positional,
            entries,
        } => Ok(entries.iter().map(|entry| &entry.value).collect()),
        _ => Err("expected a positional literal array".into()),
    }
}

fn validate_records(
    value: &LiteralValue<'_>,
    guests: &BTreeSet<&str>,
    topics: usize,
) -> Result<()> {
    let LiteralKind::Array {
        indexing: ArrayIndexing::Explicit,
        entries,
    } = &value.kind
    else {
        return Err("guests must be an explicitly guest-keyed record array".into());
    };
    let mut keys = BTreeSet::new();
    for entry in entries {
        let Some(LiteralValue {
            kind: LiteralKind::Member(name),
            ..
        }) = &entry.key
        else {
            return Err("each guest record needs an enum member key".into());
        };
        if !plain_member(entry.key.as_ref().unwrap())
            || !guests.contains(name)
            || !keys.insert(*name)
        {
            return Err("guest keys must cover Guests exactly once".into());
        }
        let LiteralKind::Record(fields) = &entry.value.kind else {
            return Err("expected a guest record".into());
        };
        let mut labels = BTreeSet::new();
        for field in fields {
            if !labels.insert(field.label.name) {
                return Err("duplicate guest record field".into());
            }
            let valid = match field.label.name {
                "name" | "surname" => matches!(field.value.kind, LiteralKind::String),
                "gender" => matches!(
                    field.value.kind,
                    LiteralKind::Member("Man" | "Woman" | "NA")
                ),
                "interest" => positional(&field.value).is_ok_and(|values| {
                    values.len() == topics
                        && values
                            .iter()
                            .all(|v| matches!(v.kind, LiteralKind::Integer))
                }),
                _ => false,
            };
            if !valid {
                return Err(format!("unsupported guest field: {}", field.label.name));
            }
        }
        if labels != BTreeSet::from(["name", "surname", "gender", "interest"]) {
            return Err("guest records require name, surname, gender and interest".into());
        }
    }
    if &keys != guests {
        return Err("guest keys must cover Guests exactly once".into());
    }
    Ok(())
}

fn group_members<'a>(
    value: &LiteralValue<'a>,
    guests: &BTreeSet<&str>,
) -> Result<Vec<(&'a str, Range<usize>)>> {
    let mut result = Vec::new();
    for group in positional(value)? {
        for (name, range) in members(group)? {
            if !guests.contains(name) {
                return Err("group contains an unknown guest".into());
            }
            result.push((name, range));
        }
    }
    Ok(result)
}

fn table_constructor(input: &Input, range: &Range<usize>) -> Result<(usize, Range<usize>)> {
    let tokens: Vec<_> = input
        .syntax()
        .tokens()
        .iter()
        .enumerate()
        .filter(|(index, token)| {
            range.contains(&input.token_range(*index).start)
                && !matches!(
                    token.kind,
                    TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                )
        })
        .map(|(index, _)| index)
        .collect();
    if tokens.len() != 9
        || tokens.iter().enumerate().any(|(position, index)| {
            position != 6
                && input.token_bytes(*index)
                    != [
                        b"Tables".as_slice(),
                        b"=",
                        b"T",
                        b"(",
                        b"1",
                        b"..",
                        b"",
                        b")",
                        b";",
                    ][position]
        })
    {
        return Err("Tables must have the exact constructor shape T(1..N)".into());
    }
    let end = input.token_bytes(tokens[6]);
    if end.is_empty() || !end.iter().all(u8::is_ascii_digit) {
        return Err("table count must be a positive decimal integer".into());
    }
    let count = std::str::from_utf8(end)
        .unwrap()
        .parse::<usize>()
        .map_err(|_| "table count overflow")?;
    if count == 0 {
        return Err("table count must be positive".into());
    }
    Ok((count, input.token_range(tokens[6])))
}

fn copy_collection(
    input: &Input,
    value: &LiteralValue<'_>,
    replacements: &[(&str, Range<usize>)],
    mappings: &[BTreeMap<&str, String>],
) -> Result<TextEdit> {
    let bytes = input.source_bytes();
    let range = value.range.clone();
    if !matches!(
        (bytes[range.start], bytes[range.end - 1]),
        (b'[', b']') | (b'{', b'}')
    ) {
        return Err("collection literals must use direct brackets or braces".into());
    }
    let inside = range.start + 1..range.end - 1;
    let last = input
        .syntax()
        .tokens()
        .iter()
        .enumerate()
        .rev()
        .find(|(index, token)| {
            inside.contains(&input.token_range(*index).start)
                && !matches!(
                    token.kind,
                    TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                )
        });
    let mut parts = vec![EditPart::Original(range.start..inside.end)];
    if let Some((_, last)) = last {
        for mapping in mappings {
            if last.kind != TokenKind::Comma {
                parts.push(EditPart::Text(",".into()));
            }
            let mut cursor = inside.start;
            for (name, replaced) in replacements {
                parts.push(EditPart::Original(cursor..replaced.start));
                parts.push(EditPart::Text(mapping[*name].clone()));
                cursor = replaced.end;
            }
            parts.push(EditPart::Original(cursor..inside.end));
        }
    }
    parts.push(EditPart::Original(inside.end..range.end));
    Ok(TextEdit {
        range,
        replacement: parts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const DATA: &[u8] = include_bytes!("fixtures/wedding.dzn");

    #[test]
    fn copies_guest_keys_and_group_members_without_rewriting_records_or_comments() {
        let candidate = grow(DATA.to_vec(), 2).unwrap();
        let text = std::str::from_utf8(&candidate).unwrap();
        assert!(text.contains("Tables = T(1..4);"));
        assert!(text.contains("GrownGuest_1_1: (name: \"A\""));
        assert!(text.contains("{GrownGuest_1_1, GrownGuest_1_2}"));
        assert_eq!(text.matches("% inside the record").count(), 2);
        assert_eq!(text.matches("% between records").count(), 2);
        assert!(candidate.starts_with(b"% unchanged heading\nTopics = {Music, Food};\n"));
        assert!(candidate.ends_with(b"max_gender_imbalance_per_table = 1; % unchanged setting\n"));
        let input = Input::parse(candidate, FileMode::Data).unwrap();
        assert_eq!(
            members(&literal(&input, "Guests").unwrap()).unwrap().len(),
            4
        );
        assert_eq!(
            positional(&literal(&input, "table_capacities").unwrap())
                .unwrap()
                .len(),
            4
        );
        assert_eq!(grow(DATA.to_vec(), 1).unwrap(), DATA);
    }

    #[test]
    fn refuses_computed_groups_collisions_and_excessive_growth() {
        let source = std::str::from_utf8(DATA).unwrap();
        let computed = source.replace("same_table = [{A, B}]", "same_table = [{A} union {B}]");
        assert!(grow(computed.into_bytes(), 2).is_err());
        let collision = source.replace("Topics = {Music, Food}", "Topics = {GrownGuest_1_1, Food}");
        assert!(
            grow(collision.into_bytes(), 2)
                .unwrap_err()
                .contains("collides")
        );
        assert!(grow(DATA.to_vec(), 0).is_err());
        assert!(grow(DATA.to_vec(), MAX_FACTOR + 1).is_err());
        assert!(grow(vec![b' '; MAX_INPUT + 1], 2).is_err());
    }
}
