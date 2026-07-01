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

각 PR 보고에는 실행한 brief 이름, 수정 파일, 검증 명령, 다음 handoff를 남긴다.
