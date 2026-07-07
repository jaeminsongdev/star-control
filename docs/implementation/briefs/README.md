# Codex Implementation Briefs

## 목적

이 디렉터리는 Codex가 E01 이후 구현을 시작할 때 먼저 읽는 짧은 작업 brief 모음이다. 상세 계약은 기존 구현 문서가 기준이고, brief는 각 EPIC의 착수 범위와 검증 명령을 빠르게 고정한다.

## 권한 관계

```text
codex-work-queue-current.md = 전체 착수 순서의 최상위 기준
briefs/E*.md = 해당 EPIC 착수용 요약 entrypoint
상세 구현 문서 = 계약과 세부 규칙의 source of truth
schema/example = machine-readable source of truth
```

문서가 충돌하면 다음 순서로 판단한다.

```text
schema/example > 상세 구현 문서 > codex-work-queue-current.md > brief
```

단, EPIC 착수 범위와 수정 금지 파일이 brief와 work queue에서 다르게 보이면 작업을 멈추고 사람에게 확인한다.

## brief 목록

```text
E01-schema-validator.md
E02-state-store.md
E03-artifact-layout-writer.md
E04-provider-registry.md
E05-fake-provider-adapter.md
E06-router-engine.md
E07-execution-engine.md
E08-cli-fake-flow.md
E09-star-sentinel-p0.md
E10-validation-engine.md
E11-integration-smoke.md
E12-cloud-provider-preflight.md
E13-cloud-cli-transport.md
E14-cloud-provider-conformance.md
E15-openai-compatible-parser.md
E16-openai-compatible-request-builder.md
E17-cloud-api-offline-fixture.md
E18-cloud-api-transport-boundary.md
E19-cloud-api-live-approval-gate.md
E20-cli-control-commands.md
E21-daemon-queue-skeleton.md
E22-api-read-only.md
E23-ui-read-only-view.md
E24-api-control-mutations.md
E25-ui-browser-control-shell.md
E26-security-redaction-utility.md
E27-observability-audit-event-writer.md
E28-cost-metric-budget-guard.md
E29-provider-conformance-hardening.md
E30-state-recovery-inspection.md
E31-release-readiness-writer.md
E32-release-readiness-api-read.md
E33-release-version-consistency-checker.md
E34-release-evidence-file-discovery.md
E35-release-profile-readiness-integration.md
E36-release-readiness-ui-read.md
E37-release-readiness-cli-read.md
E38-release-review-pack-foundation.md
E39-recovery-command-surface.md
E40-final-m9-readiness-audit.md
E41-final-completion-audit.md
E42-final-audit-evidence.md
E43-stacked-pr-readiness-coordination.md
E44-cli-providers-read-only.md
E45-cli-sentinel-command-group.md
E46-final-evidence-refresh.md
E47-stacked-merge-procedure.md
E48-provider-offline-readiness-healthcheck.md
E49-daemon-app-process-surface.md
E50-local-http-api-server-surface.md
E51-static-browser-ui-app-surface.md
E52-daemon-http-control-audit-integration.md
E53-recovery-action-dry-run-approval-surface.md
E54-release-automation-dry-run-approval-surface.md
E55-daemon-queue-scheduler-tick.md
E56-daemon-local-process-scheduler-executor.md
E57-recovery-action-executor.md
E58-release-automation-executor.md
E59-artifact-replacement-source-selection.md
E60-productization-e2e-smoke.md
E61-redaction-report-artifact-storage.md
E62-provider-cost-metric-sidecar-integration.md
E63-cloud-hard-budget-enforcement.md
E64-cli-report-redaction-artifact-wiring.md
E65-provider-output-redaction-artifact-wiring.md
E66-external-release-execution-policy.md
E67-final-readiness-pre-live-ai.md
```

## 사용 방법

Codex는 새 EPIC에 들어갈 때 다음 순서로 읽는다.

```text
1. AGENTS.md
2. README.md
3. docs/implementation/README.md
4. docs/implementation/codex-long-run-workflow.md
5. docs/implementation/codex-work-queue-current.md
6. docs/implementation/briefs/E*.md
7. brief에 적힌 상세 문서
```

## 검증 명령 기준

brief의 검증 명령은 logical slice 완료 시 실행할 기준이다. 한 줄 또는 한 파일을 수정할 때마다 full validation을 반복하지 않는다.

Windows local 기본 검증은 아래 명령이다.

```text
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
```

`scripts/test.ps1`는 `scripts/ci/run_all.py` wrapper이므로 같은 검증 단계에서 두 명령을 중복 실행하지 않는다. Windows가 아닌 환경에서는 `python scripts/ci/run_all.py`를 대신 사용한다.

각 PR 보고에는 실행한 brief 이름, 수정 파일, 검증 명령, 다음 handoff를 남긴다.
