import init, { PreparedModel } from "../web/zincite_check.js";

function canonical(value) {
    if (Array.isArray(value)) return value.map(canonical);
    if (value && typeof value === "object") return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
    return value;
}

// No network access occurs inside a timing window.
async function collect() {
    const config = await (await fetch("./config.json")).json();
    const wasm = await init();
    const cases = [];
    for (const copies of config.copies) {
        const base = `./case-${copies}`;
        const [prepared, assignments, expected] = await Promise.all([
            fetch(`${base}/prepared.json`).then(r => r.text()),
            fetch(`${base}/assignments.json`).then(r => r.json()),
            fetch(`${base}/reports.json`).then(r => r.json()),
        ]);
        const model = new PreparedModel(prepared);
        const inputs = assignments.map(a => JSON.stringify(a));
        // Full native/browser equality also checks reuse after removed inputs.
        for (let index = 0; index < inputs.length; index++) {
            if (JSON.stringify(canonical(JSON.parse(model.check(inputs[index])))) !== JSON.stringify(canonical(expected[index]))) {
                throw new Error(`Native/browser report mismatch: ${copies}/${index}`);
            }
        }
        const load_ms = [], boundary_ms = [], report_parse_ms = [], assignment_json_ms = [], edit_ms = [];
        const memory_before_bytes = wasm.memory.buffer.byteLength;
        for (let index = 0; index < config.warmup + config.samples; index++) {
            const start = performance.now();
            const decoded = new PreparedModel(prepared);
            const load = performance.now() - start;
            decoded.free();
            if (index >= config.warmup) load_ms.push(load);
        }
        for (let index = 0; index < config.warmup + config.samples; index++) {
            let start = performance.now();
            const state = index % inputs.length;
            start = performance.now();
            for (let repeat = 0; repeat < config.batch; repeat++) JSON.stringify(assignments[state]);
            const encode = (performance.now() - start) / config.batch;
            let report;
            start = performance.now();
            for (let repeat = 0; repeat < config.batch; repeat++) report = model.check(inputs[state]);
            const boundary = (performance.now() - start) / config.batch;
            start = performance.now();
            for (let repeat = 0; repeat < config.batch; repeat++) JSON.parse(report);
            const parse = (performance.now() - start) / config.batch;
            start = performance.now();
            const editReport = JSON.parse(model.check(JSON.stringify(assignments[state])));
            const edit = performance.now() - start;
            if (!editReport.status) throw new Error("missing report");
            if (index >= config.warmup) {
                edit_ms.push(edit);
                boundary_ms.push(boundary);
                report_parse_ms.push(parse); assignment_json_ms.push(encode);
            }
        }
        cases.push({ copies, load_ms, boundary_ms, report_parse_ms, assignment_json_ms, edit_ms,
            memory_before_bytes, memory_after_bytes: wasm.memory.buffer.byteLength,
            correctness: "all five full reports match native, including recovered feasible state" });
        model.free();
    }
    return { status: "success", config, user_agent: navigator.userAgent,
        browser_version: navigator.userAgentData ? await navigator.userAgentData.getHighEntropyValues(["fullVersionList", "architecture", "platformVersion"]) : null,
        time_origin: performance.timeOrigin, cases };
}

window.runCheckerMeasurements = collect;
document.querySelector("#run").addEventListener("click", async () => {
    const button = document.querySelector("#run");
    button.disabled = true;
    document.querySelector("#status").textContent = "Measuring…";
    try {
        window.checkerMeasurements = await collect();
        const link = document.querySelector("#download");
        link.href = URL.createObjectURL(new Blob([JSON.stringify(window.checkerMeasurements, null, 2)], {type: "application/json"}));
        link.download = "browser.json";
        link.hidden = false;
        document.querySelector("#status").textContent = "Complete. Download browser.json, then import it with bench-checker.py.";
    } catch (error) {
        document.querySelector("#status").textContent = String(error);
    } finally {
        button.disabled = false;
    }
});
