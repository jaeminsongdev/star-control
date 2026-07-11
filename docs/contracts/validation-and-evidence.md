# 검사·완료·증거

## 목표

검사는 많이 하는 것이 목적이 아니다. 결과가 맞는지 판단하는 데 실제로 필요한 검사만 선택하고, 작업 흐름을 불필요하게 늦추지 않아야 한다.

## 공통 실행 계약

검사와 일반 도구 실행은 shell 문자열이 아니라 `TaskInvocation`으로 기록한다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `invocation_id` | typed ID | 한 실행 요청 |
| `tool_ref` | Catalog ref | 검증된 ToolDescriptor ID와 version |
| `executable` | string | 탐지·허용 목록을 통과한 실행 파일 |
| `args` | string array | shell 재해석 없이 전달할 인자 |
| `cwd` | ProjectPathRef | Project root 안의 작업 위치 |
| `env_refs` | map<name, value/SecretRef> | 허용된 환경 이름과 값 reference |
| `stdin_ref` | optional ArtifactRef | 큰 입력 또는 민감하지 않은 입력 자료 |
| `timeout_ms` | positive integer | 강제 종료 전 한도 |
| `permission_action` | action ID | PermissionPlan에서 확인할 행동 |
| `idempotency_key` | string | 같은 side effect 중복 실행 방지 |
| `expected_exit_codes` | integer set | 성공으로 해석할 종료 code |
| `output_limits` | object | stdout·stderr와 artifact 크기 상한 |

`executable`과 `args`는 process API에 그대로 전달한다. shell이 필요한 검사는 Catalog에 포함된 신뢰된 script를 executable로 참조하고 동적 script text를 만들지 않는다. 실행 시 실제 executable path와 version을 결과에 남긴다.

## ValidationPlan 계약

ValidationPlan은 무엇을 왜 검사할지 실행 전에 정한다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `goal_id`, `run_id`, `stage_id` | typed ID | 적용 범위. goal-level이면 stage는 생략 |
| `scope_revision` | integer | 검증할 변경·계획 revision |
| `phase` | enum | `during_stage`, `stage_exit`, `goal_exit`, `merge`, `release` |
| `change_summary_ref` | ArtifactRef | 선택 근거가 된 변경 목록 |
| `risk_level` | enum | 검사 강도를 정한 위험 등급 |
| `required_checks` | CheckPlan array | 실패하면 gate를 막는 검사 |
| `optional_checks` | CheckPlan array | 정보 보강용 검사 |
| `omitted_checks` | OmittedCheck array | 가능한 검사 중 실행하지 않는 이유와 대체 증거 |
| `manual_observations` | ManualObservationPlan array | 사람이 실제 흐름을 확인해야 하는 항목 |
| `independent_review` | ReviewRequirement | 별도 Codex 검토 필요 여부와 범위 |
| `gate_policy` | GatePolicy | 결과를 완료·검토·차단으로 바꾸는 규칙 |
| `config_fingerprint` | SHA-256 | 선택에 사용한 EffectiveConfig |
| `catalog_snapshot_ref` | ArtifactRef | Check·Tool 정의 근거 |

CheckPlan은 `check_ref`, 선택 이유, TaskInvocation template, 기대 결과, timeout, retry, cache 사용 조건과 생성할 evidence 종류를 가진다. `omitted_checks`가 비어 있다는 사실과 검사 후보를 조사하지 않았다는 뜻을 섞지 않는다.

### 계획 불변식

1. 모든 필수 Check와 Tool reference는 같은 CatalogSnapshot에서 해석되어야 한다.
2. 위험을 낮게 적어 필수 검사를 피할 수 없다. 위험 결정과 검사 선택 근거를 함께 남긴다.
3. `not_run`은 `pass`가 아니며 대체 증거가 있어도 원래 검사 상태는 바꾸지 않는다.
4. 변경 revision이 달라지면 영향받는 ValidationPlan을 다시 계산한다.
5. 검사 자체가 파일·외부 상태를 바꾸면 별도 Permission action과 변경 증거를 가진다.

## ChangeSet 계약

ChangeSet은 사용자의 기존 변경과 Star-Control이 만든 변경을 분리해 영향 분석, 검사 선택과 병합이 같은 자료를 보게 한다.

| 필드 | 의미 |
|---|---|
| `change_set_id` | 문서 ID |
| `project_id`, `run_id`, `stage_id` | 변경 범위 |
| `base_revision` | 작업 시작 기준 commit·filesystem fingerprint |
| `observed_revision` | 수집 시점 workspace fingerprint |
| `entries` | add, modify, delete, rename, mode, binary, submodule 변경 |
| `preexisting_entries` | 시작 전 사용자 변경 reference |
| `classifications` | source, test, docs, config, schema, migration, generated, vendor 등 |
| `impact_edges` | 파일·symbol·package·contract·test의 직접·전이 영향 |
| `scope_relation` | planned, necessary_expansion, unrelated, unknown |
| `risk_findings` | 위험 경로, confidence와 근거 |
| `collection_limits` | 도구 미지원·미확인 영역 |

각 entry는 ProjectPathRef, 전·후 hash, 변경 종류, 가능한 line 통계, rename source, binary 여부, ownership와 생성 주체를 가진다. 전체 diff는 ArtifactRef로 분리한다. `unrelated`나 `unknown`을 자동으로 Star-Control 변경으로 덮거나 되돌리지 않는다.

## ValidationRun 계약

ValidationRun은 CheckPlan 한 항목의 실제 시도다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `validation_run_id` | typed ID | 검사 실행 ID |
| `validation_plan_ref` | document ref | 원래 계획과 revision |
| `check_ref`, `tool_ref` | Catalog ref | 사용한 정의 |
| `attempt` | positive integer | 같은 Check의 시도 번호 |
| `invocation` | TaskInvocation | 실제 인자와 제한 |
| `started_at`, `finished_at` | UTC timestamp | 실행 구간 |
| `outcome` | enum | `pass`, `fail`, `not_run`, `error`, `cancelled` |
| `completeness` | enum | `complete`, `partial`, `unverified` |
| `exit_code` | optional integer | process가 시작된 경우 |
| `termination_reason` | enum | `exited`, `timeout`, `cancelled`, `launch_error`, `outcome_unknown` |
| `diagnostic_refs` | ref array | 정규화된 진단 |
| `stdout_ref`, `stderr_ref` | optional ArtifactRef | redaction한 원문 |
| `result_artifact_refs` | ArtifactRef array | report, trace, screenshot 등 |
| `observed_tool` | object | 실제 path, version과 hash |
| `cache` | object | hit 여부, cache key와 원래 run |

`outcome=pass`이려면 실행이 시작되어 기대 종료 code와 Check 의미를 모두 만족해야 한다. parser 실패, 출력 잘림이나 결과 일부만 확인한 경우 `complete`로 만들 수 없다.

## GateDecision 계약

GateDecision은 여러 ValidationRun과 Diagnostic을 완료 판단으로 모은다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `gate_id` | typed ID | gate 판단 ID |
| `scope` | object | goal, stage, merge 또는 release와 revision |
| `decision` | enum | `auto_pass`, `human_review`, `block` |
| `required_run_refs` | ref array | 판단에 사용해야 하는 필수 검사 |
| `satisfied_run_refs` | ref array | 충족된 검사 |
| `blocking_diagnostic_refs` | ref array | 차단 원인 |
| `waivers` | WaiverRef array | 사용자가 명시적으로 수용한 예외 |
| `omissions` | OmittedCheck array | 미실행 검사와 영향 |
| `remaining_risks` | RiskRef array | 통과 뒤에도 남은 위험 |
| `policy_snapshot` | object | 적용한 gate threshold와 출처 |
| `decided_by` | ActorRef | engine 또는 사용자 |

- 필수 검사 실패, 실행 오류와 확인되지 않은 중대한 결과는 `auto_pass`가 될 수 없다.
- waiver는 실패 결과를 통과로 변조하지 않고 GateDecision에만 적용한다.
- waiver 대상, revision, 만료와 사용자가 본 evidence hash가 달라지면 새 승인이 필요하다.
- `human_review`는 차단도 성공도 아니며 RunSnapshot에 대기 상태로 나타난다.

### 공개 구현과 소비 경계

`crates/foundation/star-contracts`가 `ValidationRun`, `GateDecision`, `EvidenceBundle`, `Diagnostic`과 지원 ref·enum의 Rust 및 JSON Schema 정본을 소유한다. schema ID는 각각 `star.validation-run`, `star.gate-decision`, `star.evidence-bundle`, `star.diagnostic`으로 고정한다.

하위 adapter는 `GateDecision::authoritative_state()`가 반환한 상태만 완료 판정으로 소비한다. `ValidationRun` 목록을 다시 집계해 `GateDecision`을 대체하거나 `not_run`을 통과로 바꿀 수 없다. `validate_against`는 이미 만들어진 결정의 참조와 불변식을 검증할 뿐 새 결정을 계산하지 않는다. adapter는 `StarValidationResult`, `CompletionEvidence` 같은 호환 DTO를 만들지 않고 이 공개 계약을 직접 사용한다.

## ArtifactRef 계약

큰 출력과 파일은 계약에 복사하지 않고 ArtifactRef로 연결한다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `artifact_id` | ArtifactId | 안정 ID |
| `kind` | stable enum/string | log, report, diff, screenshot, trace 등 |
| `project_id` | optional ProjectId | 프로젝트 소속 자료 |
| `relative_path` | normalized path | artifact root 기준 위치 |
| `media_type` | string | MIME type |
| `size_bytes` | non-negative integer | 저장 크기 |
| `sha256` | lowercase hex | 저장 byte의 hash |
| `created_at` | UTC timestamp | 생성 시각 |
| `producer` | ProducerRef | 생성 component·tool |
| `redaction_status` | enum | `not_needed`, `redacted`, `quarantined`, `unknown` |
| `retention_class` | enum | `temporary`, `run`, `evidence`, `hold` |
| `source_artifact_ref` | optional ArtifactRef | redaction·변환 전 자료 |

경로는 artifact root를 벗어날 수 없고 소비자는 읽기 전에 size와 hash를 확인한다. `quarantined` 또는 `unknown` 자료는 기본 report와 MCP 응답에 포함하지 않는다.

## EvidenceBundle과 ReviewPack

EvidenceBundle은 실행 사실을 기계가 읽는 정본으로 묶고 ReviewPack은 사람이 판단하기 쉬운 순서로 참조한다.

### EvidenceBundle

- GoalSpec, StageGraph와 최종 revision reference
- 각 Stage의 RouteDecision, PermissionPlan, 결과와 Checkpoint
- 변경 전·후 fingerprint와 변경 파일 목록
- ValidationPlan, ValidationRun, Diagnostic과 GateDecision reference
- approval, retry, escalation, pause와 recovery event 구간
- CostRecord와 측정되지 않은 usage 항목
- merge 결과, remaining risk와 Handoff
- bundle manifest에 포함 artifact의 ID, hash, size와 redaction 상태

EvidenceBundle은 원문 로그를 inline으로 넣지 않고 ArtifactRef만 가진다. `complete`, `partial`, `unverified` 중 bundle completeness를 표시하고 빠진 이유를 적는다.

### ReviewPack

- 사용자가 요청한 목표와 완료 조건
- 계획 대비 실제 변경 요약
- 중요한 diff·설계 결정·permission·approval
- 검사 결과를 통과, 실패, 미실행, 확인 불가로 분리한 표
- 독립 검토 결과와 아직 남은 위험
- 비용 발생과 사용량
- 다음 선택지, 이어하기 또는 rollback 위치
- EvidenceBundle ID와 hash

ReviewPack은 evidence를 새로 해석해 사실을 바꾸지 않는다. 사람이 읽는 Markdown과 같은 내용을 가진 구조화 JSON을 함께 만들 수 있다.

## ReproductionPack과 CostRecord

### ReproductionPack

실패 재현 묶음에는 최소 입력, Project revision, tool·config·Catalog·Capability snapshot, 구조화된 실행 인자, 기대 결과, 실제 결과와 redaction한 artifact를 둔다. secret, 사용자 절대 경로와 전체 repository 사본은 포함하지 않는다. 재현할 수 없는 외부 조건은 `unverified` dependency로 명시한다.

### CostRecord

| 필드 | 의미 |
|---|---|
| `scope` | goal, stage, attempt 또는 외부 동작 |
| `source` | Codex, tool, 외부 서비스 등 측정 주체 |
| `usage` | token, duration, invocation count처럼 실제 받은 단위 |
| `monetary_cost` | provider가 검증 가능한 금액을 제공한 경우만 기록 |
| `currency` | 금액이 있을 때의 ISO currency |
| `price_source_ref` | 가격·청구 근거 artifact |
| `estimated` | 추정 여부. 금액은 기본적으로 `false`만 허용 |
| `paid_action` | PermissionPlan의 유료 동작 판정 |

가격을 알 수 없을 때 0원으로 기록하지 않고 금액 필드를 생략하며 `measurement_unavailable` 이유를 남긴다.

### BudgetSnapshot

BudgetSnapshot은 한 시점의 실행 가능 한도를 표현한다.

| 필드 | 의미 |
|---|---|
| `scope` | goal, stage 또는 attempt |
| `limits` | 시간, attempt, 병렬 수, artifact, paid action과 검증된 금액 한도 |
| `observed` | CostRecord에서 실제 측정한 양 |
| `reserved` | 이미 시작했지만 끝나지 않은 operation의 보수적 예약량 |
| `remaining` | 같은 단위로 계산 가능한 잔여량 |
| `unknown_measurements` | 측정할 수 없는 비용·usage와 영향 |
| `decision` | `within_budget`, `approval_required`, `exhausted`, `unknown` |
| `config_fingerprint` | 한도 출처 |
| `evaluated_at` | 계산 시각 |

측정 단위가 다른 값은 합산하지 않는다. 비용을 모르면 `remaining=0`으로 만들지 않고 `unknown` 또는 승인 필요로 판단한다.

## RemoteStateSnapshot 계약

원격 Git·PR·check·release 상태는 로컬 상태와 분리해 snapshot으로 기록한다.

| 필드 | 의미 |
|---|---|
| `remote_snapshot_id` | 문서 ID |
| `project_id`, `remote_kind` | 대상과 Git·GitHub 등 adapter 종류 |
| `remote_identity` | secret을 제거한 host·repository identity |
| `captured_at` | 조회 완료 시각 |
| `refs` | branch, commit, PR, check, release의 typed reference |
| `status_items` | open·merged·success·failure 등 정규화 상태 |
| `capabilities` | 조회·push·PR·merge 가능 여부 |
| `freshness` | 실행 전 다시 확인할 조건 |
| `limitations` | 권한 부족·부분 조회·provider 차이 |
| `raw_artifact_ref` | redaction한 adapter 응답 |

원격 변경 command는 이 snapshot과 대상 revision을 precondition으로 사용한다. stale이면 push·merge·release 전에 다시 조회하고 PermissionPlan을 확인한다.

## EvaluationRun 계약

EvaluationRun은 Router, Profile, Check와 정책 후보가 실제로 1인 개발자의 작업을 개선하는지 비교한다.

| 필드 | 의미 |
|---|---|
| `evaluation_run_id` | 문서 ID |
| `subject` | route rule, Profile, Check, policy의 ID와 version |
| `baseline`, `candidate` | 비교 대상 snapshot |
| `mode` | `offline`, `replay`, `shadow` |
| `corpus_ref` | 실제 사례를 비식별화한 평가 자료와 version |
| `case_result_refs` | 사례별 성공·실패·Diagnostic artifact |
| `metrics` | 성공, gate, 재작업, 시간, usage, 사용자 수정·수락·되돌림 |
| `limitations` | 표본·측정·외부 조건의 한계 |
| `comparison` | 개선·악화·불확실한 항목 |
| `recommendation` | `keep`, `trial`, `accept`, `reject`, `needs_review` |
| `decision_ref` | 실제 규칙 변경 승인과 ADR·config change |

shadow mode는 실제 작업의 route, permission, 검사와 파일을 바꾸지 않는다. EvaluationRun의 recommendation만으로 Catalog나 설정을 자동 갱신하지 않는다.

## ReleaseManifest 계약

ReleaseManifest는 Star-Control 자체 또는 대상 프로젝트 release 후보의 신원과 준비 증거를 묶는다.

| 필드 | 의미 |
|---|---|
| `release_manifest_id` | 문서 ID |
| `product_id`, `version`, `channel` | release identity |
| `source_revisions` | project별 source revision |
| `artifacts` | 파일 이름, 크기, media type, SHA-256과 ArtifactRef |
| `included_files_manifest_ref` | package에 실제 포함한 파일 목록 |
| `sbom_ref`, `provenance_ref`, `signature_refs` | 대상에 필요할 때의 공급망 자료 |
| `compatibility` | OS, config·state·schema, 설치·update 범위 |
| `validation_refs` | clean build, test, install, update, rollback, uninstall 결과 |
| `gate_decision_ref` | release readiness 판단 |
| `approval_request_ref` | publish·deploy 승인 |
| `rollback_plan_ref` | 실패 시 돌아갈 artifact와 절차 |
| `status` | `draft`, `candidate`, `ready`, `blocked`, `published`, `withdrawn` |

`ready`는 publish됐다는 뜻이 아니다. `published`는 원격 결과를 다시 확인한 event와 RemoteStateSnapshot이 있을 때만 기록한다. 필요하지 않은 SBOM·서명 field는 생략하되 생략 이유를 gate evidence에 둔다.

## 검사 선택 기준

- 무엇을 바꿨는가
- 실패했을 때 피해가 얼마나 큰가
- 프로젝트가 제공하는 공식 검사 명령이 있는가
- 빠른 부분 검사가 가능한가
- 전체 검사에 걸리는 시간
- 화면이나 외부 서비스처럼 실제 동작 확인이 필요한가
- 이전에 같은 부분에서 실패했는가

## 항상 수행할 가벼운 확인

- 작업 시작 전 기존 변경 상태 기록
- 실제 변경 파일 목록 확인
- 계획과 변경 목적 비교
- 명령 종료 결과 확인
- 비밀정보 노출 확인
- 검사 실패 누락 여부 확인
- 완료 증거와 이어하기 기록 생성 여부 확인

이 확인도 작업 종류와 무관한 파일을 모두 읽는 방식으로 구현하지 않는다.

## 필요할 때만 수행할 검사

| 변경 종류 | 필요한 검사 예시 |
|---|---|
| 문서 | 내부 연결, 중복 기준, 맞춤법, 명령 예시 |
| 코드 | 형식, 코드 검사, 빌드, 관련 테스트 |
| 공개 사용 약속 | 이전 사용 방식과의 호환, 예제, 문서 |
| 새 의존 항목 | 필요성, 잠금 파일, 보안, 라이선스 |
| 설정 | 읽기, 잘못된 값, 기본값, 이전 설정 호환 |
| 파일 저장 | 중단 복구, 동시 접근, 손상 방지 |
| 화면 | 실제 실행, 주요 흐름, 오류 표시 |
| 원격 작업 | 대상, 권한, 상태, 실행 결과 |
| 병합 | 충돌, 변경 겹침, 통합 검사 |
| 배포 | 버전, 산출물, 설치, 되돌리기 |

## 검사 단계

### 작업 중 빠른 검사

현재 고친 부분과 직접 관련된 가장 빠른 검사를 실행한다.

### 단계 종료 검사

단계 목적과 관련된 모든 필수 검사를 실행한다.

### 목표 종료 검사

여러 단계가 합쳐진 최종 상태에서 필요한 전체 검사를 실행한다. 전체 검사가 지나치게 비싸고 같은 증거를 반복한다면 생략 이유와 대체 증거를 기록한다.

## 테스트가 없는 프로젝트

- 현재 프로젝트가 제공하는 실행 방법과 예제를 먼저 사용한다.
- 변경 위험이 크면 필요한 최소 테스트를 추가한다.
- 테스트 추가 자체가 과도하면 수동 확인 방법과 남은 위험을 기록한다.
- 테스트가 없다는 이유로 검증한 것처럼 보고하지 않는다.

## 실제 동작 확인

화면, 서버, 외부 도구 연동처럼 코드 검사만으로 알 수 없는 기능은 실제 경로를 실행한다. 가능하면 가짜 입력만 확인하지 않고 실제 사용자 경로를 확인한다.

## 자동 수정과 재검사

- 승인된 단계 범위 안의 실패는 자동 수정할 수 있다.
- 같은 실패를 무한 반복하지 않는다.
- 검사를 통과시키기 위해 테스트를 삭제하거나 기준을 약화하면 안 된다.
- 새로운 위험이나 유료 동작이 생기면 새 단계 또는 승인으로 전환한다.

## 독립 검토

다음 변경은 처음 작업한 Codex와 분리된 검토를 기본으로 한다.

- 공개 사용자와의 약속
- 권한과 승인 정책
- 비밀정보 처리
- 파일 손상 또는 복구
- 병합과 배포
- 중요한 모델 배정 규칙
- 반복 실패 뒤의 최종 변경

## 자동 완료 조건

다음 조건을 만족하면 별도 사람 승인 없이 완료 처리할 수 있다.

- 목표의 완료 조건 충족
- 필수 단계 모두 완료
- 필요한 검사 통과
- 실패와 생략된 검사가 숨겨지지 않음
- 병렬 변경이 모두 통합됨
- 완료 증거 생성
- 남은 위험 기록
- 비용 한도 위반 없음

사용자는 설정으로 최종 사람 승인을 추가할 수 있다.

## 증거 묶음

최종 보고에는 다음을 포함한다.

- 무엇을 요청받았는지
- 어떤 단계로 처리했는지
- 실제로 무엇을 바꿨는지
- 어떤 검사를 실행했는지
- 통과, 실패, 생략 결과
- 범위가 어떻게 달라졌는지
- 모델과 실행 방식을 왜 선택했는지
- 재시도와 승급 내역
- 사용량과 유료 동작
- 남은 위험
- 다음에 이어갈 위치

## 로그 원칙

- 사용자에게는 핵심 요약과 실패 위치를 먼저 보여준다.
- 전체 원문이 재현에 필요하면 별도 파일로 저장한다.
- 반복되는 출력은 압축한다.
- 비밀정보는 저장 전에 가린다.
- 검사를 실행하지 않았으면 미실행이라고 기록한다.
