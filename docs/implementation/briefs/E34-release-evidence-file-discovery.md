# E34 Release Evidence File Discovery

## 목표

M9i slice는 M9h `ReleaseConsistencyChecker`가 실제 repository evidence file을 읽어 사용할 수 있는 read-only file discovery foundation을 구현한다. 이 단계는 caller가 지정한 project root 내부의 version/changelog file만 읽고, version text와 changelog text를 checker에 연결한다.

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
automatic repository-wide scan
changelog format parser
release profile integration
```

## 입력

```text
project_root
expected release version
relative version evidence path
relative changelog evidence path
```

## 출력

```text
ReleaseConsistencyResult
version-consistent check with version evidence path
changelog-updated check with changelog evidence path
blockers for unsafe path, missing version, version/changelog mismatch
```

## 핵심 TASK

```text
ReleaseEvidenceFileChecker 추가
project root containment check
unsafe relative path rejection
version file read-only loading
changelog file read-only loading
simple version declaration extraction
ReleaseConsistencyChecker 연결
no mutation regression test
```

## 완료 기준

- checker가 project root 내부 version/changelog file을 읽고 `ReleaseConsistencyResult`를 반환해야 한다.
- `VERSION` 같은 plain version file과 `version = "x.y.z"` declaration을 처리해야 한다.
- `../`, absolute path, drive-prefixed path처럼 project root를 벗어나는 evidence path를 거부해야 한다.
- missing version declaration은 blocker로 숨기지 않고 explicit error로 반환해야 한다.
- automatic repository-wide scan, changelog parser, release profile integration, CLI/API/UI surface, release/deploy/publish, repository settings, workflow, schema field 변경은 하지 않는다.

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

M9j는 release profile validation integration, release readiness CLI/UI read surface, or recovery command surface 중 하나로 이어간다. signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
