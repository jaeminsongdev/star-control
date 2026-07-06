# E41 Final Completion Audit

## 목표

M9p slice는 M0~M9 완전 구현 요구사항을 milestone 단위로 점검하는 final completion audit builder를 추가한다. 이 slice는 existing evidence path와 pass/fail 결과를 `release-readiness.schema.json` 형식으로 조립하며, 실제 release/deploy/publish automation, repository settings 변경, destructive recovery action을 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
release-readiness.md
testing-ci-release.md
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
COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS
Vec<CompleteImplementationAuditCheck>
ReleaseReadinessWriter
```

## 출력

```text
schema-valid ReleaseReadiness value
status = reserved when every complete implementation required check passes
status = not_ready when required check is missing, duplicated, or failed
blockers include release/deploy/publish and external repository settings reserved explanation
```

## 핵심 TASK

```text
COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS public contract 추가
CompleteImplementationAuditCheck pass/fail evidence validation
CompleteImplementationAuditBuilder 추가
missing/duplicate/failed completion check blocker 생성
all-pass audit reserved status regression
ready status no-generation regression
unsafe evidence path rejection
```

## 완료 기준

- `CompleteImplementationAuditBuilder`가 모든 `COMPLETE_IMPLEMENTATION_REQUIRED_CHECKS` pass 결과를 schema-valid `reserved` readiness로 조립해야 한다.
- all-pass 결과도 `ready`가 아니라 release/deploy/publish 및 external repository settings reserved blocker가 있는 `reserved` status여야 한다.
- missing, duplicate, failed completion check는 schema-valid `not_ready` readiness와 blocker로 표시해야 한다.
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

M9q는 final audit evidence 채움, stacked PR merge 정리, 또는 별도 승인된 recovery/release action surface로 이어간다. destructive recovery, signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
