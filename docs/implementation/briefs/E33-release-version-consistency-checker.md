# E33 Release Version Consistency Checker

## 목표

M9h slice는 release readiness의 `version-consistent`와 `changelog-updated` check를 생성하는 foundation checker를 구현한다. 이 단계는 caller가 제공한 version/changelog evidence text를 평가해 checks와 blockers를 만들 뿐이며, filesystem discovery, changelog parser, release profile integration, signing, publish, deploy는 수행하지 않는다.

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
release/deploy/publish automation
external account/repository settings 변경
provider live call
HTTP server 구현
browser UI app 구현
CLI command 추가
artifact signing 구현
package registry 설정
repository branch protection/settings 변경
filesystem changelog discovery
```

## 입력

```text
expected release version
declared version text
changelog text
version evidence path string
changelog evidence path string
```

## 출력

```text
ReleaseConsistencyResult
version-consistent check
changelog-updated check
blockers for version/changelog mismatch
```

## 핵심 TASK

```text
ReleaseConsistencyChecker 추가
ReleaseConsistencyResult 추가
version-consistent pass/fail check 생성
changelog-updated pass/fail check 생성
version mismatch blocker 생성
changelog missing version blocker 생성
not_ready ReleaseReadiness에 연결 가능한 checks/blockers 검증
schema field 변경 없이 release-readiness.schema.json validation 유지
```

## 완료 기준

- matching declared version과 changelog text는 `version-consistent=pass`, `changelog-updated=pass`를 반환해야 한다.
- version mismatch와 changelog gap은 `fail` checks와 blocker를 반환해야 한다.
- checker output은 `ReleaseReadinessWriter::not_ready`에 들어가 schema-valid ReleaseReadiness artifact를 만들 수 있어야 한다.
- filesystem discovery, changelog parser, release profile integration, CLI/API/UI surface, release/deploy/publish, repository settings, workflow, schema field 변경은 하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-release --locked -- --nocapture
cargo clippy -p star-control-release --all-targets --locked -- -D warnings
python scripts/ci/run_all.py
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9i는 release profile validation integration, release readiness CLI/UI read surface, changelog/version file discovery, or recovery command surface 중 하나로 이어간다. signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
