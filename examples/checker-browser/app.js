import init, { PreparedModel } from "./web/zincite_check.js";

// This application owns job IDs and the start/finish array positions for model.fzn.
const jobs = [
    { id: "1", name: "Cut panels", duration: 3 },
    { id: "2", name: "Assemble panels", duration: 2 },
];
const editor = document.querySelector("#editor");
const form = document.querySelector("#schedule-form");
const omitStarts = document.querySelector("#omit-starts");

start().catch(showError);

async function start() {
    // Loading/validation is separate from assignment, checking and presentation.
    await init();
    const response = await fetch("./schedule.json");
    if (!response.ok) throw new Error(`Could not load the schedule (${response.status}).`);
    const model = new PreparedModel(await response.text());
    const update = () => {
        try {
            const state = readSchedule();
            const result = checkSchedule(model, state);
            renderSchedule(state, result.findings);
            renderReport(result);
        } catch (error) {
            showError(error);
        }
    };
    form.addEventListener("submit", event => event.preventDefault());
    form.addEventListener("input", update);
    for (const button of document.querySelectorAll("[data-starts]")) {
        button.addEventListener("click", () => {
            button.dataset.starts.split(",").forEach((value, index) => {
                document.querySelector(`#start-${jobs[index].id}`).value = value;
            });
            omitStarts.checked = false;
            update();
        });
    }
    editor.disabled = false;
    update();
}

function readSchedule() {
    return {
        starts: jobs.map(job => document.querySelector(`#start-${job.id}`).value),
        omitStarts: omitStarts.checked,
    };
}

function assignmentFor(state) {
    return {
        entries: state.omitStarts ? [] : [{
            name: "start",
            value: {
                kind: "array",
                value: state.starts.map(value => ({ kind: "integer", value })),
            },
        }],
    };
}

function checkSchedule(model, state) {
    const assignment = assignmentFor(state);
    const assignmentJson = JSON.stringify(assignment);
    const reportJson = model.check(assignmentJson);
    const report = JSON.parse(reportJson);
    // Keep each finding, including those sharing an application identifier.
    const findings = report.violations.map(violation => ({
        jobs: affectedJobs(violation.check),
        message: feedbackMessage(violation),
    }));
    return { assignment, report, findings };
}

function affectedJobs(check) {
    let entities;
    if (check.kind === "constraint") {
        entities = check.constraint.metadata.application?.entities;
    } else if (check.kind === "domain") {
        const variable = check.variable;
        const application = variable.metadata.application;
        const position = variable.name === application?.id
            ? variable.position
            : variable.aliases.find(alias => alias.name === application?.id)?.position;
        entities = position ? [application.entities[position - 1]] : undefined;
    }
    if (!entities?.length || entities.some(id => !jobs.some(job => job.id === id))) {
        throw new Error("A finding could not be connected to a workshop job.");
    }
    return entities;
}

// Choose wording from CheckRef, ViolationKind and typed application metadata.
function feedbackMessage({ check, details }) {
    if (check.kind === "domain" && details.kind === "domain"
        && check.domain.kind === "interval" && details.value.kind === "integer") {
        const id = check.variable.metadata.application?.id;
        if (id === "start" || id === "finish") {
            return `${id === "start" ? "Start" : "Finish"} time ${details.value.value} must be between ${check.domain.lower} and ${check.domain.upper}.`;
        }
    }
    if (check.kind === "constraint") {
        const id = check.constraint.metadata.application?.id;
        if (details.kind === "linear") {
            switch (id) {
                case "finish":
                    return `Elapsed time ${details.sum} differs from the job duration ${details.bound}.`;
                case "precedence":
                    return `Cutting finishes ${details.sum} hours after assembly starts; the allowed gap is ${details.bound}.`;
                case "start_budget":
                    return `The start times add up to ${details.sum}, exceeding the budget of ${details.bound}.`;
            }
        }
        if (id === "early_start" && details.kind === "reified_less_equal") {
            return `The early-start flag ${details.result.value.value} disagrees with start ${details.left.value.value} and threshold ${details.right.value.value}.`;
        }
    }
    throw new Error("The workshop page does not recognise this scheduling finding.");
}

function renderSchedule(state, findings) {
    jobs.forEach((job, index) => {
        const row = document.querySelector(`#job-${job.id}`);
        const bar = row.querySelector(".bar");
        const start = Number(state.starts[index]);
        // Fixed durations position the drawing; only the checker judges feasibility.
        const finish = start + job.duration;
        const count = findings.filter(finding => finding.jobs.includes(job.id)).length;
        row.classList.toggle("has-findings", count > 0);
        row.querySelector(".unsupplied").hidden = !state.omitStarts;
        bar.hidden = state.omitStarts;
        bar.style.left = `${start / 12 * 100}%`;
        bar.style.width = `${job.duration / 12 * 100}%`;
        const hours = state.omitStarts ? "Start not supplied" : `Hours ${start}–${finish}`;
        row.querySelector(".track").setAttribute("aria-label", `${job.name}: ${hours.toLowerCase()}`);
        document.querySelector(`#hour-${job.id}`).value = state.starts[index];
        document.querySelector(`#job-summary-${job.id}`).textContent = hours;
        document.querySelector(`#job-findings-${job.id}`).textContent = state.omitStarts
            ? "Waiting for start times" : count ? `${count} ${count === 1 ? "finding" : "findings"}` : "No findings";
    });
}

function renderReport({ assignment, report, findings }) {
    const complete = report.completeness.complete;
    const unresolved = report.completeness.unresolved_variables.length;
    const blocked = report.completeness.blocked_checks.length;
    const titles = {
        valid: "This schedule works",
        invalid: `${findings.length} ${findings.length === 1 ? "finding needs" : "findings need"} attention`,
        incomplete: "Supply start times to check the schedule",
    };
    document.querySelector(".feedback").dataset.status = report.status;
    document.querySelector("#feedback-title").textContent = titles[report.status];
    document.querySelector("#feedback-summary").textContent = complete
        ? "All checks completed for these start times. Edit either job to check another plan."
        : "The check needs more values. Clear “Omit start times” or choose an example plan to continue.";
    document.querySelector("#findings").replaceChildren(...findings.map(finding => {
        const item = document.createElement("li");
        const names = document.createElement("span");
        names.className = "finding-jobs";
        names.textContent = finding.jobs.map(id => jobs.find(job => job.id === id).name).join(" + ");
        item.append(names, document.createTextNode(finding.message));
        return item;
    }));
    document.querySelector("#completeness").textContent = `Complete: ${complete}. Unresolved values: ${unresolved}. Blocked checks: ${blocked}.`;
    document.querySelector("#assignment").textContent = JSON.stringify(assignment, null, 2);
    document.querySelector("#report").textContent = JSON.stringify(report, null, 2);
}

function showError(error) {
    document.querySelector(".feedback").dataset.status = "error";
    document.querySelector("#feedback-title").textContent = "The schedule could not be checked";
    document.querySelector("#feedback-summary").textContent = String(error);
    document.querySelector("#findings").replaceChildren();
    document.querySelector("#assignment").textContent = "";
    document.querySelector("#report").textContent = "";
    document.querySelector("#completeness").textContent = "No current report.";
    for (const row of document.querySelectorAll(".job")) {
        row.classList.remove("has-findings");
        row.querySelector(".job-findings").textContent = "Not checked";
    }
}
