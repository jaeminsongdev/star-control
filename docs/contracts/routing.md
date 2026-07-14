# 모델·생각 깊이·실행 방식 배정

## 배정 목표

가장 싼 모델을 고르는 것이 목표가 아니다. 요구 품질과 안전을 만족하면서 실패, 재작업, 기다리는 시간까지 포함한 총비용을 줄이는 것이 목표다.

## 모델 역할 등급

현재 Codex의 Sol, Terra, Luna를 다음 역할로 사용한다.

| 역할 | 기본 용도 |
|---|---|
| Sol | 애매하고 중요한 설계, 고위험 변경, 최종 판단, 깊은 조사 |
| Terra | 일반적인 구현, 여러 파일 변경, 명확한 구조 개선 |
| Luna | 결과 기준이 분명한 반복 작업, 추출, 변환, 목록화 |

문서에는 역할 등급과 실제 모델 이름을 함께 저장한다. 모델 이름이 바뀌어도 역할 규칙은 유지하고 실행 시점에 실제 모델을 다시 찾는다.

## 생각 깊이

실행 계약에는 Codex 원시 값을 그대로 기록한다.

| 원시 값 | 문서상 의미 | 기본 용도 |
|---|---|---|
| `minimal` | 최소 판단 | 단순 추출, 정렬, 형식 변경 |
| `low` | 낮은 판단 | 결과 기준이 분명한 반복 작업 |
| `medium` | 일반 판단 | 일반 구현, 명확한 검사 추가, 문서 동기화 |
| `high` | 높은 판단 | 여러 파일 변경, 원인 분석, 구조 판단 |
| `xhigh` | 매우 높은 판단 | 복잡하고 불확실한 문제, 중요한 계약 검토 |

`minimal | low | medium | high | xhigh`는 `model_reasoning_effort`의 원시 값이다. `xhigh` 지원 여부는 모델마다 다를 수 있으므로 Codex App Server의 CapabilitySnapshot 결과를 기준으로 한다. Plan 단계에는 필요하면 별도의 plan-mode 생각 깊이를 기록한다.

Plan 전용 값은 `none | minimal | low | medium | high | xhigh`다. `none`은 Plan 전용 생각 깊이를 요청하지 않는다는 뜻이며 일반 `reasoning_effort`에는 사용할 수 없다. Plan 값도 현재 Codex와 선택 모델이 지원하는 범위에서만 사용한다.

## Max와 Ultra

Max와 Ultra는 일반 생각 깊이 값이 아니라 Star-Control의 `execution_mode`다.

- `single`: 하나의 Codex 작업으로 처리하는 기본 방식
- `max`: 하나의 어려운 단계에 더 많은 판단 시간이나 강화된 단일 실행 방식을 요청하는 방식
- `ultra`: 나눌 수 있는 큰 단계를 여러 Codex가 병렬로 조사하거나 실행하고 결과를 통합하는 방식

Ultra를 선택해도 최종 통합 판단은 하나의 Sol 단계가 담당한다.

Codex가 실행 시점에 `max` 또는 `ultra`에 해당하는 native 기능을 제공하면 그것을 우선 사용한다. 제공하지 않으면 지원되지 않는 값을 원시 설정으로 보내지 않는다. `ultra`는 Star-Control이 여러 Codex 작업을 만들고 결과를 통합하는 관리형 병렬 실행으로 실현할 수 있고, `max`는 지원되는 단일 실행 경로가 없으면 안전한 생각 깊이와 `single` 방식으로 다시 배정한다.

Plan은 `execution_mode`가 아니라 `stage_mode=plan`으로 기록한다. 따라서 한 RouteDecision은 모델 역할, 원시 생각 깊이, 단계 성격, 실행 방식을 각각 가진다.

## RouteDecision 계약

`RouteDecision`은 계획에 적힌 희망값과 실행 직전에 확인한 실제값을 함께 남긴다. 공통 Envelope의 `document_id`는 RouteDecisionId이고, 다음 필드를 가진다.

RouteDecision은 `StageSpec.executor_kind=codex`인 Stage에만 적용한다. M1 Project scan·index와 M2 CLI-only change planning처럼 `executor_kind=deterministic_local`인 application stage에는 모델·생각 깊이·CapabilitySnapshot을 합성하지 않는다. 사용자가 CLI로 계획을 요청해도 Codex가 plan을 생성했다는 RouteDecision을 만들지 않는다.

| 필드 | 형식 | 필수 | 의미 |
|---|---|---:|---|
| `goal_id` | GoalId | 예 | 소속 목표 |
| `run_id` | RunId | 예 | 소속 실행 세대 |
| `stage_id` | StageId | 예 | 배정 대상 단계 |
| `stage_revision` | integer | 예 | 판단에 사용한 StageSpec revision |
| `decision_kind` | enum | 예 | `initial`, `retry`, `escalation`, `user_override` |
| `model_role` | enum | 예 | `sol`, `terra`, `luna` 중 역할 |
| `requested_model` | string | 아니요 | 사용자가 특정 모델을 지정한 경우의 원문 ID |
| `resolved_model` | string | 예 | CapabilitySnapshot에서 실제 선택한 모델 ID |
| `requested_reasoning_effort` | enum | 아니요 | 사용자 또는 Profile이 요청한 원시 생각 깊이 |
| `reasoning_effort` | enum | 예 | 실제 전송할 `minimal`, `low`, `medium`, `high`, `xhigh` |
| `plan_reasoning_effort` | enum | 아니요 | `stage_mode=plan`일 때 별도 Plan 생각 깊이 |
| `stage_mode` | enum | 예 | `plan`, `execute`, `review` |
| `requested_execution_mode` | enum | 아니요 | 요청된 `single`, `max`, `ultra` |
| `execution_mode` | enum | 예 | 실제로 채택한 `single`, `max`, `ultra` |
| `execution_realization` | enum | 예 | Codex native 기능이면 `native`, Star-Control 조립이면 `managed` |
| `capability_snapshot_ref` | ArtifactRef | 예 | 판단 당시 지원 기능의 불변 snapshot |
| `config_fingerprint` | SHA-256 | 예 | 판단에 사용한 EffectiveConfig 식별값 |
| `risk_level` | enum | 예 | `low`, `medium`, `high`, `critical` |
| `parallelizable` | boolean | 예 | 독립 실행 가능한 부분으로 나눌 수 있는지 |
| `estimated_usage_class` | enum | 예 | `small`, `standard`, `large`, `unknown` |
| `confidence` | enum | 예 | `low`, `medium`, `high` |
| `rationale` | string array | 예 | 선택 근거. 빈 배열은 허용하지 않음 |
| `alternatives` | RouteAlternative array | 예 | 검토했지만 선택하지 않은 경로와 이유 |
| `fallback_chain` | RouteFallback array | 예 | 지원 거부·실패 시 순서가 있는 대체 경로 |
| `permission_plan_ref` | document ref | 예 | 이 경로에서 허용된 행동 범위 |

`RouteAlternative`와 `RouteFallback`은 전체 RouteDecision을 복제하지 않는다. 달라지는 모델·생각 깊이·실행 방식, 적용 조건과 선택하지 않은 이유만 기록한다.

### RouteDecision 불변식

1. `resolved_model`과 `reasoning_effort`는 참조한 CapabilitySnapshot에서 지원되어야 한다.
2. `native`는 snapshot이 해당 실행 방식을 명시적으로 지원할 때만 사용할 수 있다.
3. 관리형 Ultra는 서로 독립적인 하위 실행, 각 결과의 출처와 하나의 최종 통합 단계를 가져야 한다.
4. 요청값을 낮추거나 다른 방식으로 바꾸면 요청값, 실제값과 변경 이유를 모두 남긴다.
5. 사용자 지정도 permission, budget, Codex·관리자 제한을 넘을 수 없다.
6. StageSpec revision, EffectiveConfig fingerprint 또는 CapabilitySnapshot이 달라지면 기존 결정은 그대로 재사용하지 않고 다시 판단한다.
7. 실패 뒤 재배정은 이전 RouteDecision을 수정하지 않고 `decision_kind=retry` 또는 `escalation`인 새 revision으로 남긴다.
8. 가격 근거가 없을 때 `estimated_usage_class`를 금액으로 바꾸지 않는다.

## CapabilitySnapshot 계약

CapabilitySnapshot은 Codex의 이름을 장기 계약으로 고정하지 않기 위한 실행 시점 자료다. App Server가 돌려준 원문은 ArtifactRef로 보존하고 core에는 다음 정규화된 필드만 전달한다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `source` | enum | 현재는 `codex_app_server` |
| `captured_at` | RFC 3339 UTC | 조회 완료 시각 |
| `codex_version` | string | 확인 가능한 경우의 Codex version |
| `protocol_version` | string | 사용한 App Server protocol version |
| `models` | ModelCapability array | 모델 ID, 표시 이름, 지원 생각 깊이, 기본 생각 깊이 |
| `operations` | map<boolean> | thread, turn, steer, interrupt, review, goal 등 확인된 동작 |
| `native_execution_modes` | enum set | native로 확인된 Star-Control 대응 실행 방식 |
| `managed_execution_modes` | enum set | 현재 Controller가 조립할 수 있는 실행 방식 |
| `permission_capabilities` | object | 사용 가능한 approval·sandbox 경계 |
| `limits` | map | 병렬 수 등 실제로 확인된 제한만 기록 |
| `limitations` | string array | adapter가 정규화하지 못했거나 지원하지 않는 항목 |
| `raw_artifact_ref` | ArtifactRef | redaction한 App Server 응답 원문 |

ModelCapability은 최소한 `model_id`, `display_name`, `supported_reasoning_efforts`, `default_reasoning_effort`를 가진다. 역할 등급은 제품의 판단이므로 snapshot 원문에 쓰지 않고 Router가 별도로 해석한다.

### Snapshot 생성과 만료

- Controller 시작, Goal 실행 시작과 Codex가 기존 선택을 거부했을 때 새로 조회한다.
- 설정된 `capability_max_age_ms`를 넘은 snapshot은 새 실행 결정에 사용하지 않는다.
- 조회 실패 시 마지막 snapshot을 현재 사실처럼 사용하지 않는다. 이미 실행 중인 단계의 복구에만 provenance와 함께 제한적으로 사용할 수 있다.
- 동일한 run에서도 지원 기능이 바뀌면 새 snapshot ID를 만들고 영향을 받는 미실행 RouteDecision을 다시 계산한다.
- 원문에 없던 능력은 추정해 `true`로 만들지 않는다.

## 판단에 사용하는 정보

- 목표가 얼마나 애매한가
- 변경 실패 시 피해가 얼마나 큰가
- 자동 검사가 얼마나 강한가
- 여러 파일과 프로젝트에 걸치는가
- 처음 보는 문제인가
- 동시에 나눌 수 있는가
- 이전 시도가 실패했는가
- 사용 가능한 비용과 시간
- 결과를 사람이 쉽게 판단할 수 있는가

## 기본 배정

| 단계 성격 | 기본 배정 |
|---|---|
| 기준이 분명한 반복 작업 | Luna Low 또는 Medium |
| 일반 구현 | Terra Medium |
| 여러 파일 변경과 원인 분석 | Terra High |
| 애매한 설계와 공개 약속 | Sol High |
| 보안, 데이터 손상, 병합 정책 | Sol High 또는 xhigh |
| 중요한 설계가 반복 실패 | Sol Max |
| 독립적으로 나눌 수 있는 큰 조사 | Sol Ultra 또는 관리형 병렬 실행 |
| 고위험 최종 검토 | 처음 작업과 분리된 Sol 검토 |

## 자동 승급

1. 같은 모델과 같은 방법의 단순 재시도는 기본 한 번만 허용한다.
2. 실패 원인을 반영할 수 있으면 생각 깊이를 한 단계 올린다.
3. 같은 원인이 반복되면 더 강한 모델로 새 계획을 만든다.
4. 강한 모델도 실패하면 작업을 더 잘게 쪼개는 것이 아니라 실패 원인이 다른 단계인지 다시 판단한다.
5. 유료 한도나 재시도 한도에 도달하면 중단하고 사용자에게 알린다.

재시도 횟수와 승급 규칙은 설정 파일과 명령어로 바꿀 수 있다.

## 사용자 우선권

사용자가 모델, 생각 깊이, Max, Ultra를 직접 지정하면 자동 배정보다 우선한다. 다만 현재 Codex가 지원하지 않는 선택은 가장 가까운 지원 방식으로 바꾸기 전에 사용자에게 차이를 알린다.

## 비용 제어

- 단계별 예상 사용량 등급을 계획에 표시한다.
- 사용자 계정에서 실제 금액을 알 수 없으면 거짓 가격을 계산하지 않는다.
- 유료 동작은 실행 전에 승인받는다.
- 실패와 재작업을 비용에 포함한다.
- 하루, 목표, 단계별 한도를 설정할 수 있다.
- 품질과 안전 기준을 낮춰 한도를 맞추지 않는다.

## 비교 시험

실제 개발 작업 모음을 사용해 다음을 비교한다.

- 성공 여부
- 검사 통과 여부
- 재작업 횟수
- 걸린 시간
- 사용량
- 사람이 고친 양
- 잘못 건드린 범위

규칙 변경은 비교 결과와 이유를 기록한 뒤 적용한다.

## 공식 근거

- [Codex 모델 선택](https://developers.openai.com/codex/models/)
- [Codex 설정 Reference](https://developers.openai.com/codex/config-reference/)
- [App Server API 개요](https://learn.chatgpt.com/docs/app-server#api-overview)
