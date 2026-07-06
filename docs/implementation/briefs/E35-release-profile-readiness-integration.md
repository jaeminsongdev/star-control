# E35 Release Profile Readiness Integration

## 목표

M9j slice는 release profile validation result를 release readiness artifact 조립 흐름에 연결한다. 이 단계는 Star Sentinel release profile 결과처럼 caller가 이미 계산한 pass/fail evidence를 받아 `release-profile-passed` check로 만들고, M9h/M9i version/changelog consistency result와 합쳐 schema-valid ReleaseReadiness JSON을 생성한다.

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
Star Sentinel profile evaluator 변경
automatic repository-wide scan
changelog format parser
```

## 입력

```text
release id
target
expected release version
release profile name
release profile pass/fail result
release profile evidence paths
release profile blockers
ReleaseConsistencyResult
```

## 출력

```text
schema-valid ReleaseReadiness JSON
release-profile-passed check
version-consistent check
changelog-updated check
not_ready status when blockers exist
reserved status when checks pass but release automation remains reserved
```

## 핵심 TASK

```text
ReleaseProfileValidation 추가
ReleaseProfileReadinessBuilder 추가
release-profile-passed check 생성
profile blocker와 consistency blocker 병합
profile/consistency all-pass 상태에서도 ready status 금지
unsafe profile evidence path rejection
schema-valid readiness regression test
```

## 완료 기준

- release profile pass/fail result가 `release-profile-passed` check로 들어가야 한다.
- profile failure와 version/changelog failure가 동일 ReleaseReadiness blockers에 병합되어야 한다.
- profile/version/changelog가 모두 통과해도 `ready`를 만들지 않고 release automation reserved blocker를 둬야 한다.
- unsafe evidence path와 empty profile/blocker input을 explicit error로 반환해야 한다.
- Star Sentinel profile evaluator, release/deploy/publish, repository settings, workflow, schema field 변경은 하지 않는다.

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

M9k는 release readiness CLI/UI read surface, release review pack foundation, or recovery command surface 중 하나로 이어간다. signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
