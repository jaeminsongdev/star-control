import assert from "node:assert/strict";
import test from "node:test";
import {
  actionAvailability,
  apiErrorMessage,
  endpoint,
  stateClass,
  summarizeJob,
  terminalState,
  waitingApproval
} from "../app.js";

test("endpoint normalizes one trailing slash", () => {
  assert.equal(endpoint("http://127.0.0.1:8787/", "/daemon/state"), "http://127.0.0.1:8787/daemon/state");
});

test("job summary maps API fields to UI row fields", () => {
  assert.deepEqual(
    summarizeJob({
      job_id: "J-0001",
      summary: "Implement feature",
      state: "WAITING_APPROVAL",
      current_stage: "validate",
      updated_at: "unix:1"
    }),
    {
      jobId: "J-0001",
      title: "Implement feature",
      state: "WAITING_APPROVAL",
      stage: "validate",
      updatedAt: "unix:1"
    }
  );
});

test("state helpers classify approval and terminal states", () => {
  assert.equal(stateClass("WAITING_APPROVAL"), "waiting_approval");
  assert.equal(waitingApproval("WAITING_APPROVAL"), true);
  assert.equal(terminalState("DONE"), true);
  assert.equal(terminalState("IMPLEMENTED"), false);
});

test("action availability follows API control contract", () => {
  assert.deepEqual(actionAvailability({ state: { state: "WAITING_APPROVAL" } }), {
    approve: true,
    resume: true,
    cancel: true
  });
  assert.deepEqual(actionAvailability({ state: { state: "DONE" } }), {
    approve: false,
    resume: false,
    cancel: false
  });
});

test("API error messages prefer structured messages", () => {
  assert.equal(apiErrorMessage({ code: "MissingArtifact", message: "report missing" }), "report missing");
  assert.equal(apiErrorMessage("plain error"), "plain error");
});
