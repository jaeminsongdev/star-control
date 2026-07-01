# 0005 Full Implementation Defaults

## 상태

Accepted.

## 결정

Star-Control 완전 구현은 v0 fake flow에서 멈추지 않는다. v0는 첫 번째 반복 가능한 검증 마일스톤이며, 이후 local provider, cloud provider, daemon/API/UI, 운영 안정화, release readiness로 확장한다.

완전 구현의 기본값은 다음으로 고정한다.

```text
core crate namespace: star-control-*
Star Sentinel package: packages/star-sentinel
canonical StateStore: file-based .ai-runs artifacts
provider model: manifest + instance + capability + transport + adapter
user surface order: CLI -> daemon -> API -> UI
provider order: fake -> local_process -> local OpenAI-compatible/local server -> cloud CLI -> cloud API
```

## package naming

Core runtime crate는 `star-control-*` namespace를 사용한다.

초기 core 후보:

```text
packages/star-control-schema
packages/star-control-state
packages/star-control-provider
packages/star-control-router
packages/star-control-execution
packages/star-control-validation
packages/star-control-report
packages/star-control-cli
```

장기 surface 후보:

```text
packages/star-control-daemon
packages/star-control-api
packages/star-control-ui
```

Star Sentinel은 builtin tool 구현이므로 `packages/star-sentinel`에 둔다. `packages/star-control-star-sentinel` 같은 core/tool 혼합 이름은 만들지 않는다.

기존 scaffold인 `star-provider-*`, `star-transport-*`, `star-adapter-*` 계열은 core가 안정화된 뒤 provider/transport/adapter extension package로 분류한다. 해당 scaffold는 core namespace를 대체하지 않는다.

## dependency defaults

Rust 표준 라이브러리만으로 안정성을 해치며 재구현하지 않는다. 완전 구현에서 기본 후보로 허용할 수 있는 Rust dependency는 다음이다.

```text
serde
serde_json
thiserror
clap
tempfile
time
```

이 결정은 위 dependency를 자동 추가한다는 뜻이 아니다. 실제 `Cargo.toml` 변경은 해당 구현 PR에서 목적, 대안, 검증 명령을 명시한다. Cargo 외 package manager, provider SDK, web UI stack, release/deploy 관련 dependency는 별도 승인 대상이다.

## StateStore 기준

File-based `.ai-runs/` artifact는 canonical source로 유지한다.

DB 또는 daemon-local cache는 장기 adapter나 index 후보일 뿐이며, 다음 원칙을 따른다.

- canonical job artifact를 대체하지 않는다.
- 대상 프로젝트 `.ai-runs/` 밖에 provider/tool output을 저장하지 않는다.
- recovery와 audit는 파일 artifact를 기준으로 재구성 가능해야 한다.

## provider 확장 순서

Provider 확장은 위험이 낮은 순서로 진행한다.

1. `fake`: 외부 호출 없는 deterministic smoke.
2. `local_process`: 허용된 명령만 실행하는 local process provider.
3. local OpenAI-compatible/local server: Ollama, LM Studio, llama.cpp server, vLLM 같은 local server 계열.
4. `cloud_cli`: Codex CLI, Claude Code, Gemini CLI 등 CLI agent 계열.
5. `cloud_api`: OpenAI, Anthropic, Gemini API 등 HTTP API 계열.

Cloud/local provider adapter 구현 전에는 해당 provider 공식 문서를 최신으로 재확인한다. Credential raw value는 artifact, log, report, commit에 저장하지 않고 `credential_ref`만 사용한다.

## user surface 확장 순서

User surface는 다음 순서로 구현한다.

1. CLI: file-based flow의 기준 surface.
2. Daemon: 장시간 queue, session, resume/cancel orchestration.
3. API: UI와 외부 도구가 쓰는 read-only endpoint부터 시작.
4. UI: read-only job timeline과 review/approval view부터 시작.

API/UI는 StateStore schema를 우회하지 않는다. UI는 provider process나 Star Sentinel rule을 직접 실행하지 않는다.

## approval-required actions

다음 변경은 완전 구현 후반에도 approval required다.

```text
dependency_addition
dependency_version_change
workflow_change
release_publish
deploy
credential_change
external_account_change
destructive_file_operation
validator_policy_change
```

Approval 없이 자동 진행해야 한다면 그 설계는 거부한다.

## 범위 밖

이 결정은 다음을 수행하지 않는다.

- Cargo workspace 생성
- dependency 추가
- provider 실제 연결
- daemon/API/UI 구현
- release/deploy/publish 자동화
- GitHub repository settings 변경

## 적용 문서

이 결정은 다음 문서의 해석 기준이다.

```text
docs/implementation/repository-layout.md
docs/implementation/complete-implementation-roadmap.md
docs/implementation/codex-work-queue.md
docs/implementation/codex-work-queue-current.md
docs/providers/provider-model.md
docs/operations/Star-Control_MVP_Runbook.md
```
