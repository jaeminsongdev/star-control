const DEFAULT_STATE = {
  apiBase: "http://127.0.0.1:8787",
  projectId: "local",
  selectedJobId: null,
  jobs: [],
  detail: null,
  daemon: null
};

const state = { ...DEFAULT_STATE };
const API_TIMEOUT_MS = 3500;

export function endpoint(base, path) {
  return `${base.replace(/\/$/, "")}${path}`;
}

export function stateClass(rawState) {
  return String(rawState || "unknown").toLowerCase();
}

export function terminalState(rawState) {
  return ["done", "failed", "blocked", "cancelled"].includes(stateClass(rawState));
}

export function waitingApproval(rawState) {
  return stateClass(rawState) === "waiting_approval";
}

export function summarizeJob(job) {
  const stateValue = job.state || "UNKNOWN";
  return {
    jobId: job.job_id || "unknown",
    title: job.summary || job.title || job.job_id || "Untitled job",
    state: stateValue,
    stage: job.current_stage || "unknown",
    updatedAt: job.updated_at || "",
    runDir: job.run_dir || "",
    approvalRequired: waitingApproval(stateValue),
    nextAction: job.next_action || nextActionForState(stateValue)
  };
}

export function actionAvailability(detail) {
  const runState = detail?.state?.state || detail?.job?.state || "";
  const nextAction = String(detail?.state?.next_action || "").toLowerCase();
  const waiting = waitingApproval(runState);
  return {
    approve: waiting && nextAction !== "resume",
    resume: waiting && (nextAction === "resume" || nextAction === ""),
    cancel: Boolean(runState) && !terminalState(runState)
  };
}

export function apiErrorMessage(error) {
  if (!error) return "Unknown API error";
  if (typeof error === "string") return error;
  return error.message || error.code || "Unknown API error";
}

export function eventKind(event) {
  const haystack = `${event?.type || ""} ${event?.message || ""}`.toLowerCase();
  if (haystack.includes("approval")) return "approval";
  if (haystack.includes("validation") || haystack.includes("validate")) return "validation";
  if (haystack.includes("provider") || haystack.includes("worker")) return "provider";
  if (haystack.includes("error") || haystack.includes("failed") || haystack.includes("block")) {
    return "error";
  }
  if (haystack.includes("tool") || haystack.includes("sentinel")) return "tool";
  return "system";
}

export function artifactEntries(artifacts) {
  if (!artifacts) return [];
  if (Array.isArray(artifacts)) {
    return artifacts.flatMap((value, index) => artifactValueEntries(`artifact_${index + 1}`, value));
  }
  if (typeof artifacts === "object") {
    return Object.entries(artifacts).flatMap(([section, value]) =>
      artifactValueEntries(section, value)
    );
  }
  return artifactValueEntries("artifact", artifacts);
}

function artifactValueEntries(section, value) {
  if (value == null) return [];
  if (Array.isArray(value)) {
    return value.flatMap((item) => artifactValueEntries(section, item));
  }
  if (typeof value === "object") {
    const path = value.path || value.file || value.artifact_path || value.href || JSON.stringify(value);
    return [{ section, path: String(path), kind: classifyArtifact(section, path) }];
  }
  return [{ section, path: String(value), kind: classifyArtifact(section, value) }];
}

async function apiGet(path) {
  return apiFetch(path);
}

async function optionalApiGet(path) {
  try {
    return await apiGet(path);
  } catch (error) {
    return {
      status: "failed",
      data: null,
      error: {
        code: "fetch_failed",
        message: apiErrorMessage(error)
      }
    };
  }
}

async function apiPost(path, body) {
  return apiFetch(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body || {})
  });
}

async function apiFetch(path, options = {}) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), API_TIMEOUT_MS);
  try {
    const response = await fetch(endpoint(state.apiBase, path), {
      ...options,
      signal: controller.signal
    });
    return response.json();
  } finally {
    clearTimeout(timer);
  }
}

async function refreshAll() {
  setConnection("Connecting", "muted");
  try {
    const daemon = await apiGet("/daemon/state");
    const jobs = await apiGet(`/projects/${encodeURIComponent(state.projectId)}/jobs`);
    if (daemon.status === "failed") throw new Error(apiErrorMessage(daemon.error));
    if (jobs.status === "failed") throw new Error(apiErrorMessage(jobs.error));
    state.daemon = daemon.data.daemon_state;
    state.jobs = jobs.data.jobs || [];
    if (!state.selectedJobId && state.jobs.length > 0) {
      state.selectedJobId = state.jobs[0].job_id;
    }
    if (state.selectedJobId) {
      await loadDetail(state.selectedJobId);
    } else {
      state.detail = null;
    }
    render();
    setConnection("Connected", "");
  } catch (error) {
    setConnection(apiErrorMessage(error), "failed");
    render();
  }
}

async function loadDetail(jobId) {
  state.selectedJobId = jobId;
  const detail = await apiGet(
    `/projects/${encodeURIComponent(state.projectId)}/jobs/${encodeURIComponent(jobId)}`
  );
  if (detail.status === "failed") throw new Error(apiErrorMessage(detail.error));
  const currentStage = detail.data?.state?.current_stage || "report";
  const [events, release, report] = await Promise.all([
    optionalApiGet(
      `/projects/${encodeURIComponent(state.projectId)}/jobs/${encodeURIComponent(jobId)}/events`
    ),
    optionalApiGet(
      `/projects/${encodeURIComponent(state.projectId)}/jobs/${encodeURIComponent(
        jobId
      )}/release-readiness`
    ),
    optionalApiGet(
      `/projects/${encodeURIComponent(state.projectId)}/jobs/${encodeURIComponent(
        jobId
      )}/report?stage=${encodeURIComponent(currentStage)}`
    )
  ]);
  state.detail = {
    ...detail.data,
    apiStatus: detail.status,
    apiError: detail.error,
    events: events.data?.events || [],
    eventsError: events.status === "success" ? null : events.error,
    report: report.status === "success" ? report.data?.report : null,
    reportPath: report.status === "success" ? report.data?.report_path : null,
    reportError: report.status === "success" ? null : report.error,
    releasePath: release.status === "success" ? release.data?.readiness_path : null,
    releaseReadiness: release.status === "success" ? release.data?.readiness : null,
    releaseError: release.status === "success" ? null : release.error
  };
}

function render() {
  const job = selectedJobView();
  const runState = state.detail?.state || {};
  const availability = actionAvailability(state.detail);

  text("daemon-status", state.daemon?.status || "Unknown");
  text("job-count", String(state.jobs.length));
  text("selected-job", state.selectedJobId || "None");
  text("page-title", job?.title || "No job selected");
  text("thread-state", runState.state || job?.state || "Unknown");
  text("thread-stage", runState.current_stage || job?.stage || "unknown");
  text("thread-next-action", runState.next_action || job?.nextAction || "inspect");
  text("thread-approval", waitingApproval(runState.state || job?.state) ? "Required" : "Not required");

  renderJobs();
  renderDetail();
  renderTimeline();
  renderActions();
  renderRelease();
  renderArtifacts();
}

function renderJobs() {
  const list = element("job-list");
  const status = element("jobs-state");
  list.innerHTML = "";
  status.textContent = state.jobs.length ? `${state.jobs.length} loaded` : "No jobs";
  if (state.jobs.length === 0) {
    list.innerHTML = '<div class="detail-body empty-state">No jobs returned by the API.</div>';
    return;
  }
  for (const job of state.jobs.map(summarizeJob)) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `job-row ${job.jobId === state.selectedJobId ? "selected" : ""}`;
    button.innerHTML = `
      <span>
        <strong>${escapeHtml(job.title)}</strong>
        <small>${escapeHtml(job.jobId)} / ${escapeHtml(job.stage)}</small>
        <small>${escapeHtml(job.nextAction)}</small>
      </span>
      <span class="state ${stateClass(job.state)}">${escapeHtml(job.state)}</span>
    `;
    button.addEventListener("click", async () => {
      setConnection("Loading job", "muted");
      try {
        await loadDetail(job.jobId);
        render();
        setConnection("Connected", "");
      } catch (error) {
        setConnection(apiErrorMessage(error), "failed");
        render();
      }
    });
    list.appendChild(button);
  }
}

function renderDetail() {
  const detailNode = element("job-detail");
  const stateNode = element("detail-state");
  if (!state.detail || !state.selectedJobId) {
    detailNode.className = "detail-body empty-state";
    detailNode.textContent =
      "Select a job to inspect timeline, artifacts, approval, and release readiness.";
    stateNode.textContent = "No job selected";
    return;
  }
  const job = state.detail.job || {};
  const runState = state.detail.state || {};
  const report = state.detail.report || {};
  const title = job.request_text || job.request || job.title || state.selectedJobId;
  const blockedReason = runState.blocked_reason || report.blocked_reason || "-";
  stateNode.textContent = runState.state || state.detail.apiStatus || "Loaded";
  detailNode.className = "detail-body";
  detailNode.innerHTML = `
    <h4 class="detail-title">${escapeHtml(title)}</h4>
    <div class="meta-grid">
      ${metaBlock("Job", state.selectedJobId)}
      ${metaBlock("State", runState.state || "unknown")}
      ${metaBlock("Stage", runState.current_stage || "unknown")}
      ${metaBlock("Provider", runState.active_provider || "none")}
      ${metaBlock("Latest event", runState.latest_event_id || latestEventId() || "-")}
      ${metaBlock("Next action", runState.next_action || "inspect")}
      ${metaBlock("Approval", waitingApproval(runState.state) ? "Required" : "Not required")}
      ${metaBlock("Blocked reason", blockedReason)}
    </div>
  `;
}

function renderTimeline() {
  const timeline = element("timeline");
  timeline.innerHTML = "";
  const messages = threadMessages();
  if (!messages.length) {
    timeline.innerHTML = emptyThreadMessage();
    text("thread-note", "No events loaded");
    return;
  }
  text("thread-note", `${messages.length} thread item${messages.length === 1 ? "" : "s"}`);
  for (const message of messages) {
    const item = document.createElement("li");
    item.className = `message message-${message.kind}`;
    item.innerHTML = `
      <span class="message-icon" aria-hidden="true">${escapeHtml(message.icon)}</span>
      <div class="message-body">
        <div class="message-title">
          <strong>${escapeHtml(message.title)}</strong>
          <span class="message-meta">${escapeHtml(message.meta)}</span>
        </div>
        <p>${escapeHtml(message.body)}</p>
        ${message.details ? detailsBlock(message.detailsLabel || "Details", message.details) : ""}
      </div>
    `;
    timeline.appendChild(item);
  }
}

function renderActions() {
  const availability = actionAvailability(state.detail);
  const runState = state.detail?.state?.state || "";
  setButtonState("approve-button", availability.approve, reasonForAction("approve", runState));
  setButtonState("resume-button", availability.resume, reasonForAction("resume", runState));
  setButtonState("cancel-button", availability.cancel, reasonForAction("cancel", runState));
  text("action-hint", actionHint(availability, runState));
}

function renderRelease() {
  const node = element("release-detail");
  const stateNode = element("release-state");
  const readiness = state.detail?.releaseReadiness;
  const error = state.detail?.releaseError;
  if (!state.detail) {
    stateNode.textContent = "Not available";
    node.className = "compact-list empty-state";
    node.textContent = "Select a job to inspect release readiness.";
    return;
  }
  if (!readiness) {
    stateNode.textContent = error?.code || "Not available";
    node.className = "compact-list empty-state";
    node.textContent = apiErrorMessage(error || "Release readiness artifact not loaded.");
    return;
  }
  stateNode.textContent = readiness.status || "loaded";
  node.className = "compact-list";
  const checks = Array.isArray(readiness.checks) ? readiness.checks : [];
  const blockers = Array.isArray(readiness.blockers) ? readiness.blockers : [];
  const approvals = Array.isArray(readiness.approvals) ? readiness.approvals : [];
  node.innerHTML = [
    compactRow("Release", `${readiness.release_id || "-"} / ${readiness.version || "-"}`),
    compactRow("Target", readiness.target || "-"),
    compactRow("Checks", checks.length ? checks.map(checkLabel).join(", ") : "No checks"),
    compactRow("Blockers", blockers.length ? blockers.join("; ") : "None"),
    compactRow("Approvals", approvals.length ? `${approvals.length} recorded` : "None"),
    state.detail.releasePath ? compactRow("Path", state.detail.releasePath, true) : ""
  ].join("");
}

function renderArtifacts() {
  const node = element("artifact-detail");
  const stateNode = element("artifact-state");
  if (!state.detail) {
    stateNode.textContent = "No artifacts";
    node.className = "compact-list empty-state";
    node.textContent = "Select a job to inspect evidence paths.";
    return;
  }
  const entries = allArtifactEntries();
  stateNode.textContent = entries.length ? `${entries.length} paths` : "No artifacts";
  if (!entries.length) {
    node.className = "compact-list empty-state";
    node.textContent = "No artifact paths loaded.";
    return;
  }
  node.className = "compact-list";
  node.innerHTML = entries
    .map((entry) => compactRow(`${entry.kind} / ${entry.section}`, entry.path, true))
    .join("");
}

async function runAction(action) {
  if (!state.selectedJobId) return;
  const body =
    action === "approve"
      ? {
          response: "approved",
          reason: element("approval-reason").value,
          constraints: []
        }
      : {};
  const result = await apiPost(
    `/projects/${encodeURIComponent(state.projectId)}/jobs/${encodeURIComponent(
      state.selectedJobId
    )}/${action}`,
    body
  );
  element("action-output").textContent = JSON.stringify(result, null, 2);
  await refreshAll();
}

function threadMessages() {
  if (!state.detail || !state.selectedJobId) return [];
  const job = state.detail.job || {};
  const runState = state.detail.state || {};
  const report = state.detail.report || {};
  const messages = [];
  messages.push({
    kind: "request",
    icon: "U",
    title: "User request",
    meta: state.selectedJobId,
    body: job.request_text || job.request || job.title || "No request text available.",
    detailsLabel: "Job artifact",
    details: {
      entrypoint: job.entrypoint || null,
      project_root: job.project_root || null,
      created_at: job.created_at || null,
      updated_at: job.updated_at || null
    }
  });
  messages.push({
    kind: waitingApproval(runState.state) ? "approval" : "system",
    icon: waitingApproval(runState.state) ? "!" : "S",
    title: "Current state",
    meta: runState.current_stage || "unknown stage",
    body: stateSummary(runState, report),
    detailsLabel: "Run state",
    details: runState
  });
  for (const event of state.detail.events || []) {
    const kind = eventKind(event);
    messages.push({
      kind,
      icon: iconForKind(kind),
      title: event.type || "Event",
      meta: event.created_at || event.event_id || "",
      body: event.message || event.event_id || "Event recorded.",
      detailsLabel: "Event JSON",
      details: event
    });
  }
  if (state.detail.report || state.detail.reportError) {
    const reportError = state.detail.reportError;
    messages.push({
      kind: reportError ? "error" : "validation",
      icon: reportError ? "E" : "V",
      title: reportError ? "Report unavailable" : "Validation report",
      meta: state.detail.reportPath || report.stage || "",
      body: reportError
        ? apiErrorMessage(reportError)
        : report.verdict || report.status || report.next_step || "Report artifact loaded.",
      detailsLabel: "Report JSON",
      details: reportError || report
    });
  }
  const entries = allArtifactEntries();
  if (entries.length) {
    messages.push({
      kind: "tool",
      icon: "A",
      title: "Evidence artifacts",
      meta: `${entries.length} path${entries.length === 1 ? "" : "s"}`,
      body: entries
        .slice(0, 3)
        .map((entry) => `${entry.kind}: ${entry.path}`)
        .join(" / "),
      detailsLabel: "Artifact paths",
      details: entries
    });
  }
  return messages;
}

function stateSummary(runState, report) {
  const parts = [
    `State is ${runState.state || "unknown"}`,
    `stage is ${runState.current_stage || "unknown"}`,
    `next action is ${runState.next_action || "inspect"}`
  ];
  const blocked = runState.blocked_reason || report.blocked_reason;
  if (blocked) parts.push(`blocked reason: ${blocked}`);
  if (waitingApproval(runState.state)) parts.push("approval is required before continuing");
  return `${parts.join(", ")}.`;
}

function allArtifactEntries() {
  const entries = [
    ...artifactEntries(state.detail?.state?.artifacts),
    ...artifactEntries(state.detail?.job?.artifacts),
    ...artifactEntries(state.detail?.report?.artifacts)
  ];
  if (state.detail?.reportPath) {
    entries.push({ section: "report", path: state.detail.reportPath, kind: "validation" });
  }
  if (state.detail?.releasePath) {
    entries.push({ section: "release", path: state.detail.releasePath, kind: "release" });
  }
  return dedupeArtifacts(entries);
}

function dedupeArtifacts(entries) {
  const seen = new Set();
  return entries.filter((entry) => {
    const key = entry.path;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function classifyArtifact(section, path) {
  const value = `${section || ""} ${path || ""}`.toLowerCase();
  if (value.includes("provider-output")) return "provider";
  if (value.includes("validation") || value.includes("report") || value.includes("sentinel")) {
    return "validation";
  }
  if (value.includes("approval")) return "approval";
  if (value.includes("review-pack")) return "review";
  if (value.includes("release")) return "release";
  return "artifact";
}

function nextActionForState(rawState) {
  if (waitingApproval(rawState)) return "approve";
  if (terminalState(rawState)) return "inspect";
  return "continue";
}

function selectedJobView() {
  if (state.detail?.job || state.detail?.state) {
    const job = state.detail.job || {};
    const runState = state.detail.state || {};
    return {
      title: job.request_text || job.request || job.title || state.selectedJobId,
      state: runState.state,
      stage: runState.current_stage,
      nextAction: runState.next_action
    };
  }
  return state.jobs.map(summarizeJob).find((job) => job.jobId === state.selectedJobId) || null;
}

function latestEventId() {
  const events = state.detail?.events || [];
  return events.length ? events[events.length - 1].event_id : null;
}

function reasonForAction(action, runState) {
  if (!state.selectedJobId) return "No job selected";
  if (action === "cancel" && terminalState(runState)) return "Terminal jobs cannot be cancelled";
  if ((action === "approve" || action === "resume") && !waitingApproval(runState)) {
    return "Job is not waiting for approval";
  }
  return "";
}

function actionHint(availability, runState) {
  if (!state.selectedJobId) return "Select a job to inspect available actions.";
  const nextAction = String(state.detail?.state?.next_action || "").toLowerCase();
  if (waitingApproval(runState) && nextAction === "resume") {
    return "Approval response is recorded. Resume is enabled when ready.";
  }
  if (availability.approve || availability.resume) {
    return "Approval is required. Review evidence, approve, then resume when ready.";
  }
  if (availability.cancel) return `Job is ${runState || "active"} and can be cancelled.`;
  return "This job is terminal or has no available control action.";
}

function setButtonState(id, enabled, reason) {
  const button = element(id);
  button.disabled = !enabled;
  button.title = enabled ? "" : reason;
}

function metaBlock(label, value) {
  return `<div><span>${escapeHtml(label)}</span><strong>${escapeHtml(value || "-")}</strong></div>`;
}

function compactRow(label, value, path = false) {
  const className = path ? "artifact-path" : "";
  const tag = path ? "code" : "strong";
  return `<div class="compact-row"><span>${escapeHtml(label)}</span><${tag} class="${className}">${escapeHtml(
    value || "-"
  )}</${tag}></div>`;
}

function checkLabel(check) {
  if (!check || typeof check !== "object") return String(check || "");
  return `${check.name || "check"}:${check.status || "unknown"}`;
}

function detailsBlock(label, value) {
  return `<details><summary>${escapeHtml(label)}</summary><pre class="json-snippet">${escapeHtml(
    JSON.stringify(value, null, 2)
  )}</pre></details>`;
}

function emptyThreadMessage() {
  return `
    <li class="message message-system">
      <span class="message-icon" aria-hidden="true">S</span>
      <div class="message-body">
        <div class="message-title">
          <strong>No job selected</strong>
          <span class="message-meta">Idle</span>
        </div>
        <p>Connect to the loopback API and select a job to inspect the run thread.</p>
      </div>
    </li>
  `;
}

function iconForKind(kind) {
  return {
    approval: "!",
    validation: "V",
    provider: "P",
    error: "E",
    tool: "T",
    request: "U",
    system: "S"
  }[kind] || "S";
}

function setConnection(message, mode) {
  const node = element("connection-state");
  node.textContent = message;
  node.className = `status-pill ${mode || ""}`.trim();
}

function text(id, value) {
  element(id).textContent = value;
}

function element(id) {
  return document.getElementById(id);
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function initBrowserApp() {
  const form = element("connection-form");
  element("api-base").value = localStorage.getItem("star-control.apiBase") || state.apiBase;
  element("project-id").value = localStorage.getItem("star-control.projectId") || state.projectId;
  state.apiBase = element("api-base").value.trim() || DEFAULT_STATE.apiBase;
  state.projectId = element("project-id").value.trim() || DEFAULT_STATE.projectId;
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    state.apiBase = element("api-base").value.trim() || DEFAULT_STATE.apiBase;
    state.projectId = element("project-id").value.trim() || DEFAULT_STATE.projectId;
    localStorage.setItem("star-control.apiBase", state.apiBase);
    localStorage.setItem("star-control.projectId", state.projectId);
    state.selectedJobId = null;
    await refreshAll();
  });
  element("refresh-button").addEventListener("click", refreshAll);
  element("approve-button").addEventListener("click", () => runAction("approve"));
  element("resume-button").addEventListener("click", () => runAction("resume"));
  element("cancel-button").addEventListener("click", () => runAction("cancel"));
  refreshAll();
}

if (typeof document !== "undefined") {
  initBrowserApp();
}
