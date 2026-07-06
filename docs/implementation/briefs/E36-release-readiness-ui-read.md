# E36 Release Readiness UI Read

## 목표

M9k slice는 M9g API read endpoint로 노출된 ReleaseReadiness artifact를 `star-control-ui` read-only view model에 연결한다. 이 단계는 library-level UI model만 확장하고, browser app, HTTP server, CLI command, release/deploy/publish action은 구현하지 않는다.

## 선행 문서

```text
complete-implementation-roadmap.md
release-readiness.md
ui-shell-contract.md
testing-ci-release.md
docs/decisions/0005-full-implementation-defaults.md
```

## 허용 파일

```text
packages/star-control-ui/**
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
StateStore 직접 mutation
Star Sentinel profile evaluator 변경
```

## 입력

```text
project id
job id
ApiReadOnlyService GET /projects/{project_id}/jobs/{job_id}/release-readiness response
```

## 출력

```text
release_readiness_viewer
available true/false
readiness path
release id
target
version
status
checks
blockers
approvals
read-only mutation flags
```

## 핵심 TASK

```text
UiReadOnlyShell release_readiness view 추가
job_detail에 release_readiness_viewer 포함
missing readiness optional error surface 유지
existing readiness read-only view regression
release action disabled regression
no mutation regression
```

## 완료 기준

- `UiReadOnlyShell`이 release readiness API endpoint를 읽어 release readiness viewer를 반환해야 한다.
- `job_detail` view에 release readiness viewer가 포함되어야 한다.
- readiness artifact가 없으면 job detail 전체가 실패하지 않고 optional read-only error surface를 반환해야 한다.
- readiness artifact가 있으면 status/checks/blockers/approvals를 표시해야 한다.
- UI가 readiness artifact, StateStore, release/deploy/publish state를 수정하지 않아야 한다.
- browser app, HTTP server, CLI command, schema field, workflow, release/deploy/publish, repository settings 변경은 하지 않는다.

## 검증

```text
cargo fmt --check
cargo test -p star-control-ui --locked -- --nocapture
cargo clippy -p star-control-ui --all-targets --locked -- -D warnings
powershell -ExecutionPolicy Bypass -File .\scripts\test.ps1
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

## 다음 handoff

M9l는 release readiness CLI read surface, release review pack foundation, or recovery command surface 중 하나로 이어간다. signing, publish, deploy automation은 별도 승인 전까지 RESERVED다.
