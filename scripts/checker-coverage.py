#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Check the pinned library inventory in docs/checker-coverage.md."""

from __future__ import annotations

import argparse
import re
import subprocess
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MINIZINC_REV = "c3448c48e7f05f13805616cfd1b9e94972eef58c"
GECODE_REV = "186551447db0f74639d65cb8de5af190f15ac2c2"
BEGIN = "<!-- checker-coverage:begin -->"
END = "<!-- checker-coverage:end -->"
NAME = r"(?:[A-Za-z_][A-Za-z_0-9]*|'[^']*')"
CURRENT = {"fzn_disjunctive_strict"}
CURRENT |= {
    prefix + "int_" + shape + relation + suffix
    for prefix in ("", "gecode_", "fzn_")
    for shape in ("", "lin_")
    for relation in ("eq", "ne", "lt", "le", "gt", "ge")
    for suffix in ("", "_reif", "_imp")
}
CURRENT |= {
    prefix + name
    for name in (
        "int_abs",
        "int_plus",
        "int_minus",
        "int_negate",
        "int_times",
        "int_div",
        "int_mod",
        "int_min",
        "int_max",
    )
    for prefix in ("", "gecode_", "fzn_")
} | {
    "int_pow",
    "int_pow_fixed",
    "gecode_int_pow",
    "gecode_gecode_int_pow",
    "fzn_gecode_int_pow",
}

CURRENT |= {
    prefix + "bool_" + shape + relation + suffix
    for prefix in ("", "gecode_", "fzn_")
    for shape in ("", "lin_")
    for relation in ("eq", "ne", "lt", "le", "gt", "ge")
    for suffix in ("", "_reif", "_imp")
}
CURRENT |= {
    prefix + name
    for prefix in ("", "gecode_", "fzn_")
    for name in (
        "bool_not",
        "bool_and",
        "bool_and_imp",
        "bool_or",
        "bool_or_imp",
        "bool_xor",
        "bool_xor_imp",
        "bool_left_imp",
        "bool_right_imp",
        "array_bool_and",
        "array_bool_and_imp",
        "array_bool_or",
        "array_bool_or_imp",
        "array_bool_xor",
        "array_bool_xor_imp",
        "bool_clause",
        "bool_clause_reif",
        "bool_clause_imp",
        "bool2int",
    )
}


CURRENT |= {
    prefix + "float_" + shape + relation + suffix
    for prefix in ("", "gecode_", "fzn_")
    for shape in ("", "lin_")
    for relation in ("eq", "le", "lt")
    for suffix in ("", "_reif")
}
CURRENT |= {prefix + "float_ne" for prefix in ("", "gecode_", "fzn_")}
CURRENT |= {
    "float_" + shape + relation + "_imp"
    for shape in ("", "lin_")
    for relation in ("eq", "ne", "le", "lt")
}
CURRENT |= {"float_ne_reif", "float_lin_ne", "float_lin_ne_reif"}
CURRENT |= {
    prefix + name
    for prefix in ("", "gecode_", "fzn_")
    for name in (
        "float_abs",
        "float_plus",
        "float_times",
        "float_div",
        "float_min",
        "float_max",
        "int2float",
        "gecode_float_rel_imp_fzn",
    )
}
CURRENT |= {
    "float_in",
    "float_dom",
    "float_set_in",
    "float_ceil",
    "float_floor",
    "float_round",
}


CURRENT |= {
    prefix + "float_" + operation
    for prefix in ("", "gecode_", "fzn_")
    for operation in (
        "sqrt",
        "acos",
        "asin",
        "atan",
        "cos",
        "exp",
        "ln",
        "log10",
        "log2",
        "sin",
        "tan",
    )
}
CURRENT |= {
    "float_" + operation
    for operation in ("acosh", "asinh", "atanh", "cosh", "sinh", "tanh", "pow")
}
CURRENT |= {
    prefix + "gecode_float_" + operation
    for prefix in ("", "gecode_", "fzn_")
    for operation in ("pow", "pow_base", "array_min", "array_max")
}
CURRENT |= {"array_float_minimum", "array_float_maximum"}


CURRENT |= {
    prefix + "array_" + variable + kind + "_element"
    for prefix in ("", "gecode_", "fzn_")
    for variable in ("", "var_")
    for kind in ("bool", "int", "set")
}
CURRENT |= {"array_" + variable + "float_element" for variable in ("", "var_")}
CURRENT |= {
    "array_var_" + kind + "_element_nonshifted"
    for kind in ("bool", "int", "set", "float")
}
CURRENT |= {
    prefix + "gecode_" + variable + kind + "_element"
    for prefix in ("", "gecode_", "fzn_")
    for variable in ("", "var_")
    for kind in ("bool", "int")
}
CURRENT |= {
    prefix + "gecode_" + kind + "_element2d"
    for prefix in ("", "gecode_", "fzn_")
    for kind in ("bool", "int", "set")
}
CURRENT |= {
    prefix + "gecode_" + kind + "_ite"
    for prefix in ("", "gecode_", "fzn_")
    for kind in ("bool", "int", "set", "float")
}


CURRENT |= {
    prefix + name
    for prefix in ("", "gecode_", "fzn_")
    for name in (
        "set_eq",
        "set_ne",
        "set_le",
        "set_lt",
        "set_subset",
        "set_superset",
        "equal",
        "set_eq_reif",
        "set_ne_reif",
        "set_le_reif",
        "set_lt_reif",
        "set_subset_reif",
        "set_superset_reif",
        "equal_reif",
        "set_in",
        "set_in_reif",
        "set_in_imp",
        "int_in",
        "int_in_reif",
        "int_in_imp",
        "set_card",
        "set_union",
        "set_intersect",
        "set_diff",
        "set_symdiff",
        "gecode_set_rel_imp_fzn",
    )
}
CURRENT |= {
    "set_" + relation + "_imp"
    for relation in ("eq", "ne", "le", "lt", "subset", "superset")
}

CURRENT |= {
    prefix + name
    for prefix in ("", "gecode_", "fzn_")
    for name in (
        "count",
        "count_reif",
        "count_imp",
        "count_eq",
        "count_eq_reif",
        "count_eq_imp",
        "at_least_int",
        "at_most_int",
        "among",
        "nvalue",
        "gecode_global_cardinality",
        "gecode_global_cardinality_closed",
        "global_cardinality_low_up",
        "global_cardinality_low_up_closed",
        "gecode_among_seq_int",
        "gecode_among_seq_bool",
        "gecode_count_int_fzn",
        "gecode_count_matches_fzn",
    )
}


@dataclass(frozen=True)
class Declaration:
    file: str
    line: int
    kind: str
    name: str
    signature: str
    body: str | None
    terminated: bool


@dataclass(frozen=True)
class SourceFile:
    includes: tuple[str, ...]
    declarations: tuple[Declaration, ...]


@dataclass(frozen=True)
class Registration:
    name: str
    handler: str
    line: int
    condition: str


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("minizinc", type=Path, help="clean libminizinc Git root")
    parser.add_argument("gecode", type=Path, help="clean Gecode Git root")
    parser.add_argument(
        "--write", action="store_true", help="update only generated tables"
    )
    args = parser.parse_args()
    verify_root(args.minizinc, MINIZINC_REV)
    verify_root(args.gecode, GECODE_REV)
    files = read_library(args.minizinc / "share/minizinc/std", "S")
    files.update(read_library(args.gecode / "gecode/flatzinc/mznlib", "G"))
    registry_text = (args.gecode / "gecode/flatzinc/registry.cpp").read_text()
    registrations = read_registry(registry_text)
    block = render_inventory(files, registrations, registry_text)
    path = ROOT / "docs/checker-coverage.md"
    document = path.read_text()
    before, rest = document.split(BEGIN)
    _, after = rest.split(END)
    expected = before + BEGIN + "\n" + block + END + after
    if args.write:
        path.write_text(expected)
    elif document != expected:
        raise SystemExit("coverage tables differ; inspect sources and run with --write")
    for profile in "SG":
        absent = {
            (key, target)
            for key, file in files.items()
            if profile == "G" or key.startswith("S/")
            for target in resolved_includes(files, key, profile)
            if target.startswith("missing:")
        }
        print(
            f"{profile} default closure resolves; optional-entry missing edges: {len(absent)} "
            f"(checker-{30 if profile == 'S' else 31:03})"
        )
    counts = Counter(path[0] for path in files)
    print(
        f"checked {counts['S']} standard files, {counts['G']} Gecode files; "
        f"{sum(len(f.declarations) for f in files.values())} declarations; "
        f"{len(registrations)} active registrations / "
        f"{len({r.name for r in registrations})} literal names"
    )


def verify_root(root: Path, revision: str) -> None:
    def git(*args: str) -> str:
        return subprocess.check_output(
            ["git", "-C", str(root), *args], text=True
        ).strip()

    if git("rev-parse", "HEAD") != revision:
        raise SystemExit(f"{root}: expected revision {revision}")
    if git("status", "--porcelain", "--untracked-files=all"):
        raise SystemExit(f"{root}: source checkout has local changes")


def mask_comments(text: str, *, cpp: bool = False, strings: bool = False) -> str:
    """Blank comments without moving offsets; optionally blank quoted contents."""
    pattern = r'"(?:\\.|[^"\\])*"|\'[^\']*\'|/\*.*?\*/|'
    pattern += r"//[^\n]*" if cpp else r"%[^\n]*"

    def blank(match: re.Match[str]) -> str:
        value = match.group()
        if value[0] in "\"'" and not strings:
            return value
        return "".join("\n" if char == "\n" else " " for char in value)

    return re.sub(pattern, blank, text, flags=re.DOTALL)


def top_level(text: str, delimiter: str) -> list[int]:
    """Find a delimiter outside strings and balanced (), [] and {}."""
    positions: list[int] = []
    depth = 0
    quote = ""
    escaped = False
    for pos, char in enumerate(text):
        if quote:
            if escaped:
                escaped = False
            elif char == "\\" and quote == '"':
                escaped = True
            elif char == quote:
                quote = ""
        elif char in "\"'":
            quote = char
        elif char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
        elif char == delimiter and depth == 0:
            positions.append(pos)
    if depth or quote:
        raise ValueError("unbalanced source item")
    return positions


def normalize_signature(header: str) -> str:
    pattern = r'"(?:\\.|[^"\\])*"|\'[^\']*\'|\s+'
    return re.sub(
        pattern, lambda match: " " if match[0].isspace() else match[0], header
    )


def read_library(root: Path, prefix: str) -> dict[str, SourceFile]:
    files: dict[str, SourceFile] = {}
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        key = f"{prefix}/{path.relative_to(root).as_posix()}"
        text = mask_comments(path.read_text()) if path.suffix == ".mzn" else ""
        includes = tuple(re.findall(r'\binclude\s+"([^"\n]+)"\s*;', text))
        declarations: list[Declaration] = []
        start = 0
        for end in [*top_level(text, ";"), len(text)]:
            item = text[start:end].strip()
            match = re.match(r"(predicate|test|function|annotation)\b", item)
            if match:
                kind = match[1]
                equals = top_level(item, "=")
                header = item[: equals[0]].strip() if equals else item
                body = item[equals[0] + 1 :].strip() if equals else None
                if kind == "function":
                    name_match = re.search(r":\s*(" + NAME + r")\s*\(", header)
                else:
                    name_match = re.match(kind + r"\s+(" + NAME + r")", header)
                if not name_match:
                    raise ValueError(f"unrecognized declaration: {key}: {header}")
                offset = text.index(item[: len(match[0])], start, end)
                declarations.append(
                    Declaration(
                        key,
                        text.count("\n", 0, offset) + 1,
                        kind,
                        name_match[1],
                        normalize_signature(header),
                        body,
                        end < len(text),
                    )
                )
            start = end + 1
        files[key] = SourceFile(includes, tuple(declarations))
    return files


def read_registry(text: str) -> list[Registration]:
    clean = mask_comments(text, cpp=True)
    guards: list[str] = []
    conditions: dict[int, str] = {}
    for line, value in enumerate(clean.splitlines(), 1):
        if match := re.match(r"\s*#(ifdef|ifndef)\s+(\w+)", value):
            guards.append(("!" if match[1] == "ifndef" else "") + match[2])
        elif re.match(r"\s*#else\b", value):
            guards[-1] = (
                guards[-1][1:] if guards[-1].startswith("!") else "!" + guards[-1]
            )
        elif re.match(r"\s*#endif\b", value):
            guards.pop()
        conditions[line] = " & ".join(guards) or "always"
    registrations: list[Registration] = []
    for match in re.finditer(
        r'registry\(\)\.add\("([^"\n]+)"\s*,\s*&([^;]+)\);', clean
    ):
        line = clean.count("\n", 0, match.start()) + 1
        registrations.append(
            Registration(match[1], re.sub(r"\s+", "", match[2]), line, conditions[line])
        )
    if (len(registrations), len({r.name for r in registrations})) != (312, 308):
        raise ValueError("unexpected active registry surface")
    return registrations


# These groups cover registration literals outside the standard typed families.
FAMILIES = {
    19: """count count_reif count_imp count_eq count_eq_reif count_eq_imp
        at_least_int at_most_int gecode_global_cardinality
        gecode_global_cardinality_closed global_cardinality_low_up
        global_cardinality_low_up_closed gecode_among_seq_int gecode_among_seq_bool
        nvalue among gecode_count_int_fzn gecode_count_matches_fzn""",
    20: """all_different_int all_different_offset gecode_all_different_offset_fzn
        all_equal_int array_int_lt array_int_lq array_bool_lt array_bool_lq
        array_int_minimum array_int_maximum gecode_minimum_arg_int_offset
        gecode_maximum_arg_int_offset gecode_minimum_arg_bool_offset
        gecode_maximum_arg_bool_offset sort inverse_offsets increasing_int
        increasing_bool decreasing_int decreasing_bool gecode_precede
        member_int gecode_member_int_reif member_bool gecode_member_bool_reif
        gecode_array_int_ne gecode_not_all_equal_int gecode_array_bool_ne
        gecode_bool_int_channel gecode_sort_permutation gecode_all_different_optional
        gecode_all_different_except gecode_arg_min_int gecode_arg_max_int
        gecode_arg_min_bool gecode_arg_max_bool""",
    22: "gecode_regular gecode_regular_set gecode_regular_nfa_int_fzn",
    23: "gecode_circuit gecode_circuit_cost_array gecode_circuit_cost",
    24: """cumulatives gecode_schedule_unary gecode_schedule_unary_optional
        gecode_schedule_cumulative_optional fzn_disjunctive_strict""",
    25: """gecode_unary_flex gecode_cumulative_flex gecode_cumulatives_flex
        gecode_unary_flex_optional gecode_cumulative_flex_optional""",
    26: """gecode_bin_packing_load gecode_nooverlap gecode_nooverlap_fzn
        gecode_nooverlap_optional gecode_nooverlap_flex_optional""",
    27: """disjoint array_set_union array_set_partition set_convex array_set_seq
        array_set_seq_union gecode_set_weights gecode_set_convex_hull
        gecode_set_atmost_one gecode_array_set_intersect gecode_set_convex
        gecode_set_min gecode_set_min_reif gecode_set_min_imp gecode_set_max
        gecode_set_max_reif gecode_set_max_imp gecode_array_set_seq
        gecode_array_set_seq_union""",
    28: """gecode_link_set_to_booleans gecode_array_set_element_union
        gecode_array_set_element_intersect gecode_array_set_element_intersect_in
        gecode_array_set_element_partition gecode_int_set_channel gecode_range
        gecode_inverse_set gecode_precede_set gecode_set_channel_sorted""",
    29: """gecode_inter_distance gecode_inter_distance_fzn gecode_gcd_int
        gecode_gcd_int_reif gecode_gcd_int_imp gecode_divides_int
        gecode_divides_int_reif gecode_divides_int_imp gecode_product_int
        gecode_product_int_reif gecode_product_int_imp gecode_product_mod_fixed_fzn
        gecode_product_mod_fixed_fzn_reif gecode_product_mod_fixed_fzn_imp
        gecode_product_mod_var_fzn gecode_product_mod_var_fzn_reif
        gecode_product_mod_var_fzn_imp gecode_minimum_distance_fzn
        gecode_minimum_distance_required_fzn gecode_int_nroot""",
    31: "gecode_blackbox gecode_blackbox_bounds",
}
EXPLICIT_OWNERS = {
    name: task for task, names in FAMILIES.items() for name in names.split()
}
FLOAT_FUNCTIONS = {
    "acos",
    "acosh",
    "asin",
    "asinh",
    "atan",
    "atanh",
    "cos",
    "cosh",
    "exp",
    "ln",
    "log10",
    "log2",
    "sqrt",
    "pow",
    "sin",
    "sinh",
    "tan",
    "tanh",
}


COMPILER_PREDICATES = {
    "implied_constraint",
    "occurs",
    "index_sets_agree",
    "mzn_internal_check_debug_mode",
    "mzn_in_root_context",
    "mzn_in_redundant_constraint",
    "mzn_in_symmetry_breaking_constraint",
    "mzn_symmetry_breaking_constraint",
    "mzn_redundant_constraint",
    "output_to_section",
    "output_to_json_section",
}


def direct_owner(name: str) -> int:
    if name in EXPLICIT_OWNERS:
        return EXPLICIT_OWNERS[name]
    if name in {"equal", "equal_reif", "int_in", "int_in_reif", "int_in_imp"}:
        return 17
    if name.startswith("gecode_table_"):
        return 21
    if "element" in name or name.endswith("_ite"):
        return 16
    if name.startswith("int_"):
        return (
            13 if re.match(r"int_(?:lin_)?(?:eq|ne|le|lt|ge|gt)(?:_|$)", name) else 12
        )
    if name == "gecode_int_pow":
        return 12
    if name.startswith(("bool_", "array_bool_")) or name == "bool2int":
        return 14
    if name.startswith("set_") or name == "gecode_set_rel_imp_fzn":
        return 17
    if name.startswith("float_"):
        return 18 if name.split("_")[1] in FLOAT_FUNCTIONS else 15
    if name in {
        "gecode_float_pow",
        "gecode_float_pow_base",
        "gecode_float_array_min",
        "gecode_float_array_max",
        "array_float_minimum",
        "array_float_maximum",
    }:
        return 18
    if name in {"int2float", "gecode_float_rel_imp_fzn"}:
        return 15
    raise ValueError(f"unassigned direct operation: {name}")


def source_url(key: str) -> str:
    prefix, path = key.split("/", 1)
    if prefix == "S":
        url = f"https://github.com/MiniZinc/libminizinc/blob/{MINIZINC_REV}/share/minizinc/std/{path}"
    else:
        url = f"https://github.com/Gecode/gecode/blob/{GECODE_REV}/gecode/flatzinc/mznlib/{path}"
    return url


def code(value: str) -> str:
    delimiter = "``" if "`" in value else "`"
    return delimiter + " " + value.replace("|", r"\|") + " " + delimiter


def task_link(task: int) -> str:
    return f"[checker-{task:03}](#family-{task:03})"


def select_file(
    files: dict[str, SourceFile], name: str, profile: str, parent: str = ""
) -> str:
    # MiniZinc ParserState::canonicalFilename checks include paths before the parent.
    candidates = ([f"G/{name}"] if profile == "G" else []) + [f"S/{name}"]
    if parent:
        candidates.append(str(Path(parent).parent / name))
    for candidate in candidates:
        if candidate in files:
            return candidate
    if re.fullmatch(r"fzn_decreasing_(bool|float|int|set)(_reif)?\.mzn", name):
        return f"missing:{name}"
    raise ValueError(f"unresolved {profile} include {name} from {parent}")


def resolved_includes(
    files: dict[str, SourceFile], key: str, profile: str
) -> list[str]:
    targets = [select_file(files, name, profile, key) for name in files[key].includes]
    # The parser also loads every matching deprecated companion, without selecting one.
    companion = Path(key).name + ".deprecated.mzn"
    for prefix in ["G", "S"] if profile == "G" else ["S"]:
        if (candidate := f"{prefix}/{companion}") in files:
            targets.append(candidate)
    return targets


def closure(files: dict[str, SourceFile], profile: str) -> set[str]:
    pending = [
        select_file(files, name, profile)
        for name in ("stdlib.mzn", "solver_redefinitions.mzn", "globals.mzn")
    ]
    if profile == "G":
        pending.append("G/gecode.mzn")
    selected: set[str] = set()
    while pending:
        key = pending.pop()
        if key.startswith("missing:"):
            raise ValueError(f"default closure has missing include {key}")
        if key not in selected:
            selected.add(key)
            pending.extend(resolved_includes(files, key, profile))
    return selected


def calls(body: str) -> str:
    names = sorted(
        set(
            re.findall(
                r"\b([A-Za-z_][A-Za-z_0-9]*)\s*\(", mask_comments(body, strings=True)
            )
        )
    )
    return ", ".join(code(name) for name in names) or "expression/operators"


def output_rule(name: str, task: int, *, wrapper: bool = False) -> str:
    if task in {11, 31} and ("blackbox" in name or "restart" in name):
        return "excluded hook; diagnostic"
    if wrapper:
        return "compiler lowering; emitted definitions"
    if name == "bool_left_imp":
        return "argument 3 (y -> x)"
    if name == "bool_right_imp":
        return "argument 3 (x -> y)"
    if name in {"array_float_minimum", "array_float_maximum"}:
        return "argument 1 (extremum)"
    if name in {"gecode_float_rel_imp_fzn", "gecode_set_rel_imp_fzn"}:
        return "none (guard)"
    if name.endswith("_imp") and name != "bool_clause_imp":
        return "none (guard)"
    if name.endswith("_reif") or name == "bool_clause_imp":
        return "last Boolean (truth)"
    if task == 29 and "minimum_distance" in name:
        return "argument 4 (minimum distance)"
    if task in {12, 15, 18, 29}:
        if name in {
            "gecode_inter_distance",
            "gecode_inter_distance_fzn",
            "gecode_divides_int",
            "float_in",
            "float_dom",
            "float_set_in",
        }:
            return "none"
        if re.fullmatch(r"float_(?:lin_)?(?:eq|ne|le|lt|ge|gt)", name):
            return "exact equality slot" if name.endswith("eq") else "none"
        return "last result; family domain rule"
    if task == 13:
        return "exact single linear/equality slot" if name.endswith("eq") else "none"
    if task == 14:
        if name in {"bool_eq", "bool_ne", "bool_not", "bool2int"}:
            return "either scalar, when unique"
        if name.startswith("bool_lin_"):
            return "exact equality slot only" if name.endswith("eq") else "none"
        return "result Boolean when present; XOR arity rule"
    if task == 16:
        return "selected result (known index/condition)"
    if task == 17:
        if name in {
            "set_union",
            "set_intersect",
            "set_diff",
            "set_symdiff",
            "set_card",
        }:
            return "last result"
        return "either set" if name in {"set_eq", "equal"} else "none"
    if task in {24, 25, 22}:
        return "none"
    if task == 19:
        if name.removeprefix("fzn_") in {
            "at_least_int",
            "at_most_int",
            "global_cardinality_low_up",
            "global_cardinality_low_up_closed",
            "gecode_among_seq_int",
            "gecode_among_seq_bool",
        }:
            return "none"
        return "count(s), when equality fixes them"
    if task == 21:
        return "none"
    if task in {20, 23, 26, 27, 28}:
        return f"[unique outputs](#family-{task:03})"
    return "compiler lowering; emitted definitions"


def registry_signature(name: str) -> str:
    """Positional contracts for compatibility names without a library declaration."""
    if re.fullmatch(
        r"(?:int|bool)_(?:lin_)?(?:eq|ne|lt|le|gt|ge)(?:_reif|_imp)?", name
    ):
        scalar = "bool" if name.startswith("bool") else "int"
        args = (
            (
                f"array[int] of int: a, array[int] of var {scalar}: x, "
                + ("var int: c" if scalar == "bool" else "int: c")
            )
            if "_lin_" in name
            else f"var {scalar}: x, var {scalar}: y"
        )
        if name.endswith(("_reif", "_imp")):
            args += ", var bool: b"
    else:
        args = {
            "int_minus": "var int: x, var int: y, var int: z",
            "int_negate": "var int: x, var int: y",
            "bool_left_imp": "var bool: x, var bool: y, var bool: b",
            "bool_right_imp": "var bool: x, var bool: y, var bool: b",
            "int_in": "var int: x, set of int: s",
            "int_in_reif": "var int: x, set of int: s, var bool: b",
            "int_in_imp": "var int: x, set of int: s, var bool: b",
            "count": "array[int] of var int: x, var int: value, var int: count",
            "count_reif": "array[int] of var int: x, var int: value, var int: count, var bool: b",
            "count_imp": "array[int] of var int: x, var int: value, var int: count, var bool: b",
            "gecode_regular_set": "array[int] of var int: x, int: Q, int: first_symbol, int: symbols, array[int] of int: d, int: q0, set of int: F",
            "array_set_union": "array[int] of var set of int: x, var set of int: result",
        }.get(name)
        if args is None:
            raise ValueError(f"missing registry-only signature: {name}")
    return f"{name}({args})"


def render_registry(
    registrations: list[Registration],
    declarations: list[Declaration],
    ids: dict[Declaration, str],
) -> list[str]:
    rows = [
        "## Registry and exact aliases",
        "",
        "Each row is one active registration statement. Prefix expansion is literal; all three spellings use that row's handler and signature(s). Conditions describe the upstream build, not Zincite's target scope. `C` applies only to the exact current names listed above; every other alias is planned.",
        "",
        "| Row / source | Exact accepted spellings | Handler / condition | Signature source(s) or positional contract | Now | Owner | Useful output |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for number, reg in enumerate(registrations, 1):
        aliases = (reg.name, "gecode_" + reg.name, "fzn_" + reg.name)
        same_handlers = {r.name for r in registrations if r.handler == reg.handler}
        handler_aliases = {
            a for n in same_handlers for a in (n, "gecode_" + n, "fzn_" + n)
        }
        matches = [
            d
            for d in declarations
            if d.kind == "predicate" and d.body is None and d.name in handler_aliases
        ]
        signature = ", ".join(f"[{ids[d]}](#{ids[d].lower()})" for d in matches)
        if reg.handler.startswith("p_int_in"):
            matches = [d for d in matches if "var set of" not in d.signature]
            signature = ", ".join(f"[{ids[d]}](#{ids[d].lower()})" for d in matches)
        if not signature:
            signature = code(registry_signature(reg.name)) + " (handler)"
        if reg.handler.startswith("p_int_in"):
            boolean_args = "var bool: x, set of int: s" + (
                ", var bool: b" if reg.name.endswith(("_reif", "_imp")) else ""
            )
            signature += "; handler: " + code(f"{reg.name}({boolean_args})")
        if reg.name in {"array_bool_and", "array_bool_or", "array_bool_xor"}:
            signature += (
                "; handler: "
                + code(f"{reg.name}(array[int] of var bool: x)")
                + " or "
                + code(f"{reg.name}(array[int] of var bool: x, var bool: b)")
            )
        task = direct_owner(reg.name)
        now = ", ".join(code(a) for a in aliases if a in CURRENT) or "planned"
        source = f"https://github.com/Gecode/gecode/blob/{GECODE_REV}/gecode/flatzinc/registry.cpp#L{reg.line}"
        rows.append(
            f'| <a id="r{number:03}"></a>[R{number:03}]({source}) | '
            + ", ".join(code(a) for a in aliases)
            + f" | {code(reg.handler)} / {code(reg.condition)} | {signature} | {now} | {task_link(task)} | {output_rule(reg.name, task)} |"
        )
    return rows


def render_inventory(
    files: dict[str, SourceFile], registrations: list[Registration], registry_text: str
) -> str:
    validate_inventory(files, registry_text)
    declarations = [d for file in files.values() for d in file.declarations]
    ids = {d: f"D{i:04}" for i, d in enumerate(declarations, 1)}
    file_ids = {key: f"file-{i:03}" for i, key in enumerate(files, 1)}
    rows = render_name_index(files, declarations, ids)
    rows.extend(render_registry(registrations, declarations, ids))
    rows.extend(render_include_index(files, file_ids))
    rows.extend(render_declarations(files, registrations, declarations, ids, file_ids))
    return "\n".join(rows) + "\n"


def validate_inventory(files: dict[str, SourceFile], registry_text: str) -> None:
    lowerer = (ROOT / "crates/zincite-check/src/prepare.rs").read_text()
    arity = lowerer.split("match name.as_str() {", 1)[1].split("_ =>", 1)[0]
    arithmetic = (ROOT / "crates/zincite-check/src/arithmetic.rs").read_text()
    arithmetic_names = arithmetic.split("match name {", 1)[1].split("_ =>", 1)[0]
    relations = (ROOT / "crates/zincite-check/src/relations.rs").read_text()
    relation_names = relations.split("match name {", 1)[1].split("_ =>", 1)[0]
    boolean = (ROOT / "crates/zincite-check/src/boolean.rs").read_text()
    boolean_names = boolean.split("match name {", 1)[1].split("_ =>", 1)[0]
    floats = (ROOT / "crates/zincite-check/src/floats.rs").read_text()
    float_names = floats.split("match name {", 1)[1].split("_ =>", 1)[0]
    selection = (ROOT / "crates/zincite-check/src/selection.rs").read_text()
    selection_names = selection.split("match name {", 1)[1].split("_ =>", 1)[0]
    sets = (ROOT / "crates/zincite-check/src/sets.rs").read_text()
    set_names = sets.split("match name {", 1)[1].split("_ =>", 1)[0]
    admitted = set(
        re.findall(
            r'"([a-z_0-9]+)"',
            arity
            + arithmetic_names
            + relation_names
            + boolean_names
            + float_names
            + selection_names
            + set_names,
        )
    )
    counting = (ROOT / "crates/zincite-check/src/counting.rs").read_text()
    counting_names = counting.split("match base {", 1)[1].split("_ =>", 1)[0]
    admitted |= {
        prefix + name
        for name in re.findall(r'"([a-z_0-9]+)"', counting_names)
        for prefix in ("", "gecode_", "fzn_")
    }
    if admitted != CURRENT:
        raise ValueError("current admission changed; review current-support rows")
    counts = Counter(key[0] for key in files)
    if (counts["S"], counts["G"]) != (743, 102):
        raise ValueError("unexpected library file counts")
    raw = re.findall(r'registry\(\)\.add\("([^"\n]+)"', registry_text)
    if (len(raw), len(set(raw))) != (315, 311):
        raise ValueError("unexpected textual registry counts")


def render_name_index(
    files: dict[str, SourceFile],
    declarations: list[Declaration],
    ids: dict[Declaration, str],
) -> list[str]:
    closures = {profile: closure(files, profile) for profile in "SG"}
    rows = [
        "## Extracted totals and name index",
        "",
        f"The two source trees contain 743 standard files and 102 Gecode files, with {len(declarations)} declarations. The static default closure selects {len(closures['S'])} files for the standard profile and {len(closures['G'])} files for Gecode. Every file is indexed below, including optional entry points, deprecated companions and resources outside those default closures.",
        "",
        "The name index links to the first declaration; each source file table lists its overloads and repeated declarations. The registry table lists every generated alias separately from this source index.",
        "",
    ]
    first_ids: dict[str, str] = {}
    for d in declarations:
        first_ids.setdefault(d.name, ids[d])
    for letter in sorted({name[0].upper() for name in first_ids}):
        names = [name for name in sorted(first_ids) if name[0].upper() == letter]
        rows.append(
            f"**{letter}**: "
            + ", ".join(f"[{code(name)}](#{first_ids[name].lower()})" for name in names)
            + "\n"
        )
    return rows


def render_include_index(
    files: dict[str, SourceFile], file_ids: dict[str, str]
) -> list[str]:
    closures = {profile: closure(files, profile) for profile in "SG"}
    rows = [
        "",
        "## Include and selection index",
        "",
        "`S/` identifies the standard tree; `G/` identifies Gecode's tree. `default` marks membership in the roots' transitive closure. Other selected files are optional entry points. `shadowed` files still have source rows so the standard fallback stays visible. Targets list explicit includes plus compiler-loaded deprecated companions. Resources have no MiniZinc declarations.",
        "",
        "| Source file / declarations | Standard selection → targets | Gecode selection → targets |",
        "| --- | --- | --- |",
    ]
    for key, file in files.items():
        selections: list[str] = []
        for profile in "SG":
            if key.startswith("G/") and profile == "S":
                selections.append("outside standard profile")
                continue
            selected = select_file(files, key[2:], profile)
            if selected != key:
                selections.append("shadowed by " + code(selected))
                continue
            label = "default" if key in closures[profile] else "optional"
            targets = []
            for target in resolved_includes(files, key, profile):
                if target in file_ids:
                    targets.append(f"[{code(target)}](#{file_ids[target]})")
                else:
                    targets.append(
                        code(target) + " / " + task_link(30 if profile == "S" else 31)
                    )
            selections.append(label + " → " + (", ".join(targets) or "—"))
        rows.append(
            f"| [{code(key)}](#{file_ids[key]}) / {len(file.declarations)} | {selections[0]} | {selections[1]} |"
        )
    return rows


def render_declarations(
    files: dict[str, SourceFile],
    registrations: list[Registration],
    declarations: list[Declaration],
    ids: dict[Declaration, str],
    file_ids: dict[str, str],
) -> list[str]:
    aliases = {
        alias: reg.name
        for reg in registrations
        for alias in (reg.name, "gecode_" + reg.name, "fzn_" + reg.name)
    }
    builtin_names = {
        d.name
        for d in files["S/flatzinc_builtins.mzn"].declarations
        if d.kind == "predicate"
    }
    rows = [
        "",
        "## Source signatures and decomposition calls",
        "",
        "Signatures preserve fixed/variable types, dimensions, argument order and declaration annotations; whitespace outside strings is collapsed. A source body is `wrapper`, with its explicit call heads listed (operators and compiler lowering also contribute). `declaration` rows link source definitions by name; overload selection still belongs to MiniZinc. Profile-specific `direct` rows need runtime evaluation; `intrinsic` rows belong to compiler elaboration. None of these routes counts as working support unless `Now` says `C`. Each file heading links the exact semantic source, with the line in each row; direct Gecode semantics also use the registry handler and family notes.",
        "",
    ]
    for key, file in files.items():
        rows += [f'<a id="{file_ids[key]}"></a>', f"### [{key}]({source_url(key)})", ""]
        if not file.declarations:
            rows += ["Include/resource entry only; see the selection index.", ""]
            continue
        rows += [
            "| Row / line | Exact source signature | Route / semantic calls or definitions | Now | Owner | Useful output |",
            "| --- | --- | --- | --- | --- | --- |",
        ]
        for d in file.declarations:
            primitive = (
                d.name in aliases
                or d.name in builtin_names
                or d.name == "fzn_disjunctive_strict"
            )
            hook = "experimental/" in key and ("blackbox" in key or "on_restart" in key)
            if hook:
                task = 31
            elif d.kind == "annotation":
                task = 11
            elif d.body is not None or d.kind == "function" or not primitive:
                task = 31 if key.startswith("G/") else 30
            else:
                task = direct_owner(aliases.get(d.name, d.name))
            route = declaration_route(d, files, declarations, ids, primitive)
            if not d.terminated:
                route += "; source has no terminating semicolon"
            now = (
                "C (qualified)"
                if d.name in CURRENT and d.kind == "predicate" and d.body is None
                else "planned"
            )
            if hook:
                output = "excluded hook; diagnostic"
            elif d.kind == "annotation":
                output = "metadata"
            else:
                output = output_rule(
                    d.name,
                    task,
                    wrapper=d.kind == "function" or d.body is not None or not primitive,
                )
            row = ids[d]
            rows.append(
                f'| <a id="{row.lower()}"></a>{row} / {d.line} | {code(d.signature)} | {route} | {now} | {task_link(task)} | {output} |'
            )
        rows.append("")
    return rows


def declaration_route(
    declaration: Declaration,
    files: dict[str, SourceFile],
    declarations: list[Declaration],
    ids: dict[Declaration, str],
    primitive: bool,
) -> str:
    key = declaration.file
    if "experimental/" in key and ("blackbox" in key or "on_restart" in key):
        return "excluded hook"
    if declaration.kind == "annotation":
        return "annotation metadata; see classes"
    if declaration.body is not None:
        return "wrapper → " + calls(declaration.body)
    if declaration.kind == "function":
        return "compiler intrinsic"
    definitions = [
        d
        for d in declarations
        if d.kind == "predicate" and d.name == declaration.name and d.body is not None
    ]
    if not primitive:
        links = ", ".join(f"[{ids[d]}](#{ids[d].lower()})" for d in definitions)
        if links:
            return "declaration → " + links
        if declaration.name not in COMPILER_PREDICATES:
            raise ValueError(
                f"unclassified bodyless predicate: {key}: {declaration.name}"
            )
        return "compiler intrinsic"
    routes: list[str] = []
    for profile in "SG":
        if key.startswith("G/") and profile == "S":
            routes.append("S outside profile")
            continue
        selected = [
            d
            for d in definitions
            if (profile == "G" or d.file.startswith("S/"))
            and select_file(files, d.file[2:], profile) == d.file
        ]
        links = ", ".join(f"[{ids[d]}](#{ids[d].lower()})" for d in selected)
        routes.append(profile + (" decomposition → " + links if links else " direct"))
    return "; ".join(routes)


if __name__ == "__main__":
    main()
