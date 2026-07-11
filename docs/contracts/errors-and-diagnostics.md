# 오류와 진단 계약

## 목적

오류는 요청을 수행하지 못한 이유이고 Diagnostic은 결과나 상태에서 발견한 문제다. 문자열 log를 해석해 성공·실패를 판단하지 않도록 안정 code, severity, 위치, 근거와 다음 행동을 구조화한다.

## 구분

| 종류 | 질문 | 예시 |
|---|---|---|
| ErrorEnvelope | 요청 자체가 왜 처리되지 않았는가 | 잘못된 설정, revision 충돌, process 시작 실패 |
| Diagnostic | 처리한 결과에서 무엇이 문제인가 | 테스트 실패, 계약 위반, 의심되는 secret 노출 |
| EventEnvelope | 실제로 무슨 일이 일어났는가 | 승인 요청, Stage 시작, 검사 종료 |
| GateDecision | 이 결과로 진행할 수 있는가 | 자동 통과, 사용자 검토, 차단 |

하나의 실패는 error event와 Diagnostic을 모두 만들 수 있지만 서로 대신하지 않는다.

## ErrorEnvelope 계약

| 필드 | 형식 | 의미 |
|---|---|---|
| `schema_id` | string | `star.error` |
| `schema_version` | integer | 계약 version |
| `code` | stable uppercase string | 기계 판단용 code |
| `category` | enum | 오류 영역 |
| `message` | string | 사용자가 이해할 짧은 설명 |
| `retryable` | boolean | 같은 요청을 안전하게 재시도할 수 있는지 |
| `retry_after_ms` | optional integer | 실제 근거가 있을 때의 대기 시간 |
| `user_action` | optional object | 사용자가 할 수 있는 다음 행동과 command hint |
| `context` | redacted map | ID, key, 기대·실제 type 등 최소 진단 문맥 |
| `correlation_id` | string | MCP·IPC·event·log 연결 |
| `caused_by` | optional ErrorRef | 더 낮은 원인 code와 safe summary |
| `artifact_refs` | ArtifactRef array | 격리된 상세 log·report |
| `occurred_at` | UTC timestamp | 오류 생성 시각 |
| `component` | string | 오류를 정규화한 component |

`message` text를 분기 조건으로 사용하지 않는다. 내부 stack, OS 사용자 이름, secret, 전체 환경과 외부 provider 원문은 `context`에 넣지 않는다. 원인이 여러 겹이면 bounded cause chain으로 보존하고 반복·순환 reference를 거부한다.

## 오류 namespace

code는 `<CATEGORY>_<SPECIFIC_REASON>` 형식이며 이미 배포한 의미를 바꾸지 않는다.

| category | 대표 code | 의미 |
|---|---|---|
| `config` | `CONFIG_PARSE_FAILED`, `CONFIG_UNKNOWN_KEY`, `CONFIG_CONSTRAINT_CONFLICT` | 설정 읽기·병합·제약 실패 |
| `contract` | `CONTRACT_SCHEMA_INVALID`, `CONTRACT_VERSION_UNSUPPORTED` | 데이터 계약 위반 |
| `state` | `STATE_REVISION_CONFLICT`, `STATE_IDEMPOTENCY_CONFLICT`, `STATE_CORRUPT_LOG` | 상태·동시성·복구 문제 |
| `policy` | `POLICY_DENIED`, `POLICY_APPROVAL_REQUIRED`, `POLICY_APPROVAL_STALE` | 권한·승인 결과 |
| `route` | `ROUTE_NO_SUPPORTED_MODEL`, `ROUTE_MODE_UNAVAILABLE`, `ROUTE_BUDGET_EXCEEDED` | 실행 배정 실패 |
| `tool` | `TOOL_MANIFEST_INVALID`, `TOOL_DESCRIPTOR_STALE`, `TOOL_LANE_MISMATCH`, `TOOL_EXECUTABLE_UNTRUSTED`, `TOOL_EXECUTABLE_INCOMPATIBLE`, `TOOL_PROTOCOL_INVALID` | live Tool Registry·실행 protocol 문제 |
| `codex` | `CODEX_NOT_READY`, `CODEX_PROTOCOL_MISMATCH`, `CODEX_OPERATION_LOST` | Plugin·MCP·App Server 연동 문제 |
| `validation` | `VALIDATION_CHECK_FAILED`, `VALIDATION_TOOL_ERROR`, `VALIDATION_INCOMPLETE` | 검사 실패·미확인 |
| `vcs` | `VCS_DIRTY_TARGET`, `VCS_CONFLICT`, `VCS_REMOTE_REJECTED` | worktree·merge·원격 문제 |
| `ipc` | `IPC_CONTROLLER_UNAVAILABLE`, `IPC_AUTH_FAILED`, `IPC_SERVER_IDENTITY_MISMATCH`, `IPC_PROTOCOL_MISMATCH`, `IPC_FRAME_INVALID`, `IPC_BACKPRESSURE` | local 통신 문제 |
| `release` | `RELEASE_GATE_BLOCKED`, `RELEASE_ARTIFACT_INVALID` | 배포 gate·산출물 문제 |
| `internal` | `INTERNAL_INVARIANT_BROKEN` | 예상하지 못한 제품 결함 |

외부 tool의 종료 code와 문구는 adapter에서 위 code 또는 Diagnostic rule로 정규화하고 원문은 ArtifactRef로 둔다.

### Live Tool Registry 대표 오류

| code | 발생 조건 | 기본 행동 |
|---|---|---|
| `TOOL_NOT_FOUND` | 최신 usable snapshot에 ToolId가 없음 | search를 다시 수행 |
| `TOOL_SEARCH_CURSOR_STALE` | cursor snapshot과 현재 snapshot이 다름 | 같은 query로 첫 page부터 다시 검색 |
| `TOOL_REGISTRY_CURSOR_STALE` | status cursor의 Registry·diagnostic revision 또는 filter가 달라짐 | 같은 filter로 첫 page부터 다시 조회 |
| `TOOL_MANIFEST_INVALID` | TOML·Schema·reference·binding이 유효하지 않음 | 해당 package의 last-known-good 유지 또는 비활성 |
| `TOOL_PACKAGE_CONFLICT` | 같은 ID·version이 다른 내용이거나 replacement graph가 충돌 | 우선순위 덮기 없이 candidate 거부 |
| `TOOL_REQUIRED_PACKAGE_INVALID` | required core package를 안전하게 읽지 못함 | core workflow를 닫힌 상태로 차단 |
| `TOOL_DESCRIPTOR_STALE` | describe 뒤 현재 descriptor hash가 달라짐 | side effect 없이 거부하고 다시 describe |
| `TOOL_LANE_MISMATCH` | descriptor보다 낮거나 다른 risk lane으로 invoke | side effect 없이 거부 |
| `TOOL_ARGUMENT_INVALID` | input Schema·binding·NUL·command-line 제한 위반 | process 시작 전에 입력 수정 요구 |
| `TOOL_EXECUTABLE_NOT_FOUND` | resolved executable 또는 필수 integrity file이 없음 | unavailable, 자동 설치하지 않음 |
| `TOOL_EXECUTABLE_CHANGED` | path의 file identity·hash·version이 달라짐 | update policy에 따라 probe 또는 거부 |
| `TOOL_EXECUTABLE_UNTRUSTED` | source·path·hash·trust 조건이 맞지 않음 | trust 절차 전까지 실행 금지 |
| `TOOL_EXECUTABLE_INCOMPATIBLE` | architecture·version·protocol·probe가 선언과 다름 | 새 candidate를 publish하지 않음 |
| `TOOL_SIGNATURE_INVALID` | required Authenticode chain·subject가 맞지 않음 | process 시작 전 unavailable |
| `TOOL_INTEGRITY_INVALID` | EXE·DLL·runtime sidecar identity가 선언과 다름 | process 시작 전 unavailable |
| `TOOL_ISOLATION_UNAVAILABLE` | required AppContainer adapter 경계를 만들 수 없음 | weaker profile로 자동 하향하지 않음 |
| `TOOL_UPDATE_POLICY_DENIED` | source에서 허용되지 않는 `follow_path` 등 사용 | package 거부와 수정 안내 |
| `TOOL_PROCESS_START_FAILED` | 검증 뒤 `CreateProcessW`가 실패 | OS code를 redaction해 반환, 자동 재시도 없음 |
| `TOOL_TIMEOUT` | 선언·EffectiveConfig의 process deadline 초과 | cancel grace 뒤 Job 종료와 outcome 확인 |
| `TOOL_PROTOCOL_INVALID` | JSON frame·encoding·exit 의미·response Schema 위반 | process 종료, 원문 격리, 실행 실패 |
| `TOOL_OUTPUT_LIMIT` | stdout·stderr·progress·artifact 제한 초과 | 성공으로 자르지 않고 artifact 또는 명확한 실패 |
| `TOOL_REGISTRY_LIMIT` | package·action·Schema·watch root 상한 초과 | 초과 candidate만 거부 |
| `TOOL_OUTCOME_UNKNOWN` | crash·강제 종료 뒤 side effect 결과를 확정할 수 없음 | 자동 재실행 금지, evidence 검토 |

Registry 오류는 MCP 연결을 자동 종료하지 않는다. required core package가 아닌 한 정상 package의 search와 실행은 계속 가능해야 한다.

## Diagnostic 계약

| 필드 | 형식 | 의미 |
|---|---|---|
| `diagnostic_id` | DiagnosticId | 한 발견 instance |
| `rule_id` | stable Catalog ID | 발견 규칙과 version |
| `title` | string | 짧은 문제 이름 |
| `message` | string | 영향과 관찰 사실 |
| `severity` | enum | `info`, `warning`, `error`, `critical` |
| `confidence` | enum | `low`, `medium`, `high` |
| `status` | enum | `confirmed`, `suspected`, `unverified`, `suppressed`, `resolved` |
| `scope` | object | Goal·Run·Stage·ValidationRun revision |
| `locations` | LocationRef array | 파일·line·symbol 등 |
| `evidence_refs` | ArtifactRef array | 판단 근거 |
| `fingerprint` | SHA-256 | 같은 문제 중복 묶기 |
| `remediation` | optional object | 안전한 다음 행동과 자동 수정 가능성 |
| `suppression` | optional SuppressionRef | 승인된 예외, 범위, 이유와 만료 |
| `first_seen_at`, `last_seen_at` | UTC timestamp | 관찰 기간 |

공개 Rust type과 `star.diagnostic` JSON Schema는 `crates/foundation/star-contracts`가 소유한다. adapter는 severity, confidence, status, suppression 의미를 다시 정의하거나 suppression으로 원래 severity를 변경하지 않는다.

### severity와 confidence

- severity는 맞다고 가정했을 때의 영향이다.
- confidence는 그 판단이 사실일 가능성이다.
- `critical + low confidence`를 숨기지 않고 확인이 필요한 중대한 의심으로 표시한다.
- parser 실패나 증거 부족을 자동으로 `confirmed`로 올리지 않는다.

### 위치와 fingerprint

LocationRef는 ProjectId, project-relative path, 1-based start, exclusive end, optional symbol을 가진다. 절대 경로만 있는 외부 자료는 redacted LocalPathRef로 분리한다.

fingerprint는 rule ID, 정규화된 위치와 문제의 안정 특징으로 만든다. line 이동만으로 새 문제처럼 폭증하지 않도록 rule별 fingerprint input을 Catalog에 선언한다.

### suppression과 해결

- suppression은 Diagnostic을 삭제하거나 severity를 바꾸지 않는다.
- 대상 fingerprint 또는 rule, project scope, 이유, ActorRef, 생성·만료 시각을 가진다.
- 코드·설정 revision이 달라져 scope hash가 맞지 않으면 다시 검토한다.
- 다음 검사에서 관찰되지 않으면 기존 instance를 `resolved`로 표시하되 과거 evidence는 보존한다.

## 사용자 메시지 규칙

오류 응답은 다음 순서로 만든다.

1. 무엇이 멈췄는지
2. 실제 원인 code와 안전한 설명
3. 자동 재시도 가능한지
4. 사용자가 결정하거나 고칠 항목
5. 상세 근거 ArtifactRef 또는 correlation ID

내부 component 이름만 노출한 메시지, “unknown error”만 있는 메시지와 실행하지 않은 검사를 성공처럼 표현하는 메시지는 허용하지 않는다.

## CLI 종료 code

| code | 의미 |
|---:|---|
| `0` | 요청 성공 또는 조회 성공 |
| `2` | 입력·설정·계약이 잘못됨 |
| `3` | 승인·질문·정책으로 대기 또는 차단 |
| `4` | 실행·외부 도구·Controller 실패 |
| `5` | 검사 또는 gate가 변경을 차단 |
| `6` | protocol·schema·제품 version 비호환 |
| `7` | 제품 내부 invariant 실패 |

`accepted` 비동기 요청은 접수 자체가 성공했으므로 CLI가 기다리지 않는 명령에서는 0을 반환하되 OperationId를 출력한다. 완료를 기다리는 명령은 최종 상태의 code를 사용한다.

## 재시도 규칙

- `retryable=true`는 같은 idempotency key로 같은 payload를 보내도 side effect가 중복되지 않을 때만 설정한다.
- 입력 오류, deny, stale approval와 지원되지 않는 version은 수정 없이 재시도하지 않는다.
- timeout 뒤 operation 상태를 먼저 조회하고 새 실행을 만들지 않는다.
- 반복 실패는 마지막 error만 남기지 않고 attempt별 code와 하나의 root-cause summary로 묶는다.

## Redaction과 보관

- ErrorEnvelope와 Diagnostic을 저장하기 전에 built-in·project redaction rule을 적용한다.
- redaction이 확실하지 않은 원문은 `quarantined` artifact로 두고 기본 UI·MCP·report에 내보내지 않는다.
- code, timestamp, ID, hash와 redaction 결과는 감사 가능하도록 유지한다.
- debug mode도 secret 원문 허용 설정이 아니다.
