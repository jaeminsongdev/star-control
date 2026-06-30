# E02 File-based StateStore Brief

## 목표

`.ai-runs/{job_id}/` 기반 file StateStore를 구현한다.

## 선행 문서

```text
docs/implementation/state-store.md
docs/implementation/state-store-recovery.md
docs/implementation/artifact-layout.md
docs/implementation/artifact-naming.md
docs/implementation/run-lifecycle.md
docs/implementation/schema-validator.md
```

## 수정 허용 파일

```text
packages/star-control-state/** 또는 선택된 state crate
관련 unit tests
필요한 최소 docs/example 업데이트
```

## 수정 금지 파일

```text
RouterEngine 구현 파일
ProviderAdapter 구현 파일
ExecutionEngine 구현 파일
ValidationEngine 구현 파일
CLI 구현 파일
Star Sentinel rule 구현 파일
```

## 핵심 작업

```text
job directory resolver
job.json / run-state.json reader-writer
events.jsonl append
atomic write 후보
tmp file policy
terminal state guard
resume precondition helper
schema validator integration
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

JobSpec, RunState, CoreEvent를 파일로 읽고 쓰며 schema 검증을 통과해야 한다.

## handoff

E03/E07이 사용할 path helper, write API, recovery behavior를 PR 보고에 남긴다.

## 중단 조건

파일 삭제, 대량 이동, repository root `.ai-runs/` 생성이 필요해 보이면 멈춘다.
