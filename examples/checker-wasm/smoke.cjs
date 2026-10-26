// Inputs are built bindings, prepared artifacts, typed assignments and a native checker.
const assert = require("node:assert/strict");
const { readFileSync, writeFileSync, mkdtempSync, rmSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { resolve, join } = require("node:path");
const { spawnSync } = require("node:child_process");

assert.equal(process.argv.length, 12,
    "usage: node smoke.cjs GLUE.js SCHEDULE.json EXACT.json BENCH.json FLOAT.json SELECTION.json SETS.json FUNCTIONS.json CANDIDATES.json NATIVE_CHECKER");
const [glue, schedule, exact, bench, floats, selection, sets, functions, candidates, native] = process.argv.slice(2).map(path => resolve(path));
const { PreparedModel } = require(glue);
const paths = { schedule, exact, bench, floats, selection, sets, functions };
const defaults = {};
const models = Object.fromEntries(Object.entries(paths).map(([name, path]) =>
    [name, new PreparedModel(readFileSync(path, "utf8"))]));
for (const [name, model] of Object.entries(models)) defaults[name] = JSON.parse(model.floatTolerance());
const cases = JSON.parse(readFileSync(candidates, "utf8"));
const directory = mkdtempSync(join(tmpdir(), "zincite-wasm-"));

try {
    // The second pass checks fresh completion after changing and removing inputs.
    for (let pass = 0; pass < 2; pass++) {
        for (const candidate of cases) {
            const tolerance = candidate.tolerance ?? defaults[candidate.model];
            models[candidate.model].setFloatTolerance(tolerance.absolute, tolerance.relative);
            assert.deepEqual(JSON.parse(models[candidate.model].floatTolerance()), tolerance);
            if (candidate.error) {
                assert.throws(() => models[candidate.model].check(JSON.stringify(candidate.assignment)), thrown => {
                    const error = JSON.parse(thrown);
                    assert.equal(error.kind, candidate.error);
                    assert.equal(error.diagnostics[0].location.line, 8);
                    const text = error.diagnostics.map(({ location, message }) =>
                        `${location.source}:${location.line}:${location.column}: ${message}`).join("\n");
                    assert.equal(text, nativeReport(candidate));
                    return true;
                });
                if (pass === 0) console.log(`${candidate.name}: located ${candidate.error} error`);
                continue;
            }
            const report = JSON.parse(models[candidate.model].check(JSON.stringify(candidate.assignment)));
            assert.equal(report.status, candidate.status, candidate.name);
            assert.equal(report.completeness.complete, candidate.complete, candidate.name);
            assert.deepEqual(report, nativeReport(candidate), candidate.name);
            checkDetails(candidate.name, report);
            if (pass === 0) console.log(`${candidate.name}: ${report.status}, complete=${report.completeness.complete}`);
        }
    }
    assert.throws(() => new PreparedModel('{"format_version":2,"model":{}}'));
    assert.throws(() => models.exact.check(JSON.stringify({ entries: [{
        name: "x", value: { kind: "scalar", value: { kind: "integer", value: 9007199254740992 } }
    }] })), error => String(error).includes("string"));
    assert.throws(() => models.exact.check(JSON.stringify({ entries: [{
        name: "missing", value: { kind: "scalar", value: { kind: "integer", value: "1" } }
    }] })), error => JSON.parse(error).kind === "assignment");
    for (const value of [null, "1.0"]) {
        assert.throws(() => models.floats.check(JSON.stringify({ entries: [{ name: "x", value: { kind: "scalar", value: { kind: "float", value } } }] })));
    }
    assert.throws(() => models.floats.check('{"entries":[{"name":"x","value":{"kind":"scalar","value":{"kind":"float","value":1e999}}}]}'));
    for (const value of [NaN, Infinity, -1]) assert.throws(() => models.floats.setFloatTolerance(value, 0));
    console.log(`Native/Wasm reports and errors agree across ${cases.length * 2} checks; finite floats/functions, saved/custom/strict tolerance, exact integers, arithmetic, relations, Booleans, sets, selections, global findings and input errors verified.`);
} finally {
    for (const model of Object.values(models)) model.free();
    rmSync(directory, { recursive: true, force: true });
}

function nativeReport(candidate) {
    const assignment = join(directory, "candidate.dzn");
    writeFileSync(assignment, candidate.assignment.entries.map(entry => {
        assert.match(entry.name, /^[A-Za-z_][A-Za-z0-9_]*$/);
        const value = entry.value.kind === "array"
            ? `[${entry.value.value.map(literal).join(",")}]`
            : literal(entry.value.value);
        return `${entry.name}=${value};`;
    }).join("\n"));
    const flags = candidate.tolerance ? ["--float-absolute-tolerance", String(candidate.tolerance.absolute), "--float-relative-tolerance", String(candidate.tolerance.relative)] : [];
    const result = spawnSync(native, ["--load", "--format", "json", ...flags, paths[candidate.model], assignment],
        { encoding: "utf8" });
    if (result.error) throw result.error;
    if (candidate.error) {
        assert.equal(result.status, 2, result.stderr);
        assert.equal(result.stdout, "");
        return result.stderr.trim();
    }
    assert.equal(result.status, candidate.status === "valid" ? 0 : 1, result.stderr);
    return JSON.parse(result.stdout);
}

function literal(value) {
    if (value.kind === "float") {
        assert.equal(typeof value.value, "number");
        assert(Number.isFinite(value.value));
        const text = String(value.value);
        return /[.eE]/.test(text) ? text : `${text}.0`;
    }
    if (value.kind === "boolean") {
        assert.equal(typeof value.value, "boolean");
        return String(value.value);
    }
    if (value.kind === "integer_set") {
        assert(Array.isArray(value.value.ranges));
        // A union of ranges has no literal FlatZinc spelling; these focused inputs use one interval or singleton ranges.
        const ranges = value.value.ranges;
        if (ranges.length === 1 && ranges[0].lower !== ranges[0].upper) {
            return `${decimal(ranges[0].lower)}..${decimal(ranges[0].upper)}`;
        }
        assert(ranges.every(range => range.lower === range.upper));
        return `{${ranges.map(range => decimal(range.lower)).join(",")}}`;
    }
    assert.equal(value.kind, "integer");
    return decimal(value.value);
}

function decimal(value) {
    assert.equal(typeof value, "string");
    assert.match(value, /^-?\d+$/);
    return value;
}

function checkDetails(name, report) {
    if (name === "sets-valid") assert.equal(report.objective.argument.value.value, "2");
    if (name === "sets-swapped") {
        const order = report.violations.find(finding => finding.details.native_code === "8");
        assert.equal(order.details.kind, "set_relation");
        assert.equal(order.details.relation, "characteristic_less");
        assert.equal(order.details.left.value.value.ranges[0].lower, "9007199254740992");
        assert.equal(order.check.constraint.metadata.application.id, "native_order");
        assert(report.violations.some(finding => finding.details.relation === "less_equal"));
    }
    if (name === "sets-conflict") {
        for (const kind of ["set_operation", "set_membership", "set_cardinality"]) {
            assert(report.violations.some(finding => finding.details.kind === kind), kind);
        }
    }
    if (name === "sets-incomplete-invalid") {
        assert(report.violations.some(finding => finding.details.kind === "boolean_relation"));
        assert(report.completeness.blocked_checks.length > 0);
    }
    if (name === "selection-saved") assert.equal(report.objective.argument.value.value, "7");
    if (name === "selection-conflict") {
        for (const kind of ["element_mismatch", "conditional_mismatch"]) {
            assert(report.violations.some(finding => finding.details.kind === kind), kind);
        }
        const element = report.violations.find(finding => finding.check.constraint?.metadata.application?.id === "selection");
        assert.deepEqual(element.details.layout, {kind: "offset", offset: "-2"});
        assert.equal(element.details.selected.value.value, "7");
        assert.equal(element.details.result.variable.name, "picked");
    }
    if (name === "selection-range") {
        const range = report.violations.find(finding => finding.details.kind === "element_range");
        assert.equal(range.details.indices[0].value.value, "9223372036854775807");
        assert.equal(range.details.result, null);
    }

    if (name === "float-conflict") {
        for (const kind of ["float_arithmetic", "float_conversion", "float_relation", "float_membership"]) {
            assert(report.violations.some(finding => finding.details.kind === kind), kind);
        }
    }
    if (name === "float-default") assert.equal(report.objective.argument.value.value, 1.8455124082255701);
    if (name === "exact-arithmetic-conflict") {
        const finding = report.violations.find(finding => finding.details.operation === "add");
        assert.equal(finding.details.kind, "integer_arithmetic");
        assert.equal(finding.details.inputs[0].value.value, "9007199254740993");
        assert.equal(finding.details.result.value.value, "9007199254740995");
        assert.equal(finding.details.result.variable.name, "incremented");
        const scalar = report.violations.find(finding => finding.details.relation === "greater_equal");
        assert.equal(scalar.details.kind, "integer_relation");
        assert.equal(scalar.details.left.value.value, "9007199254740993");
        assert.equal(scalar.details.condition.kind, "reified");
        assert.equal(scalar.details.condition.result.value.value, false);
        const linear = report.violations.find(finding => finding.details.kind === "linear_relation");
        assert.equal(linear.details.relation, "less");
        assert.equal(linear.details.sum, "9007199254740993");
        assert.equal(linear.details.bound, "9007199254740994");
        assert.equal(linear.details.condition.result.variable.name, "at_least");
    }
    if (name === "exact-boolean-conflict") {
        for (const kind of ["boolean_operation", "boolean_relation", "boolean_clause", "boolean_linear", "boolean_to_integer"]) {
            assert(report.violations.some(finding => finding.details.kind === kind), kind);
        }
        const linear = report.violations.find(finding => finding.details.kind === "boolean_linear");
        assert.equal(linear.details.sum, "9007199254740992");
        assert.equal(linear.details.bound.value.value, "0");
        assert.equal(linear.details.bound.variable.name, "boolean_total");
    }
    if (name === "exact-boolean-incomplete-invalid") {
        assert(report.violations.some(finding => finding.details.kind === "boolean_to_integer"));
        assert(report.completeness.blocked_checks.length > 0);
    }
    if (name === "infeasible") {
        assert.equal(report.violations.length, 3);
        assert(report.violations.some(finding =>
            finding.check.constraint?.metadata.application?.id === "precedence"));
    }
    if (name === "incomplete" || name === "incomplete-invalid") {
        assert(report.completeness.unresolved_variables.length > 0);
        assert(report.completeness.blocked_checks.length > 0);
    }
    if (name === "incomplete-invalid") assert(report.violations.length > 0);
    if (name === "exact-valid") {
        assert.equal(report.objective.argument.value.value, "9007199254740993");
    }
    if (name === "bench-invalid") {
        assert.equal(report.violations.length, 5);
        const finding = report.violations.find(finding => finding.details.kind === "disjunctive_strict");
        assert.deepEqual(finding.details.conflicts, [{ first: 1, second: 2 }]);
        assert.deepEqual(finding.check.constraint.metadata.application, { id: "shared_bench", entities: ["1", "2"] });
    }
    if (name === "bench-incomplete") {
        assert.equal(report.violations[0].details.kind, "disjunctive_strict");
        assert(report.completeness.blocked_checks.length > 0);
    }
    if (name === "exact-invalid") {
        const finding = report.violations[0];
        const setFinding = report.violations.find(finding => finding.details.kind === "set_equal");
        assert.deepEqual(setFinding.details.left.value.value.ranges, [{ lower: "9007199254740992", upper: "9007199254740992" }]);
        assert.deepEqual(setFinding.details.right.value.value.ranges, [{ lower: "9007199254740993", upper: "9007199254740993" }]);
        assert.equal(finding.details.sum, "9007199254740992");
        assert.equal(finding.details.bound, "9007199254740993");
        assert.deepEqual(finding.check.constraint.metadata.application,
            { id: "exact", entities: ["9007199254740993"] });
    }
}
