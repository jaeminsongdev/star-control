# Codex Long-run Workflow

## 목적

이 문서는 Codex가 Star-Control을 장시간 목표추진 방식으로 구현할 때 따를 운영 규칙이다. 목표는 전체 완성형 구현이지만, 작업은 반드시 작은 PR과 검증 가능한 checkpoint로 나눈다.

## 핵심 원칙

- 전체 목표는 길게 유지하되 PR은 작게 유지한다.
- 한 PR은 한 목적만 가진다.
- `main`은 직접 수정하지 않는다.
- 항상 새 `work/...` 브랜치에서 작업한다.
- 실패한 테스트, CI, 정책 검사를 삭제하거나 약화하지 않는다.
- v0 runtime stack은 Rust + Cargo workspace다.
- Cargo workspace 파일은 구현 PR에서 추가한다.
- 새 production dependency, Cargo 외 package manager, 배포, 릴리즈, 외부 계정 변경은 명시 승인 전까지 하지 않는다.
- Star-Control core는 provider-neutral이어야 한다.
- Star Sentinel은 builtin tool 경계 안에 둔다.
- 실행 산출물은 Star-Control repository가 아니라 대상 프로젝트 `.ai-runs/`에 둔다.

## 작업 시작 전 필수 읽기

Codex는 구현 작업을 시작하기 전에 다음을 읽어야 한다.

```text
AGENTS.md
README.md
docs/implementation/README.md
docs/decisions/0002-runtime-stack.md
docs/decisions/0003-fake-provider-instance.md
docs/implementation/codex-long-run-workflow.md
docs/implementation/codex-work-queue-current.md
```

Star Sentinel P0 작업은 `docs/decisions/0004-star-sentinel-p0-scope.md`와 `docs/implementation/star-sentinel-p0-implementation-split.md`도 함께 읽는다.

`docs/implementation/codex-work-queue-current.md`는 실제 착수 순서의 최상위 기준이다. `docs/implementation/codex-work-queue.md`는 장기 backlog로만 사용한다.

그리고 해당 EPIC과 관련된 세부 문서를 읽는다.

예시:

- Schema Validator 작업: `schema-validator.md`, `data-contracts.md`, `ci-contract-validation.md`
- StateStore 작업: `state-store.md`, `artifact-layout.md`, `artifact-naming.md`, `state-store-recovery.md`, `data-contracts.md`, `run-lifecycle.md`
- Router 작업: `router-decision-matrix.md`, `router-engine.md`, `provider-system.md`, `policy-profiles.md`
- Validation 작업: `validation-engine.md`, `validation-handoff.md`, `star-sentinel-p0-contracts.md`, `approval-review-flow.md`
- CLI 작업: `cli-command-reference.md`, `state-store.md`, `execution-engine.md`, `ci-contract-validation.md`

## 작업 단위

Codex는 `codex-work-queue-current.md`의 EPIC/TASK 단위를 따른다. EPIC 하나가 너무 크면 TASK 단위로 더 쪼갠다. 장기 backlog인 `codex-work-queue.md`의 항목을 현재 큐보다 앞당기지 않는다.

권장 단위:

```text
EPIC -> TASK -> PR
```

한 PR은 다음 조건을 만족해야 한다.

- 수정 파일 범위가 명확하다.
- 문서/계약/구현/테스트 목적이 하나다.
- validation command가 명확하다.
- 실패 시 원인을 추적할 수 있다.
- 다음 EPIC/TASK로 넘기는 handoff가 명확하다.

## 브랜치 규칙

브랜치 이름은 짧고 중립적으로 만든다.

권장:

```text
work/e01-schema
work/e02-state
work/e03-artifacts
work/e04-provider-registry
work/e05-fake-provider
```

피해야 할 이름:

```text
ai/...
fix-everything
big-implementation
rewrite-all
```

## PR 본문 규칙

PR 본문에는 다음을 포함한다.

```text
목표
변경 파일
수정 금지 파일 준수 여부
검증 명령
검증 결과
남은 작업
다음 작업
```

긴 로그 전체를 붙이지 말고 핵심 결과를 요약한다.

## 작업 전 확인 질문 기준

Codex는 다음 상황에서 작업을 멈추고 질문해야 한다.

- 새 production dependency 추가가 필요해 보일 때
- Cargo workspace 기준 밖의 package manager 도입이 필요해 보일 때
- 배포, 릴리즈, 외부 계정 변경이 필요할 때
- public API breaking change가 필요한 때
- schema breaking change가 필요한 때
- workflow permission 변경이 필요한 때
- 문서와 schema가 충돌할 때
- 현재 큐와 장기 backlog 중 어느 쪽을 따라야 하는지 모호할 때
- 요구 범위가 불명확해 여러 해석이 가능할 때

질문하지 않고 진행 가능한 경우:

- 문서에 명확한 계약이 있음
- allowed files가 명확함
- test/CI 실패 원인이 코드 오류로 명확함
- 기존 계약을 더 엄격하게 따르는 수정
- `docs/decisions/0002-runtime-stack.md` 범위 안의 Rust + Cargo workspace baseline 구현

## 구현 중 금지 사항

Codex는 다음을 하면 안 된다.

- 테스트를 삭제해서 통과시키기
- assertion을 약화해서 통과시키기
- CI job을 삭제하거나 이름을 바꿔 우회하기
- schema-example-check case 삭제
- implementation-documentation-check required path를 이유 없이 제거하기
- Star Sentinel naming policy 우회
- core에 provider 제품명 package 추가
- Star Sentinel rule을 core에 직접 구현
- Star-Control repo 내부에 `.ai-runs/` 생성
- credential raw value 저장
- 승인된 Cargo workspace 범위를 벗어난 package manager 또는 lockfile 무단 생성
- local/cloud provider, daemon, API, UI, release automation을 현재 큐보다 앞당겨 구현하기

## validation command 규칙

각 PR은 최소 하나 이상의 validation command를 가져야 한다.

현재 기본 검증:

```text
python scripts/ci/run_all.py
```

개별 실패를 디버깅할 때는 아래 명령을 따로 실행한다.

```text
python scripts/ci/check_repo_policy.py
python scripts/ci/check_data_formats.py
python scripts/ci/check_manifest_contracts.py
python scripts/ci/check_star_sentinel_naming.py
python scripts/ci/check_schema_examples.py
python scripts/ci/check_implementation_docs.py
python scripts/ci/check_work_queue_consistency.py
```

실제 Cargo workspace가 생긴 뒤에는 package별 test command를 추가한다.

초기 Rust 후보:

```text
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo check --workspace
```

## 실패 처리 규칙

실패가 발생하면 다음 순서로 처리한다.

1. 실패 job 확인
2. 실패 step 확인
3. 핵심 오류 메시지 확인
4. 실제 코드/문서/계약 위반인지 판단
5. 위반이면 수정
6. 오탐이면 문서화된 의도에 맞게 검사 범위 조정
7. 검사 삭제/약화는 금지

실패 보고 형식:

```text
실패 job:
실패 step:
핵심 오류:
원인 판단:
수정 내용:
재검증 결과:
```

## checkpoint 규칙

장시간 목표추진 중에도 다음 checkpoint마다 멈춘다.

- PR 생성 전
- CI 실패 시
- approval required change 발견 시
- EPIC 완료 시
- 다음 EPIC 진입 전
- 문서 계약과 코드가 충돌할 때
- 현재 작업이 `codex-work-queue-current.md` 범위를 벗어날 때

## 완료 기준

TASK 완료 기준:

- 목표 파일 구현 완료
- 금지 파일 미수정
- 관련 테스트 추가 또는 갱신
- 문서와 schema 계약 위반 없음
- validation command 성공
- PR 본문에 검증 결과 기록
- 다음 TASK 또는 EPIC handoff 기록

EPIC 완료 기준:

- 해당 EPIC의 모든 TASK 완료
- integration smoke 또는 contract test 성공
- `codex-validation-report.md` 형식으로 결과 요약 가능
- 다음 EPIC 진입 조건 충족

## 장시간 작업 중 context 관리

Codex 세션이 길어지면 다음을 요약하고 새 세션에서 이어간다.

```text
현재 EPIC/TASK
완료한 PR
현재 branch
수정 파일
검증 결과
남은 작업
주의해야 할 계약
다음 EPIC/TASK handoff
```

새 세션은 반드시 `AGENTS.md`, `README.md`, `docs/implementation/README.md`, `docs/decisions/0002-runtime-stack.md`, `docs/implementation/codex-long-run-workflow.md`, `docs/implementation/codex-work-queue-current.md`, 현재 EPIC 문서를 다시 읽는다.

## 사람이 승인해야 하는 조건

아래 조건은 자동 진행 금지다.

```text
dependency_addition
dependency_version_change
unapproved_package_manager_introduction
workflow_change
release_publish
deploy
external_account_change
public_api_breaking_change
schema_breaking_change
validator_policy_weakening
credential_change
```

## Codex 최종 보고 형식

각 PR 완료 후 보고:

```text
PR:
branch:
변경 파일:
검증 명령:
검증 결과:
남은 위험:
다음 작업:
```

EPIC 완료 후 보고:

```text
EPIC:
완료 TASK:
merge된 PR:
최종 검증:
남은 TODO:
다음 EPIC 진입 가능 여부:
```
