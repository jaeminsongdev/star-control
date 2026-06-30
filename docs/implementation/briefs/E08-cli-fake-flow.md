# E08 CLI read-only + fake run Brief

## 목표

초기 CLI에서 status/report read-only command와 fake provider run flow를 연결한다.

## 선행 문서

```text
docs/implementation/cli-command-reference.md
docs/implementation/state-store.md
docs/implementation/execution-engine.md
docs/implementation/ci-contract-validation.md
```

## 수정 허용 파일

```text
packages/star-control-cli/** 또는 선택된 CLI crate
apps/starctl/** scaffold 범위
examples/cli-contracts/** 필요 최소 범위
관련 unit tests
```

## 수정 금지 파일

```text
daemon 구현
API 구현
UI 구현
local/cloud provider 실제 연결
release automation
```

## 핵심 작업

```text
status command
report command
run dry-run
run with fake provider
--json output envelope
error envelope
approve/cancel/resume 후보
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

fake project에서 `run`, `status`, `report`가 동작하고 JSON output이 schema를 만족해야 한다.

## handoff

E09/E10 smoke에서 사용할 CLI command shape, exit code, sample fake run project를 PR 보고에 남긴다.

## 중단 조건

daemon/API/UI, release automation, local/cloud provider 연결이 필요하면 멈춘다.
