#!/usr/bin/env python3
"""Run a local productization smoke without live AI connectors."""

from __future__ import annotations

import http.client
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = ROOT / "specs" / "schemas"
TARGET_DIR = ROOT / "target" / "debug"
EXE = ".exe" if os.name == "nt" else ""
STAR_CONTROL = TARGET_DIR / f"star-control{EXE}"
STAR_DAEMON = TARGET_DIR / f"star-daemon{EXE}"


def run(command: list[str], *, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise AssertionError(
            f"command failed ({result.returncode}): {' '.join(command)}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def run_json(command: list[str], *, env: dict[str, str]) -> dict:
    result = run(command, env=env)
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise AssertionError(f"command did not return JSON: {' '.join(command)}") from exc


def assert_eq(actual, expected, label: str) -> None:
    if actual != expected:
        raise AssertionError(f"{label}: expected {expected!r}, got {actual!r}")


def free_loopback_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def http_get(port: int, path: str) -> dict:
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    try:
        connection.request("GET", path)
        response = connection.getresponse()
        body = response.read().decode("utf-8")
    finally:
        connection.close()
    if response.status != 200:
        raise AssertionError(f"GET {path} returned HTTP {response.status}: {body}")
    return json.loads(body)


def http_get_with_retry(port: int, path: str) -> dict:
    deadline = time.time() + 10
    last_error: Exception | None = None
    while time.time() < deadline:
        try:
            return http_get(port, path)
        except OSError as exc:
            last_error = exc
            time.sleep(0.05)
    raise AssertionError(f"GET {path} did not connect to 127.0.0.1:{port}") from last_error


def build_binaries() -> None:
    run(
        [
            "cargo",
            "build",
            "--quiet",
            "-p",
            "star-control-cli",
            "-p",
            "star-daemon",
            "--locked",
        ]
    )


def write_release_readiness(project: Path) -> None:
    release_dir = project / ".ai-runs" / "J-0001" / "release"
    release_dir.mkdir(parents=True, exist_ok=True)
    readiness = json.loads((ROOT / "examples" / "release-contracts" / "release-readiness.example.json").read_text())
    readiness["release_id"] = "productization-smoke"
    readiness["blockers"] = ["Local AI connector live execution", "Cloud AI connector live execution"]
    (release_dir / "release-readiness.json").write_text(
        json.dumps(readiness, indent=2) + "\n",
        encoding="utf-8",
    )


def assert_static_ui_surface() -> None:
    index = ROOT / "apps" / "star-control-ui" / "index.html"
    app = ROOT / "apps" / "star-control-ui" / "app.js"
    styles = ROOT / "apps" / "star-control-ui" / "styles.css"
    for path in [index, app, styles]:
        if not path.is_file():
            raise AssertionError(f"static UI file missing: {path}")
    app_text = app.read_text(encoding="utf-8")
    for token in ["fetch", "/projects", "release-readiness", "approve", "cancel", "resume"]:
        if token not in app_text:
            raise AssertionError(f"static UI app.js does not contain expected token {token!r}")


def assert_fake_cost_metric(project: Path) -> None:
    path = project / ".ai-runs" / "J-0001" / "provider-output" / "fake-default" / "cost-metric.json"
    if not path.is_file():
        raise AssertionError(f"fake provider cost metric missing: {path}")
    metric = json.loads(path.read_text(encoding="utf-8"))
    assert_eq(metric["job_id"], "J-0001", "fake cost metric job")
    assert_eq(metric["provider_instance_id"], "fake-default", "fake cost metric provider")
    assert_eq(metric["estimated_cost"], 0, "fake cost estimate")
    assert_eq(metric["currency"], "USD", "fake cost currency")
    assert_eq(metric["input_tokens"], 0, "fake input tokens")
    assert_eq(metric["output_tokens"], 0, "fake output tokens")


def assert_provider_request_redaction(project: Path) -> None:
    request_path = project / ".ai-runs" / "J-0001" / "provider-output" / "fake-default" / "request.json"
    request_text = request_path.read_text(encoding="utf-8")
    if "sk-test-secret" in request_text:
        raise AssertionError("fake provider request artifact leaked synthetic secret marker")
    if "[REDACTED]" not in request_text:
        raise AssertionError("fake provider request artifact did not contain redaction placeholder")
    report_path = (
        project
        / ".ai-runs"
        / "J-0001"
        / "audit"
        / "provider-redaction-fake-default-request-json.json"
    )
    if not report_path.is_file():
        raise AssertionError(f"provider request redaction report missing: {report_path}")
    report_text = report_path.read_text(encoding="utf-8")
    if "sk-test-secret" in report_text:
        raise AssertionError("provider redaction report leaked synthetic secret marker")


def assert_cli_report_redaction(project: Path, env: dict[str, str]) -> None:
    report_path = project / ".ai-runs" / "J-0001" / "reports" / "implement-report.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    report["risks"] = ["Authorization: Bearer sk-test-secret"]
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    result = run(
        [
            str(STAR_CONTROL),
            "report",
            "--project",
            str(project),
            "--job",
            "J-0001",
            "--stage",
            "implement",
            "--json",
        ],
        env=env,
    )
    if "sk-test-secret" in result.stdout:
        raise AssertionError("CLI report output leaked synthetic secret marker")
    output = json.loads(result.stdout)
    assert_eq(output["data"]["report"]["risks"][0], "[REDACTED]", "CLI report redaction")
    redaction_path = project / ".ai-runs" / "J-0001" / "audit" / "redaction-report-implement.json"
    if not redaction_path.is_file():
        raise AssertionError(f"CLI report redaction report missing: {redaction_path}")
    redaction_report = json.loads(redaction_path.read_text(encoding="utf-8"))
    assert_eq(redaction_report["artifact_path"], "reports/implement-report.json", "redaction artifact path")
    assert_eq(redaction_report["redacted"], True, "redaction report status")


def run_daemon_api_smoke(config_root: Path, project: Path) -> None:
    port = free_loopback_port()
    process = subprocess.Popen(
        [
            str(STAR_DAEMON),
            "api",
            "--config-root",
            str(config_root),
            "--schema-root",
            str(SCHEMA_ROOT),
            "--bind",
            f"127.0.0.1:{port}",
            "--max-requests",
            "2",
            "--project-id",
            "local",
            "--project-root",
            str(project),
            "--json",
        ],
        cwd=ROOT,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        projects = http_get_with_retry(port, "/projects")
        jobs = http_get(port, "/projects/local/jobs")
        stdout, stderr = process.communicate(timeout=10)
    finally:
        if process.poll() is None:
            process.kill()
    if process.returncode != 0:
        raise AssertionError(f"star-daemon api failed:\nstdout:\n{stdout}\nstderr:\n{stderr}")
    daemon_output = json.loads(stdout)
    assert_eq(daemon_output["data"]["handled_requests"], 2, "daemon handled requests")
    assert_eq(daemon_output["data"]["process"]["remote_exposure_enabled"], False, "remote exposure")
    assert_eq(daemon_output["data"]["process"]["local_ai_live_connector"], "disabled", "local AI")
    assert_eq(daemon_output["data"]["process"]["cloud_ai_live_connector"], "disabled", "cloud AI")
    assert_eq(projects["status"], "success", "projects status")
    assert_eq(jobs["data"]["jobs"][0]["job_id"], "J-0001", "jobs response")


def main() -> int:
    build_binaries()
    with tempfile.TemporaryDirectory(prefix="star-control-productization-smoke-") as temp:
        root = Path(temp)
        project = root / "project"
        config_root = root / "config"
        project.mkdir()
        config_root.mkdir()
        env = os.environ.copy()
        env["STAR_CONTROL_HOME"] = str(ROOT)

        run_output = run_json(
            [
                str(STAR_CONTROL),
                "run",
                "--project",
                str(project),
                "--request",
                "productization smoke Authorization: Bearer sk-test-secret",
                "--json",
            ],
            env=env,
        )
        assert_eq(run_output["data"]["state"], "IMPLEMENTED", "CLI fake run state")
        assert (project / ".ai-runs" / "J-0001" / "provider-output" / "fake-default" / "response.json").is_file()
        assert_fake_cost_metric(project)
        assert_provider_request_redaction(project)
        assert_cli_report_redaction(project, env)

        provider_health = run_json([str(STAR_CONTROL), "providers", "healthcheck", "--json"], env=env)
        assert_eq(provider_health["data"]["healthcheck_mode"], "offline_readiness", "provider health mode")
        assert_eq(provider_health["data"]["live_calls_performed"], False, "provider live calls")

        daemon_status = run_json(
            [
                str(STAR_DAEMON),
                "status",
                "--config-root",
                str(config_root),
                "--schema-root",
                str(SCHEMA_ROOT),
                "--json",
            ],
            env=env,
        )
        assert_eq(daemon_status["data"]["process"]["local_ai_live_connector"], "disabled", "daemon local AI")
        assert_eq(daemon_status["data"]["process"]["cloud_ai_live_connector"], "disabled", "daemon cloud AI")

        run_daemon_api_smoke(config_root, project)
        assert_static_ui_surface()

        tmp_artifact = project / ".ai-runs" / "J-0001" / "tmp" / "productization.tmp"
        tmp_artifact.parent.mkdir(parents=True, exist_ok=True)
        tmp_artifact.write_text("{ partial", encoding="utf-8")
        recovery = run_json(
            [
                str(STAR_CONTROL),
                "recover",
                "--project",
                str(project),
                "--job",
                "J-0001",
                "--action",
                "tmp-cleanup",
                "--dry-run",
                "--json",
            ],
            env=env,
        )
        assert_eq(recovery["data"]["mode"], "dry_run", "recovery mode")
        assert_eq(recovery["data"]["destructive_actions_performed"], False, "recovery destructive")

        write_release_readiness(project)
        release_policy = run_json(
            [
                str(STAR_CONTROL),
                "release",
                "--project",
                str(project),
                "--job",
                "J-0001",
                "--action",
                "deploy",
                "--dry-run",
                "--json",
            ],
            env=env,
        )
        assert_eq(
            release_policy["data"]["external_execution_policy"]["live_execution_enabled"],
            False,
            "release external live policy",
        )
        assert_eq(
            release_policy["data"]["external_execution_policy"]["blocked_operations"][0],
            "deploy_flow",
            "release external blocked operation",
        )
        release = run_json(
            [
                str(STAR_CONTROL),
                "release",
                "--project",
                str(project),
                "--job",
                "J-0001",
                "--action",
                "rollback-checklist",
                "--json",
            ],
            env=env,
        )
        assert_eq(release["data"]["mode"], "approved_execution", "release execution mode")
        assert_eq(release["data"]["external_actions_performed"], False, "release external effects")
        assert (project / ".ai-runs" / "J-0001" / "release" / "rollback-checklist-automation-result.json").is_file()

        sentinel = run_json([str(STAR_CONTROL), "sentinel", "selfcheck", "--json"], env=env)
        assert_eq(sentinel["status"], "success", "sentinel selfcheck")

    print(
        json.dumps(
            {
                "status": "success",
                "smoke": "productization_e2e",
                "local_ai_live_connector": "disabled",
                "cloud_ai_live_connector": "disabled",
                "provider_request_redaction_report": "written",
                "cli_report_redaction_report": "written",
                "external_release_policy": "reserved",
                "external_release_actions_performed": False,
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
