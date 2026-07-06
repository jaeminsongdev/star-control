# E40 Final M9 Readiness Audit

## 목표

M9o slice는 M9 hardening/recovery/release-readiness foundation이 빠짐없이 증거화되었는지 확인하는 final readiness audit builder를 추가한다. 이 slice는 existing evidence path와 pass/fail 결과를 `release-readiness.schema.json` 형식으로 조립하며, 실제 release/deploy/publish automation이나 destructive recovery action을 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
release-readiness.md
testing-ci-release.md
state-store-recovery.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
packages/star-control-release/**
docs/implementation/**
docs/operations/**
PLANS.md
README.md
```

## 금지 파일

```text
GitHub workflow
schema field 변경
Cargo 외 package manager
새 external dependency
ready status 생성
release/deploy/publish automation
signing automation
package registry 설정
external account/repository settings 변경
destructive recovery action
tmp file 삭제
event log trim
artifact 교체
provider live call
HTTP server 구현
browser UI app 구현
```

## 입력

```text
M9_REQUIRED_READINESS_CHECKS
Vec<M9ReadinessCheck>
ReleaseReadinessWriter
```

## 출력

```text
schema-valid ReleaseReadiness value
status = reserved when every M9 required check passes
status = not_ready when required check is missing, duplicated, or failed
blockers include final release/deploy/publish reserved explanation
```

## 핵심 TASK

```text
M9_REQUIRED_READINESS_CHECKS public contract 추가
M9ReadinessCheck pass/fail evidence validation
M9ReadinessAuditBuilder 추가
missing/duplicate/failed check blocker 생성
all-pass audit reserved status regression
ready status no-generation regression
unsafe evidence path rejection
```

## 완료 기준

- `M9ReadinessAuditBuilder`가 모든 `M9_REQUIRED_READINESS_CHECKS` pass 결과를 schema-valid `reserved` readiness로 조립해야 한다.
- all-pass 결과도 `ready`가 아니라 final release/deploy/publish reserved blocker가 있는 `reserved` status여야 한다.
- missing, duplicate, failed M9 check는 schema-valid `not_ready` readiness와 blocker로 표시해야 한다.
- check name은 public required list에 있는 값만 허용해야 한다.
- evidence path는 project-relative safe path만 허용해야 한다.
- schema field, workflow, dependency, CLI/API/UI surface, signing, publish, deploy, destructive recovery action은 변경하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-release --locked -- --nocapture
cargo clippy -p star-control-release --all-targets --locked -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9p는 final completion audit, stacked PR merge 정리, 또는 별도 승인된 recovery/release action surface로 이어간다. destructive recovery, signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
