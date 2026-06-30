# 0002 Runtime Stack Decision

## 상태

Accepted.

## 결정

Star-Control의 v0 runtime 구현 스택은 다음으로 고정한다.

```text
implementation language: Rust
package manager: Cargo
workspace model: Cargo workspace
initial implementation flow: E01 -> E11 sequential implementation
```

이 결정은 구현 언어와 package manager 미정 상태를 해소하기 위한 정본 결정이다.

## 범위

이 결정은 다음 영역에 적용한다.

- `packages/` 아래 Star-Control runtime package 구현
- `apps/starctl/` CLI entrypoint 구현
- Star Sentinel runtime package 구현
- Codex가 E01~E11 구현 PR을 만들 때의 build/test 기준

## Cargo workspace 기준

초기 구현은 Cargo workspace를 사용한다.

목표 package 경계는 `docs/implementation/repository-layout.md`의 package 책임을 따른다. 실제 crate 생성은 구현 PR에서 시작한다.

초기 workspace 후보:

```text
packages/star-control-schema
packages/star-control-state
packages/star-control-provider
packages/star-control-router
packages/star-control-execution
packages/star-control-validation
packages/star-control-report
packages/star-control-cli
packages/star-sentinel
apps/starctl
```

이 목록은 목표 경계다. E01 구현 시점에는 필요한 최소 crate부터 생성한다.

## 구현 순서 기준

구현은 전체 목표를 길게 유지하되, 실제 PR은 E01~E11 순서로 진행한다.

```text
E01 Schema / Runtime Validator
E02 File-based StateStore
E03 Artifact Layout Writer
E04 Provider Registry
E05 FakeProviderAdapter
E06 RouterEngine
E07 ExecutionEngine
E08 CLI read-only + fake run
E09 Star Sentinel P0
E10 ValidationEngine
E11 Integration Smoke
```

기본적인 compile/test/schema 오류는 각 EPIC 구현 중에 잡는다. 별도 실사용 디버깅은 v0 fake flow와 integration smoke가 안정화된 뒤 진행한다.

## 승인 정책

이 결정으로 승인되는 것:

- Rust를 Star-Control v0 구현 언어로 사용
- Cargo를 Star-Control v0 package manager로 사용
- 구현 PR에서 Cargo workspace와 최소 `Cargo.toml`을 추가
- 구현 PR에서 `Cargo.lock`을 생성하거나 갱신

이 결정으로 승인되지 않는 것:

- 임의 production dependency 추가
- Cargo 외 package manager 도입
- release/deploy/publish automation 구현
- cloud/local provider를 fake flow 이전에 활성화
- daemon/API/UI를 fake CLI flow 이전에 구현

새 dependency가 필요하면 해당 PR에서 이유, 대안, 검증 방법을 명시하고 별도 승인을 받는다.

## CI / 검증 기준

Cargo workspace가 생성된 뒤 기본 검증 후보는 다음이다.

```text
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo check --workspace
```

기존 문서/schema/manifest 검증은 계속 유지한다. CI를 통과시키기 위해 테스트, schema, example, policy, naming check를 약화하지 않는다.

## 비결정 사항

아래 항목은 이 결정에서 확정하지 않는다.

- crate별 public API 세부 설계
- 외부 dependency 목록
- release packaging 방식
- daemon/API/UI 구현 시점
- local/cloud provider 실제 연결 방식

## 근거

Rust + Cargo는 Star-Control의 file-based StateStore, CLI, provider adapter, path guard, deterministic validation, long-running daemon 확장에 적합하다. 초기 구현은 fake provider와 local file artifact flow에 집중하고, local/cloud provider와 UI 계층은 E11 integration smoke 이후에 확장한다.
