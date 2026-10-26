// Inputs are built bindings, prepared artifacts, typed assignments and a native checker.
const assert = require("node:assert/strict");
const { readFileSync, writeFileSync, mkdtempSync, rmSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { resolve, join } = require("node:path");
const { spawnSync } = require("node:child_process");

assert.equal(process.argv.length, 8,
    "usage: node smoke.cjs GLUE.js SCHEDULE.json EXACT.json BENCH.json CANDIDATES.json NATIVE_CHECKER");
const [glue, schedule, exact, bench, candidates, native] = process.argv.slice(2).map(path => resolve(path));
const { PreparedModel } = require(glue);
const paths = { schedule, exact, bench };
const models = Object.fromEntries(Object.entries(paths).map(([name, path]) =>
    [name, new PreparedModel(readFileSync(path, "utf8"))]));
const cases = JSON.parse(readFileSync(candidates, "utf8"));
const directory = mkdtempSync(join(tmpdir(), "zincite-wasm-"));

try {
    // The second pass checks fresh completion after changing and removing inputs.
    for (let pass = 0; pass < 2; pass++) {
        for (const candidate of cases) {
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
    console.log(`Native/Wasm reports agree across ${cases.length * 2} checks; exact integers, arithmetic, sets, global findings and input errors verified.`);
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
    const result = spawnSync(native, ["--load", "--format", "json", paths[candidate.model], assignment],
        { encoding: "utf8" });
    if (result.error) throw result.error;
    assert.equal(result.status, candidate.status === "valid" ? 0 : 1, result.stderr);
    return JSON.parse(result.stdout);
}

function literal(value) {
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
    if (name === "exact-arithmetic-conflict") {
        const finding = report.violations.find(finding => finding.details.operation === "add");
        assert.equal(finding.details.kind, "integer_arithmetic");
        assert.equal(finding.details.inputs[0].value.value, "9007199254740993");
        assert.equal(finding.details.result.value.value, "9007199254740995");
        assert.equal(finding.details.result.variable.name, "incremented");
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
