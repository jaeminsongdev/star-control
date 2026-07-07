const DEFAULT_STATE = {
  apiBase: "http://127.0.0.1:8787",
  projectId: "local",
  selectedJobId: null,
  jobs: [],
  detail: null,
  daemon: null
};

const state = { ...DEFAULT_STATE };

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
  return {
    jobId: job.job_id || "unknown",
    title: job.summary || job.title || job.job_id || "Untitled job",
    state: job.state || "UNKNOWN",
    stage: job.current_stage || "unknown",
    updatedAt: job.updated_at || ""
  };
}

export function actionAvailability(detail) {
  const runState = detail?.state?.state || detail?.job?.state || "";
  return {
    approve: waitingApproval(runState),
    resume: waitingApproval(runState),
    cancel: Boolean(runState) && !terminalState(runState)
  };
}

export function apiErrorMessage(error) {
  if (!error) return "Unknown API error";
  if (typeof error === "string") return error;
  return error.message || error.code || "Unknown API error";
}

async function apiGet(path) {
  const response = await fetch(endpoint(state.apiBase, path));
  return response.json();
}

async function apiPost(path, body) {
  const response = await fetch(endpoint(state.apiBase, path), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body || {})
  });
  return response.json();
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
    }
    render();
    setConnection("Connected", "");
  } catch (error) {
    setConnection(apiErrorMessage(error), "failed");
  }
}

async function loadDetail(jobId) {
  state.selectedJobId = jobId;
  const detail = await apiGet(
    `/projects/${encodeURIComponent(state.projectId)}/jobs/${encodeURIComponent(jobId)}`
  );
  const events = await apiGet(
    `/projects/${encodeURIComponent(state.projectId)}/jobs/${encodeURIComponent(jobId)}/events`
  );
  const release = await apiGet(
    `/projects/${encodeURIComponent(state.projectId)}/jobs/${encodeURIComponent(jobId)}/release-readiness`
  );
  state.detail = {
    ...detail.data,
    apiStatus: detail.status,
    apiError: detail.error,
    events: events.data?.events || [],
    releaseReadiness: release.status === "success" ? release.data?.readiness : null,
    releaseError: release.status === "success" ? null : release.error
  };
}

function render() {
  text("daemon-status", state.daemon?.status || "Unknown");
  text("job-count", String(state.jobs.length));
  text("selected-job", state.selectedJobId || "None");
  renderJobs();
  renderDetail();
  renderTimeline();
  renderActions();
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
        <strong>${escapeHtml(job.title)}</strong><br />
        <span>${escapeHtml(job.jobId)} · ${escapeHtml(job.stage)}</span>
      </span>
      <span class="state ${stateClass(job.state)}">${escapeHtml(job.state)}</span>
    `;
    button.addEventListener("click", async () => {
      await loadDetail(job.jobId);
      render();
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
  stateNode.textContent = runState.state || state.detail.apiStatus || "Loaded";
  detailNode.className = "detail-body";
  detailNode.innerHTML = `
    <h4 class="detail-title">${escapeHtml(job.request || job.title || state.selectedJobId)}</h4>
    <div class="detail-meta">
      ${metaBlock("Job", state.selectedJobId)}
      ${metaBlock("State", runState.state || "unknown")}
      ${metaBlock("Stage", runState.current_stage || "unknown")}
      ${metaBlock("Next action", runState.next_action || "inspect")}
      ${metaBlock("Approval", waitingApproval(runState.state) ? "Required" : "Not required")}
      ${metaBlock("Release", releaseLabel(state.detail.releaseReadiness, state.detail.releaseError))}
    </div>
  `;
}

function renderTimeline() {
  const timeline = element("timeline");
  timeline.innerHTML = "";
  const events = state.detail?.events || [];
  if (!events.length) {
    timeline.innerHTML = '<li><strong>No events</strong><br /><span>Event log is empty.</span></li>';
    return;
  }
  for (const event of events) {
    const item = document.createElement("li");
    item.innerHTML = `<strong>${escapeHtml(event.type || "EVENT")}</strong><br /><span>${escapeHtml(
      event.message || event.event_id || ""
    )}</span>`;
    timeline.appendChild(item);
  }
}

function renderActions() {
  const availability = actionAvailability(state.detail);
  element("approve-button").disabled = !availability.approve;
  element("resume-button").disabled = !availability.resume;
  element("cancel-button").disabled = !availability.cancel;
}

async function runAction(action) {
  if (!state.selectedJobId) return;
  const body =
    action === "approve"
      ? {
          response: "APPROVED",
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

function metaBlock(label, value) {
  return `<div><span>${escapeHtml(label)}</span><br /><strong>${escapeHtml(value || "-")}</strong></div>`;
}

function releaseLabel(readiness, error) {
  if (readiness?.status) return readiness.status;
  if (error?.code) return error.code;
  return "Not available";
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
