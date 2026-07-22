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
| `environment` | `DOCTOR_PROBE_UNREGISTERED`, `DOCTOR_PROBE_SIDE_EFFECT_FORBIDDEN`, `CLEAN_ROOM_SPEC_INCOMPLETE` | read-only 개발 환경·clean-room 진단 실패 |
| `state` | `STATE_REVISION_CONFLICT`, `STATE_IDEMPOTENCY_CONFLICT`, `STATE_CORRUPT_LOG` | 상태·동시성·복구 문제 |
| `policy` | `POLICY_DENIED`, `POLICY_APPROVAL_REQUIRED`, `POLICY_APPROVAL_STALE` | 권한·승인 결과 |
| `route` | `ROUTE_NO_SUPPORTED_MODEL`, `ROUTE_MODE_UNAVAILABLE`, `ROUTE_BUDGET_EXCEEDED` | 실행 배정 실패 |
| `planning` | `PLANNING_TASK_INPUT_INCOMPLETE`, `PLANNING_SCOPE_CONFLICT`, `PLANNING_INPUT_CHANGED` | Task·scope·plan coherence 실패 |
| `impact` | `IMPACT_REQUIRED_INPUT_UNAVAILABLE`, `IMPACT_OUTPUT_INVALID` | 영향 계산 필수 입력·결과 실패 |
| `affected` | `AFFECTED_REQUIRED_CHECK_UNRESOLVED`, `AFFECTED_SCOPE_UNBINDABLE` | required 검사·scope 선택 실패 |
| `registry` | `REGISTRY_MANIFEST_INVALID`, `REGISTRY_DUPLICATE_ID`, `REGISTRY_NAMESPACE_COLLISION`, `REGISTRY_ID_REUSE_FORBIDDEN` | Managed Registry source·identity·lifecycle·consumer 문제 |
| `tool` | `TOOL_MANIFEST_INVALID`, `TOOL_DESCRIPTOR_STALE`, `TOOL_LANE_MISMATCH`, `TOOL_EXECUTABLE_UNTRUSTED`, `TOOL_EXECUTABLE_INCOMPATIBLE`, `TOOL_PROTOCOL_INVALID` | live Tool Registry·실행 protocol 문제 |
| `codex` | `CODEX_NOT_READY`, `CODEX_PROTOCOL_MISMATCH`, `CODEX_OPERATION_LOST` | Plugin·MCP·App Server 연동 문제 |
| `validation` | `VALIDATION_CHECK_FAILED`, `VALIDATION_TOOL_ERROR`, `VALIDATION_INCOMPLETE`, `VALIDATION_SUBJECT_CHANGED`, `VALIDATION_PROFILE_CLOSURE_STALE` | 검사 실패·미확인·stale subject·plan closure |
| `reproduction` | `REPRODUCTION_INPUT_INCOMPLETE`, `REPRODUCTION_EXTERNAL_CONDITION_UNVERIFIED`, `RECOVERY_PLAN_INVALID` | 실패 재현·회귀·복구 계약 실패 |
| `security` | `SECURITY_DATA_STALE`, `SECURITY_DATA_SOURCE_UNVERIFIED`, `SECURITY_REDACTION_FAILED` | 외부 보안 자료·redaction 처리 실패 |
| `dependency` | `DEPENDENCY_INPUT_STALE`, `DEPENDENCY_MANAGER_UNREGISTERED`, `DEPENDENCY_UPDATE_REPLAN_REQUIRED` | dependency 관찰·update 준비 실패 |
| `maintenance` | `MAINTENANCE_STORE_INCOMPATIBLE`, `MAINTENANCE_RADAR_INPUT_STALE` | 공통 저장 계약·Radar projection 실패 |
| `migration` | `MIGRATION_VERSION_UNKNOWN`, `MIGRATION_OUTCOME_UNKNOWN`, `MIGRATION_ROLLBACK_FAILED` | migration version·chain·실행·복구 실패 |
| `performance` | `PERFORMANCE_MEASUREMENT_UNAVAILABLE`, `PERFORMANCE_NOT_COMPARABLE` | 측정 부재·비교 cohort 불일치 |
| `equivalence` | `LANGUAGE_EQUIVALENCE_INCOMPLETE`, `PLATFORM_RUNTIME_UNVERIFIED` | 언어 동등성·실제 플랫폼 검증 부족 |
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
| `TOOL_INTEGRITY_INVALID` | release catalog checksum 또는 EXE·DLL·runtime sidecar identity가 선언과 다름 | candidate 거부 또는 process 시작 전 unavailable |
| `TOOL_ISOLATION_UNAVAILABLE` | required AppContainer adapter 경계를 만들 수 없음 | weaker profile로 자동 하향하지 않음 |
| `TOOL_UPDATE_POLICY_DENIED` | source에서 허용되지 않는 `follow_path` 등 사용 | package 거부와 수정 안내 |
| `TOOL_PROCESS_START_FAILED` | 검증 뒤 `CreateProcessW`가 실패 | OS code를 redaction해 반환, 자동 재시도 없음 |
| `TOOL_TIMEOUT` | 선언·EffectiveConfig의 process deadline 초과 | cancel grace 뒤 Job 종료와 outcome 확인 |
| `TOOL_PROTOCOL_INVALID` | JSON frame·encoding·exit 의미·response Schema 위반 | process 종료, 원문 격리, 실행 실패 |
| `TOOL_OUTPUT_LIMIT` | stdout·stderr·progress·artifact 제한 초과 | 성공으로 자르지 않고 artifact 또는 명확한 실패 |
| `TOOL_REGISTRY_LIMIT` | package·action·Schema·watch root 상한 초과 | 초과 candidate만 거부 |
| `TOOL_OUTCOME_UNKNOWN` | crash·강제 종료 뒤 side effect 결과를 확정할 수 없음 | 자동 재실행 금지, evidence 검토 |

Registry 오류는 MCP 연결을 자동 종료하지 않는다. required core package가 아닌 한 정상 package의 search와 실행은 계속 가능해야 한다.

### 공통 개발 관리 오류

[공통 개발 관리 계약](development-management.md)의 repository category와 application precondition은 다음 stable code로 정규화한다.

| code | 조건 | 기본 행동 |
|---|---|---|
| `MANAGEMENT_STORE_UNAVAILABLE` | local store open·I/O 불가 | mutation 중단, recovery 안내 |
| `MANAGEMENT_STORE_BUSY` | single-writer lease·bounded contention 실패 | side effect 없이 retry 가능 여부 반환 |
| `MANAGEMENT_MIGRATION_REQUIRED` | 지원 과거 store version | status·plan·backup만 허용 |
| `MANAGEMENT_VERSION_UNSUPPORTED` | future 또는 지원 밖 version | read-only recovery 또는 거부 |
| `MANAGEMENT_INTEGRITY_FAILED` | relation·fingerprint·event·artifact 불일치 | read-write close, quarantine |
| `MANAGEMENT_READ_ONLY` | recovery mode에서 mutation 요청 | side effect 없이 거부 |
| `MANAGEMENT_REVISION_CONFLICT` | expected store·entity revision 불일치 | 최신 revision 반환 |
| `MANAGEMENT_IDEMPOTENCY_CONFLICT` | 같은 key에 다른 canonical input | 이전 결과를 덮지 않고 거부 |
| `MANAGEMENT_IDENTITY_CONFLICT` | 같은 derived ID에 다른 identity payload | merge 금지, store suspect |
| `PROJECT_NOT_ATTACHED` | root binding을 해석할 수 없음 | reattach 요구 |
| `PROJECT_ID_MISMATCH` | root의 shared ProjectId가 요청과 다름 | scan·write 금지 |
| `WORKSPACE_SNAPSHOT_STALE` | source·scope·file hash precondition 변경 | snapshot 재수집 |
| `SCAN_INCOMPLETE` | source·Rule 결과가 complete하지 않음 | Finding resolve·auto pass 금지 |
| `RULE_CONTRACT_INVALID` | Rule ID·version·identity·redaction 계약 오류 | 해당 Rule 실행 전 거부 |
| `CHANGE_PLAN_STALE` | Finding·Recipe·config·workspace precondition 변경 | 재계획 |
| `PATCH_PRECONDITION_FAILED` | file before hash·permission·plan revision 불일치 | patch 적용 전 거부 |
| `PATCH_POSTCONDITION_FAILED` | 실제 operation·after hash·mode·존재 상태가 immutable PatchSet 기대와 불일치 | B01 Diagnostic 보존·자동 완료 금지·복구 판단 |
| `VALIDATION_RESULT_STALE` | subject·plan·config fingerprint 불일치 | 재검증 |
| `RETENTION_PLAN_STALE` | store revision·candidate fingerprint 변경 | plan 재생성 |
| `MANAGEMENT_REDACTION_REJECTED` | 금지 값이 persistence boundary에 도달 | 저장하지 않고 completeness 하향 |

backend 고유 오류 이름·SQL text·database filename과 raw path를 code나 사용자 message에 넣지 않는다.

### 변경 계획·영향 분석 대표 오류

[변경 계획·영향 분석 계약](change-planning-and-impact.md)의 command failure는 다음 stable code로 정규화한다. partial·possible·fallback처럼 유효한 결과 안의 정확도 제한은 ErrorEnvelope가 아니라 ImpactAnalysis·ValidationPlan limitation/reason code다.

| code | 조건 | 기본 행동 |
|---|---|---|
| `PLANNING_TASK_INPUT_INCOMPLETE` | objective·Project·scope·완료 조건 필수 입력 누락 | 사용자 입력 요구, 추측 보완 금지 |
| `PLANNING_PROJECT_AMBIGUOUS` | Project·Checkout·symbol selector가 둘 이상과 일치 | stable ID 선택 요구 |
| `PLANNING_SCOPE_CONFLICT` | 같은 scope 축의 explicit include/exclude 충돌 또는 excluded planned change expansion | revision 생성 전 거부 |
| `PLANNING_INPUT_CHANGED` | 계산·commit 사이 Task·source·Catalog·Index fingerprint 변경 | 결과 publish 금지, 재계획 |
| `PLANNING_SNAPSHOT_STALE` | ready plan에 current required snapshot이 없음 | rescan 또는 possible+fallback 가능 여부 반환 |
| `PLANNING_OUTPUT_COHERENCE` | ChangePlan·ValidationPlan의 Task·Scope·Impact·ChangeSet ref 불일치 | 둘 다 ready publish 금지 |
| `PLANNING_USER_DECISION_REQUIRED` | proposed scope expansion·ambiguous target·waiver가 미해결 | user decision 대기 |
| `IMPACT_REQUIRED_INPUT_UNAVAILABLE` | required seed·graph partition을 어떤 safe fallback으로도 설명할 수 없음 | blocked ImpactAnalysis와 replan reason |
| `IMPACT_OUTPUT_INVALID` | edge path·fingerprint·certainty invariant 위반 | 결과 commit 금지, internal diagnostic |
| `AFFECTED_REQUIRED_CHECK_UNRESOLVED` | required family의 descriptor·tool·scope를 해결하지 못함 | ValidationPlan `blocked` |
| `AFFECTED_SCOPE_UNBINDABLE` | package/workspace/full 어느 invocation에도 typed scope bind 불가 | 임의 shell 금지, review/block |

사용자가 Check를 생략했다고 `AFFECTED_REQUIRED_CHECK_UNRESOLVED`를 pass로 바꾸지 않는다. explicit Waiver와 human review gate가 있으면 valid plan을 만들 수 있지만 원래 candidate outcome은 `user_waived`로 남긴다.

### 4단계 Patch·Refactor·codemod 대표 오류

[안전한 Patch·Refactor·codemod 엔진 계약](safe-patch-and-codemod.md)의 prepare·apply·recovery failure는 다음 stable code로 정규화한다. project source의 test·contract failure는 ErrorEnvelope가 아니라 M3 Diagnostic·GateDecision이다.

| code | 조건 | 기본 행동 |
|---|---|---|
| `RECIPE_CONTRACT_INVALID` | Recipe v2 ID·Schema·selector·pre/postcondition·assurance invariant 위반 | transformer 시작 전 거부 |
| `RECIPE_VERSION_CONFLICT` | 같은 Recipe ID/version에 다른 definition fingerprint | Catalog conflict, 어느 쪽도 실행 금지 |
| `RECIPE_INPUT_INVALID` | typed input이 Schema·redaction·limit을 통과하지 못함 | preview 생성 전 거부 |
| `RECIPE_TARGET_UNRESOLVED` | required typed selector target 0건·stale·partial | rescan/replan 또는 block |
| `RECIPE_TARGET_AMBIGUOUS` | selector multiplicity보다 많은 target 또는 ownership 충돌 | stable target 선택 요구 |
| `RECIPE_LANGUAGE_UNSUPPORTED` | target language/version·transform capability 없음 | lower assurance 자동 fallback 금지 |
| `RECIPE_ASSURANCE_UNSATISFIED` | syntax·semantic·generator coverage/postcondition을 증명 못함 | auto apply 금지, review/block |
| `RECIPE_IDEMPOTENCE_FAILED` | expected-after replay가 다시 operation을 만들거나 확인 불가 | PatchSet automatic apply 금지 |
| `PATCH_PREVIEW_INCOMPLETE` | preview ChangeSet·diff·output·redaction이 partial/unverified | immutable apply candidate publish 금지 |
| `PATCH_SCOPE_VIOLATION` | Recipe·accepted planned scope 밖 file/symbol/contract/generated output | candidate 폐기, replan 또는 Recipe 수정 |
| `PATCH_DIRTY_OVERLAP` | preexisting change와 path/range/rename/generated owner overlap 또는 unknown | current checkout apply 금지, isolated/block |
| `PATCH_REPLAN_REQUIRED` | preview가 새 path·change class·risk·Profile·Check/fallback을 요구 | candidate invalidated, M2 후 새 prepare |
| `PATCH_PARTIAL_APPLY` | operation 일부만 receipt 완료 또는 actual manifest 부분/superset | 성공 금지, source reconcile·recovery |
| `PATCH_OUTCOME_UNKNOWN` | crash·I/O 유실로 operation effect 여부 불명 | 자동 retry 금지, actual hash probe |
| `PATCH_RECOVERY_BLOCKED` | reverse/discard precondition·ownership·permission 불충족 | byte 보존, manual recovery ReviewPack |

기존 `PATCH_PRECONDITION_FAILED`, `PATCH_POSTCONDITION_FAILED`는 M4에서도 그대로 사용한다. external process의 start·timeout·output·protocol·outcome failure는 `TOOL_PROCESS_START_FAILED`, `TOOL_TIMEOUT`, `TOOL_OUTPUT_LIMIT`, `TOOL_PROTOCOL_INVALID`, `TOOL_OUTCOME_UNKNOWN`을 유지하며 Recipe success code로 다시 포장하지 않는다.

### 관리형 Symbol Registry 대표 오류

5단계 Managed Registry의 stable error는 source manifest 또는 command 자체를 안전하게 처리할 수 없는 이유를 나타낸다. source 안에서 발견한 binding·consumer·문서 drift는 command error로 숨기지 않고 Diagnostic Rule family로 보고한다. exact lifecycle과 field는 [관리형 Symbol·상수·에러 코드 Registry 계약](managed-symbol-registry.md)이 소유한다.

| code | 발생 조건 | 기본 행동 |
|---|---|---|
| `REGISTRY_MANIFEST_INVALID` | root·fragment Schema, explicit reference 또는 namespace claim이 유효하지 않음 | invalid source를 current snapshot으로 publish하지 않음 |
| `REGISTRY_SNAPSHOT_STALE` | Git manifest·source binding과 derived DB Index fingerprint가 다름 | Git을 우선하고 새 scan 요구 |
| `REGISTRY_DECLARATION_CONFLICT` | 같은 stable declaration/version의 정의가 다르거나 owner·type이 충돌 | 우선순위 덮기 없이 차단 |
| `REGISTRY_DUPLICATE_ID` | 같은 stable ID·public uniqueness scope가 중복됨 | Patch 준비·적용 차단 |
| `REGISTRY_NAMESPACE_COLLISION` | namespace claim이 겹치거나 위임 밖 선언이 존재함 | owner 결정 전 차단 |
| `REGISTRY_ID_REUSE_FORBIDDEN` | removed·reserved tombstone의 ID 또는 public value 재사용 시도 | 영구 차단, 새 ID 요구 |
| `REGISTRY_ALIAS_INVALID` | alias cycle, 무기한 window, 이미 제거된 target 등 lifecycle 위반 | 호환 가능으로 판정하지 않음 |
| `REGISTRY_BINDING_UNRESOLVED` | 필수 definition·Schema·문서·generated binding을 찾거나 검증할 수 없음 | snapshot partial/invalid, 자동 통과 금지 |
| `REGISTRY_CHANGE_STALE` | ChangePlan·PatchSet 뒤 manifest·Index·consumer 관찰이 바뀜 | M2 재계획과 새 dry-run 요구 |
| `REGISTRY_CROSS_PROJECT_APPLY_UNSUPPORTED` | 9단계 전 여러 Project source 적용을 요청함 | read-only 영향만 반환하고 write 거부 |

대표 Diagnostic Rule ID는 `star.validation.registry.binding-drift`, `star.validation.registry.consumer-not-migrated`, `star.validation.registry.deprecated-reference`, `star.validation.registry.removed-reference`, `star.validation.registry.alias-window-expired`, `star.validation.registry.generated-output-stale`, `star.validation.registry.generated-direct-edit`, `star.validation.registry.docs-schema-drift`다.

### 6단계 계약·문서·환경 대표 오류

[6단계 계약 호환성·환경 정본](contract-compatibility-and-environment.md)의 command·preflight를 안전하게 수행할 수 없는 경우만 ErrorEnvelope를 사용한다. baseline/current 차이, breaking change, 문서 drift, config key 미사용, toolchain 누락과 Windows 차이는 정상적으로 생산된 Diagnostic이지 command error가 아니다.

아래 code와 Rule ID는 M6 목표 계약이며 현재 제품·Registry·Catalog에 구현됐다는 뜻이 아니다. 구현 change에서 M5 ManagedDeclaration·namespace/lifecycle과 Catalog Rule·mapping을 먼저 등록하고 Schema·fixture·consumer를 같은 Gate로 검증한다.

| code | 발생 조건 | 기본 행동 |
|---|---|---|
| `CONTRACT_MANIFEST_NOT_FOUND` | required Profile인데 `.star-control/contracts.toml`이 없음 | 암묵 manifest/baseline 생성 금지, plan block |
| `CONTRACT_BASELINE_INVALID` | baseline이 mutable·미승인·hash 불일치 또는 current 자동 채택 | 비교 시작 전 거부 |
| `CONTRACT_SNAPSHOT_STALE` | source·Registry·config·Catalog·Tool·environment fingerprint가 snapshot과 다름 | snapshot 재생성·M2 replan |
| `CONTRACT_COMPARISON_INCOMPLETE` | required surface를 parse/normalize할 수 없거나 comparison invariant 위반 | report를 pass로 publish하지 않음 |
| `DOCUMENTATION_SNAPSHOT_INVALID` | docs entry·policy·location·generated provenance 계약을 정규화할 수 없음 | invalid entry를 누락하지 않고 block |
| `DOCTOR_PROBE_UNREGISTERED` | exact read-only ToolDescriptor 없는 probe를 요청 | 실행하지 않고 `not_run` |
| `DOCTOR_PROBE_SIDE_EFFECT_FORBIDDEN` | probe가 network·package/source/system mutation을 선언·관찰 | 실행 전 거부, 이미 시작됐다면 outcome 보존·block |
| `DOCTOR_INPUT_CHANGED` | 진단 중 manifest·lockfile·config·Catalog·environment binding 변경 | report publish 금지, 새 snapshot 요구 |
| `CLEAN_ROOM_SPEC_INCOMPLETE` | source/toolchain/lockfile/command/network/cache/path constraint 필수값 누락 | environment 생성·install 없이 `not_ready` |
| `CLEAN_ROOM_COMMAND_UNREGISTERED` | 명세 command가 registered Check/Tool descriptor와 결합되지 않음 | raw shell 실행 금지, `not_ready` |

6단계 required Diagnostic namespace는 다음과 같다.

- compatibility: `star.validation.contract.baseline-missing`, `star.validation.contract.breaking-change`, `star.validation.contract.unknown-change`, `star.validation.contract.consumer-unverified`, `star.validation.contract.migration-guide-missing`, `star.validation.contract.companion-change-missing`, `star.validation.contract.deprecation-window-invalid`, `star.validation.contract.public-surface-expanded`
- docs: `star.validation.docs.broken-link`, `star.validation.docs.broken-anchor`, `star.validation.docs.command-unregistered`, `star.validation.docs.command-signature-drift`, `star.validation.docs.command-unsafe`, `star.validation.docs.snippet-invalid`, `star.validation.docs.snippet-unverified`, `star.validation.docs.config-example-invalid`, `star.validation.docs.schema-drift`, `star.validation.docs.generated-reference-drift`, `star.validation.docs.assumption-drift`
- config: `star.validation.config.key-undocumented`, `star.validation.config.key-unused`, `star.validation.config.override-untracked`, `star.validation.config.deprecated-key-used`, `star.validation.config.removed-key-used`, `star.validation.config.environment-variable-undocumented`
- environment: `star.validation.environment.toolchain-missing`, `star.validation.environment.tool-version-mismatch`, `star.validation.environment.package-manager-mismatch`, `star.validation.environment.lockfile-drift`, `star.validation.environment.command-unavailable`, `star.validation.environment.path-case-collision`, `star.validation.environment.encoding-mismatch`, `star.validation.environment.line-ending-mismatch`, `star.validation.environment.path-length-risk`, `star.validation.environment.fingerprint-drift`, `star.validation.environment.clean-room-unverified`, `star.validation.environment.mutation-required`

`mutation-required`는 자동 조치가 아니라 current environment가 선언된 조건을 만족하려면 사용자 설치·설정이 필요하다는 관찰이다. remediation은 `safe_auto_fix=false`이고 값·credential·raw absolute path를 포함하지 않는다.

### 7단계 실패·보안·dependency 대표 오류

[7단계 정본](failure-security-and-dependency-maintenance.md)의 preflight·adapter invocation·document publish를 안전하게 수행할 수 없는 경우만 ErrorEnvelope를 사용한다. 재현된 failure, secret 후보, vulnerable/outdated dependency, mutable workflow action과 flaky test는 정상적으로 생산된 Diagnostic·Finding이다.

아래 code와 Rule ID는 M7 목표 계약이며 현재 제품·Registry·Catalog에 구현됐다는 뜻이 아니다.

| code | 발생 조건 | 기본 행동 |
|---|---|---|
| `MAINTENANCE_STORE_INCOMPATIBLE` | 공통 Finding·Evidence·Suppression 계약 version을 M7이 안전하게 재사용할 수 없음 | 별도 DB 생성 금지, compatibility/migration block |
| `MAINTENANCE_RADAR_INPUT_STALE` | Radar input revision·evaluation time·valid_until이 current가 아님 | 기존 순위를 current로 표시하지 않고 재계산 |
| `REPRODUCTION_INPUT_INCOMPLETE` | revision·structured args·environment·input/seed·expected/actual required field 누락 | pack publish·fixed claim 금지 |
| `REPRODUCTION_SUBJECT_CHANGED` | attempt 중 source·manifest·lockfile·tool/environment binding 변경 | occurrence 분리, 새 plan 요구 |
| `REPRODUCTION_EXTERNAL_CONDITION_UNVERIFIED` | service·device·clock·network 조건을 확인/재현할 수 없음 | `blocked_external\|unverified` 보존 |
| `RECOVERY_PLAN_INVALID` | rollback·roll-forward·restore가 섞였거나 precondition·stop·validation 누락 | 실행 permit 금지 |
| `SECURITY_DATA_STALE` | required external source의 valid_until 경과 | clean/pass 금지, 승인된 refresh 제안 |
| `SECURITY_DATA_SOURCE_UNVERIFIED` | source/query/schema/tool/coverage provenance 누락 | vulnerability/license/current status를 unknown으로 유지 |
| `SECURITY_REDACTION_FAILED` | artifact를 default report에 안전하게 넣을 수 없음 | quarantine/drop, report block |
| `DEPENDENCY_INPUT_STALE` | Project/Index 또는 manifest·lockfile·M6 input이 current subject와 다름 | M1/M6 재수집 |
| `DEPENDENCY_MANAGER_UNREGISTERED` | manifest·lockfile owner의 trusted adapter가 없음 | 직접 편집 금지, human review |
| `DEPENDENCY_NETWORK_APPROVAL_REQUIRED` | refresh/download/resolve의 exact 승인 없음 | side effect 없이 승인 대기 |
| `DEPENDENCY_CHANGE_APPROVAL_REQUIRED` | add/update/remove/lockfile change 승인 없음 | preview/apply 시작 금지 |
| `DEPENDENCY_LOCKFILE_OWNER_UNVERIFIED` | lockfile이 manager 결과인지 증명할 수 없거나 core/text edit 관찰 | PatchSet·Gate block |
| `DEPENDENCY_UPDATE_REPLAN_REQUIRED` | isolated preview actual diff가 계획 scope·impact·Check를 바꿈 | 기존 PatchSet 폐기, M2 재계획 |
| `DEPENDENCY_PATCH_APPROVAL_REQUIRED` | immutable PatchSet apply 승인 없음 | `awaiting_apply_approval` |
| `DEPENDENCY_ROLLBACK_BLOCKED` | previous manifest·lockfile 또는 rollback validation 근거 부족 | apply 금지·복구 계획 보완 |

7단계 required Diagnostic namespace는 다음과 같다.

- failure: `star.validation.failure.reproduction-unverified`, `star.validation.failure.identity-changed`, `star.validation.failure.after-evidence-incompatible`, `star.validation.failure.after-flaky`, `star.validation.failure.recovery-plan-unverified`, `star.validation.failure.sensitive-artifact-unsafe`
- security: `star.validation.security.secret-candidate`, `star.validation.security.redaction-failed`, `star.validation.security.dangerous-command-candidate`, `star.validation.security.dangerous-command-executable`, `star.validation.security.workflow-permission-widened`, `star.validation.security.external-action-mutable-ref`, `star.validation.security.external-database-stale`, `star.validation.security.external-scan-unverified`, `star.validation.security.release-manifest-incomplete`
- dependency: `star.validation.dependency.input-stale`, `star.validation.dependency.status-unknown`, `star.validation.dependency.lockfile-owner-unverified`, `star.validation.dependency.unapproved-network-effect`, `star.validation.dependency.unapproved-change`, `star.validation.dependency.patch-replan-required`, `star.validation.dependency.rollback-unverified`
- radar: `star.validation.maintenance.radar-input-stale`, `star.validation.maintenance.suppression-expired`, `star.validation.maintenance.evidence-expired`

### 8단계 migration·performance·language/platform 대표 오류

[8단계 정본](migration-performance-and-platform.md)의 상태 전이와 Gate를 안전하게 계산할 수 없는 경우 아래 ErrorEnvelope code를 사용한다. migration 실패·부분 성공, 성능 비교 불가와 기능 동등성 미확인은 성공을 꾸며내지 않고 각각 typed result와 Diagnostic으로도 보존한다. 아래 code는 M8 목표 계약이며 현재 제품에 구현됐다는 뜻이 아니다.

| code | 발생 조건 | 기본 행동 |
|---|---|---|
| `MIGRATION_VERSION_UNKNOWN` | current/target version을 신뢰할 수 있게 식별하지 못함 | chain 선택·execute 금지 |
| `MIGRATION_CHAIN_GAP` | current에서 target까지 연속 step이 없음 | 지원되는 중간 version 또는 새 step 요구 |
| `MIGRATION_CHAIN_AMBIGUOUS` | 같은 입력에 둘 이상의 자동 선택 가능 path가 있음 | 자동 선택 금지, plan 수정 |
| `MIGRATION_UNKNOWN_FIELD_UNPRESERVED` | unknown/extension field의 round-trip 보존을 증명하지 못함 | destructive 취급, 기본 block |
| `MIGRATION_BACKUP_UNVERIFIED` | backup 존재만 확인했거나 integrity/restore evidence가 부족함 | restore 가능 주장 금지, required rehearsal 수행 |
| `MIGRATION_REHEARSAL_REQUIRED` | 정책상 필요한 restore/migration rehearsal이 없거나 live plan과 다름 | pre-execute Gate block |
| `MIGRATION_APPROVAL_REQUIRED` | live/destructive effect의 exact single-use 승인이 없음 | side effect 없이 승인 대기 |
| `MIGRATION_PRECONDITION_STALE` | subject·version·step/tool·config·environment binding이 plan 이후 변경됨 | 기존 승인 폐기, 재계획 |
| `MIGRATION_OUTCOME_UNKNOWN` | timeout/crash 뒤 durable commit 여부를 판정하지 못함 | 새 실행 금지, reconcile 요구 |
| `MIGRATION_PARTIAL` | 일부 durable step만 완료되고 target version·post Gate에 도달하지 못함 | 성공 금지, resume/rollback decision 요구 |
| `MIGRATION_INVARIANT_FAILED` | before/after invariant 또는 post Gate 불충족 | activation 금지 또는 rollback 판단 |
| `MIGRATION_ROLLBACK_FAILED` | rollback/restore 또는 post-rollback 검증 실패 | `rollback_failed`, human recovery |
| `PERFORMANCE_WORKLOAD_NOT_DECLARED` | 사용자가 중요 경로와 workload를 활성화하지 않음 | 측정하지 않고 `not_requested` |
| `PERFORMANCE_MEASUREMENT_UNAVAILABLE` | numeric value·unit·collector·raw sample이 없음 | 수치·개선 주장 금지 |
| `PERFORMANCE_NOT_COMPARABLE` | workload/input/tool/environment/mode/revision cohort가 다름 | 승패 계산 금지, 새 동등 cohort 요구 |
| `PERFORMANCE_NOISE_INCONCLUSIVE` | 사전 선언한 noise/outlier 처리 뒤 유효 표본이 부족하거나 interval이 겹침 | `inconclusive`, 임의 outlier 제거 금지 |
| `PERFORMANCE_CORRECTNESS_UNVERIFIED` | candidate correctness Gate가 완료되지 않음 | 성능상 이점으로 cutover 정당화 금지 |
| `LANGUAGE_BEHAVIOR_BASELINE_MISSING` | 현재 동작 계약·consumer 관찰점이 없음 | 번역/cutover 계획 block |
| `LANGUAGE_EQUIVALENCE_INCOMPLETE` | required equivalence dimension이 partial/not_run/unverified | compile 성공과 분리해 Gate block |
| `LANGUAGE_SEMANTICS_HUMAN_REVIEW` | 자동 변환으로 의미를 확정할 수 없음 | `HUMAN_REVIEW`, 자동 완료 금지 |
| `PLATFORM_RUNTIME_UNVERIFIED` | 지원한다고 주장할 OS/arch의 실제 runtime evidence가 없음 | 해당 플랫폼 pass 주장 금지 |
| `LANGUAGE_CUTOVER_NOT_READY` | source·consumer 순서, compatibility window, rollback 또는 Gate가 미충족 | writer/canonical source cutover 금지 |
| `CROSS_PROJECT_MIGRATION_DEFERRED` | 하나의 migration이 둘 이상의 Project에 write해야 함 | read-only handoff만 만들고 9단계 ChangeBundle로 이관 |

8단계 required Diagnostic namespace는 다음과 같다.

- migration: `star.validation.migration.version-unknown`, `star.validation.migration.chain-gap`, `star.validation.migration.backup-unverified`, `star.validation.migration.restore-unverified`, `star.validation.migration.partial`, `star.validation.migration.invariant-failed`, `star.validation.migration.rollback-failed`
- performance: `star.validation.performance.workload-undeclared`, `star.validation.performance.measurement-unavailable`, `star.validation.performance.cohort-mismatch`, `star.validation.performance.noise-inconclusive`, `star.validation.performance.correctness-unverified`, `star.validation.performance.maintainability-regressed`
- language/platform: `star.validation.language.baseline-missing`, `star.validation.language.equivalence-incomplete`, `star.validation.language.semantic-review-required`, `star.validation.language.consumer-unverified`, `star.validation.language.compatibility-window-invalid`, `star.validation.platform.runtime-unverified`

### 9단계 CrossRepo ChangeBundle 대표 오류

[9단계 정본](cross-repo-change-bundle.md)의 Project relation, participant, worktree, merge queue와 remote operation을 안전하게 계산하거나 수행할 수 없을 때 아래 ErrorEnvelope code를 사용한다. project별 실패·부분 성공·검증 미완료는 ErrorEnvelope만으로 축약하지 않고 해당 participant state, `ProjectMergeResult`, `GateDecision`과 Diagnostic에도 보존한다. 아래 code는 9단계 목표 계약이며 현재 제품에 구현됐다는 뜻이 아니다.

| code | 발생 조건 | 기본 행동 |
|---|---|---|
| `CHANGE_BUNDLE_RELATION_UNVERIFIED` | required provider·consumer relation이 stale·partial·unknown임 | 자동 순서·apply 금지, current Catalog/graph probe 또는 human review |
| `CHANGE_BUNDLE_DEPENDENCY_CYCLE` | required BundleStepGraph가 cycle임 | bundle prepare Gate block, compatibility/cutover plan 수정 |
| `CHANGE_BUNDLE_PARTICIPANT_STALE` | base·dirty manifest·PatchSet·contract·Gate binding 중 하나가 변경됨 | 해당 participant와 downstream을 `held`, `pending_action=reprepare`로 전이 |
| `CHANGE_BUNDLE_PARTIAL` | 적어도 한 project effect는 확인됐지만 required graph가 완료되지 않음 | 전체 성공 금지, resume·compensate·hold 결정 요구 |
| `CHANGE_BUNDLE_OUTCOME_UNKNOWN` | process 중단 뒤 project-local durable effect를 판정하지 못함 | 새 apply·merge 금지, actual state reconcile |
| `CHANGE_BUNDLE_ROLLBACK_REQUIRED` | project Gate 또는 compatibility invariant 실패로 보상이 필요함 | 성공 projection 금지, project별 compensation plan·승인 요구 |
| `WORKTREE_USER_CHANGE_CONFLICT` | 사용자 기존 staged·unstaged·untracked byte와 계획 변경의 안전한 분리를 증명하지 못함 | primary checkout을 건드리지 않고 별도 owned worktree 또는 block |
| `WORKTREE_OWNERSHIP_MISMATCH` | root binding·Git registration·owner token·manifest가 일치하지 않음 | apply·cleanup·재사용 금지, 보존 후 human recovery |
| `CHANGE_BUNDLE_OVERLAP_UNKNOWN` | file·range·symbol·contract·generated·lockfile overlap 검사가 incomplete임 | 병렬 실행·merge 준비 금지 |
| `CHANGE_BUNDLE_RESOURCE_LIMIT` | project/process/worktree/disk/merge queue 한도를 예약할 수 없음 | effect 시작 전 대기·보류, 한도를 자동 확대하지 않음 |
| `MERGE_QUEUE_ORDER_BLOCKED` | provider/consumer dependency 또는 compatibility window가 queue head와 불일치 | dequeue·local merge 금지, predecessor 완료 대기 |
| `MERGE_TARGET_STALE` | target tip·queue predecessor·merge-base가 계획 뒤 변경됨 | 기존 MergePlan 폐기, current base에서 재계획 |
| `MERGE_CONFLICT_REVIEW_REQUIRED` | 충돌 양쪽 PatchSet intent·관련 contract·base를 완전하게 제시하지 못하거나 기계적으로 유일한 해가 없음 | 자동 해소 금지, typed conflict와 human review |
| `PROJECT_POST_MERGE_GATE_FAILED` | local merge 뒤 project required Gate가 fail·partial·unverified임 | 해당 project 완료 금지, downstream·전체 Goal Gate block |
| `REMOTE_SNAPSHOT_STALE` | adapter snapshot이 expired·partial이거나 ref/PR/check head가 변경됨 | remote effect 금지, adapter refresh |
| `REMOTE_APPROVAL_REQUIRED` | exact push·PR create/update·merge·publish action의 single-use 승인이 없음 | remote API 호출 없이 `awaiting_approval` |
| `REMOTE_RESULT_UNVERIFIED` | timeout/crash 또는 partial after-snapshot 때문에 remote side effect 성공 여부를 확인할 수 없음 | 같은 operation 재시도 금지, `outcome_unknown`과 idempotency key로 reconcile |
| `RELEASE_HANDOFF_INCOMPLETE` | participant·source revision·artifact·Gate·compatibility 또는 remote merge binding이 incomplete/stale임 | 10단계 handoff를 release-ready로 발행하지 않음 |

9단계 required Diagnostic namespace는 다음과 같다.

- bundle: `star.validation.change-bundle.relation-unverified`, `star.validation.change-bundle.dependency-cycle`, `star.validation.change-bundle.participant-stale`, `star.validation.change-bundle.partial`, `star.validation.change-bundle.outcome-unknown`, `star.validation.change-bundle.rollback-required`, `star.validation.change-bundle.resource-limit`
- worktree/overlap: `star.validation.worktree.user-change-conflict`, `star.validation.worktree.ownership-mismatch`, `star.validation.change-bundle.overlap-unknown`
- merge: `star.validation.merge.queue-order-blocked`, `star.validation.merge.target-stale`, `star.validation.merge.conflict-review-required`, `star.validation.merge.post-gate-failed`
- remote/release: `star.validation.remote.snapshot-stale`, `star.validation.remote.approval-required`, `star.validation.remote.result-unverified`, `star.validation.release.handoff-incomplete`

### 10단계 CI·Release·평가 대표 오류

[10단계 CI·Release·평가·최종 제품 완성](ci-release-evaluation-and-product-completion.md)의 planning, Gate, promotion, evaluation과 lifecycle failure는 다음 stable code를 사용한다. 외부 build·installer·signer·CI·registry·deploy tool의 raw code는 adapter detail에 보존하되 이 code로 정규화한다.

| code | 조건 | 기본 행동 |
|---|---|---|
| `RELEASE_SUBJECT_STALE` | Task·source revision·dirty/config/Catalog/Tool identity가 plan과 다름 | 새 candidate·ValidationPlan 요구 |
| `RELEASE_PROFILE_MISMATCH` | resolved final 16 Profile closure·fingerprint가 계층 사이에 다름 | release Gate `block` |
| `RELEASE_CLEAN_ENVIRONMENT_REQUIRED` | release Check가 clean declared Windows environment가 아님 | evidence 제외·`block` |
| `RELEASE_ARTIFACT_SUBJECT_MISMATCH` | artifact source/build invocation이 release subject와 다름 | candidate 폐기·rebuild plan |
| `RELEASE_ARTIFACT_DIGEST_MISMATCH` | artifact/file-list/set digest 재계산 불일치 | quarantine·`block` |
| `RELEASE_REBUILD_FORBIDDEN` | 승격·검증·publish 단계가 final artifact를 다시 build/package하려 함 | 실행 전 거부 |
| `RELEASE_PACKAGE_CONTENT_INVALID` | dry-run/file list에 누락·unexpected·forbidden payload가 있음 | `block` |
| `RELEASE_METADATA_INCOMPLETE` | version·changelog·metadata·license·applicability 근거 누락 | `block` |
| `RELEASE_PLATFORM_EVIDENCE_MISSING` | required x64 Stable native evidence 또는 ARM64 Preview cross-build·simulation evidence 누락 | `blocked_external` 또는 `block` |
| `RELEASE_INSTALL_LIFECYCLE_FAILED` | install·safe_default·update·rollback·uninstall의 required Check 실패 | `rollback_required` 또는 `block` |
| `RELEASE_APPROVAL_REQUIRED` | publish·deploy·withdrawal·rollback exact action 승인 없음 | effect 없이 대기 |
| `RELEASE_APPROVAL_STALE` | manifest/digest/destination/before snapshot/expiry가 승인과 다름 | 새 snapshot·승인 요구 |
| `RELEASE_REMOTE_RESULT_UNVERIFIED` | receipt 뒤 after snapshot이 version·source/tag·digest를 확인하지 못함 | action `outcome_unknown`; publication이면 top-level `publish_outcome_unknown`, published 금지 |
| `RELEASE_ROLLBACK_REQUIRED` | 배포 관찰이 rollback trigger를 충족 | 사용자 데이터 보존 rollback 계획·승인 요구 |
| `EVALUATION_NOT_COMPARABLE` | case·source·config·Catalog·Tool·environment·protocol 차이를 통제 못함 | recommendation `needs_review` |
| `EVALUATION_GROUND_TRUTH_INCOMPLETE` | actual defect·false positive 판정 또는 denominator가 불완전 | accept/reject 자동 판정 금지 |
| `EVALUATION_VALIDATOR_WEAKENING` | required Rule·Check·severity·ratchet·Corpus·freshness를 낮춤 | candidate `reject`·Gate `block` |
| `EVALUATION_POLICY_CHANGED` | 실행 중 threshold·sample·retry·adjudication policy가 바뀜 | run 무효·새 EvaluationRun |
| `CATALOG_LIFECYCLE_MIGRATION_REQUIRED` | deprecated/retired descriptor의 replacement·consumer migration 근거 누락 | default selection·retirement 차단 |

Diagnostic namespace는 release에 `star.validation.release.subject-stale`, `artifact-digest-mismatch`, `package-content-invalid`, `platform-evidence-missing`, `install-lifecycle-failed`, `remote-result-unverified`를 사용하고 evaluation에 `star.validation.evaluation.not-comparable`, `ground-truth-incomplete`, `validator-weakening`, `policy-changed`를 사용한다. provider·architecture·channel 이름을 Rule ID로 만들지 않고 typed parameter로 기록한다.

### 11단계 Rust style 대표 reason code

[Rust 코드 스타일 자동 교정](../features/rust-code-style-auto-fix.md)의 discovery·candidate·auto-apply는 다음 stable code를 사용한다. 이 code는 M11 목표 계약이며 현재 Rust enum·Schema·제품 code에 구현됐다는 뜻이 아니다.

| code | 조건 | 표현·공통 mapping | 기본 행동 |
|---|---|---|---|
| `RUST_TOOLCHAIN_UNRESOLVED` | project-pinned stable toolchain 또는 cargo/rustc/rustfmt/clippy identity를 완전히 resolve하지 못함 | inspect limitation Diagnostic; prepare 불가면 input/readiness Error | check 결과에 limitation, auto apply 금지 |
| `RUST_COMPONENT_UNAVAILABLE` | rustfmt/Clippy/required target component가 없음 | root cause가 executable 없음이면 `TOOL_EXECUTABLE_NOT_FOUND`; coverage cell Diagnostic에 Rust code | 설치·download 없이 unavailable, auto apply 금지 |
| `RUST_STYLE_CONFIG_AMBIGUOUS` | 같은 precedence config 후보 또는 workspace/config boundary를 단일하게 정하지 못함 | config/discovery Error + Diagnostic | 임의 선택 없이 candidate `block` |
| `RUST_STYLE_UNSTABLE_OPTION_UNSUPPORTED` | nightly 또는 unstable rustfmt option/style contract가 필요함 | unsupported input Error + Diagnostic | option 무시 금지, auto candidate `block` |
| `RUST_STYLE_COVERAGE_INCOMPLETE` | required package/target/feature/triple/cfg/ownership cell이 skipped/unavailable/conflicted/unverified | Gate Diagnostic; 실행 원인은 관련 generic Tool/Validation error ref | `AUTO_PASS` 금지 |
| `RUST_CLIPPY_FIX_NOT_ALLOWED` | exact lint ID가 allowlist에 없거나 Clippy version/scope constraint 불일치 | informational/review Diagnostic, command Error 아님 | suggestion skip, source 불변 |
| `RUST_CLIPPY_SUGGESTION_NOT_MACHINE_APPLICABLE` | applicability가 `MaybeIncorrect|HasPlaceholders|Unspecified|unknown` | informational/review Diagnostic, command Error 아님 | suggestion skip, 자동 승격 금지 |
| `RUST_STYLE_SIDE_EFFECT_VIOLATION` | non-`.rs`, generated/vendor/out-of-scope/public/config/lockfile write 또는 unmatched hunk | M11 Diagnostic + M4 `PATCH_SCOPE_VIOLATION` 또는 validation undeclared effect relation | candidate 전체 폐기 |
| `RUST_STYLE_NON_IDEMPOTENT` | full-pipeline replay에서 mutation/impact delta가 남음 | M11 Diagnostic; M4 idempotence failure relation | PatchSet publish·auto apply 금지 |
| `RUST_STYLE_AUTO_SCOPE_MISMATCH` | exact candidate가 `personal_auto` standing grant의 Project/Profile/pipeline/policy/scope/action/diff ceiling 밖 | approval/policy Diagnostic; 승인 대기면 exit 3 | automatic ApprovalDecision·permit 없음 |

한 failure에 generic execution code와 Rust reason이 함께 존재할 수 있다. primary error는 가장 구체적인 안전한 command failure를 사용하고 Rust code는 coverage/Gate relation의 reason ref로 연결한다. 예를 들어 Clippy timeout의 primary ErrorEnvelope는 `TOOL_TIMEOUT`이고 coverage cell은 `RUST_STYLE_COVERAGE_INCOMPLETE`를 가진다. timeout을 Rust code로 다시 포장해 attempt 원인을 잃지 않는다.

기존 공통 mapping은 다음처럼 재사용한다.

- process start/nonzero/timeout/output/protocol/outcome unknown: `TOOL_PROCESS_START_FAILED|TOOL_TIMEOUT|TOOL_OUTPUT_LIMIT|TOOL_PROTOCOL_INVALID|TOOL_OUTCOME_UNKNOWN`
- ToolDescriptor/executable drift: `TOOL_DESCRIPTOR_STALE` 또는 `VALIDATION_TOOL_STALE`
- source/config/Catalog/policy drift: `VALIDATION_SUBJECT_CHANGED|VALIDATION_CATALOG_STALE|VALIDATION_EVIDENCE_STALE`
- dirty overlap, Patch before/after, partial/recovery: `PATCH_DIRTY_OVERLAP|PATCH_PRECONDITION_FAILED|PATCH_POSTCONDITION_FAILED|PATCH_PARTIAL_APPLY|PATCH_RECOVERY_BLOCKED`
- required candidate/post Check 실패·incomplete: 공통 ValidationRun·Gate Diagnostic과 `VALIDATION_EVIDENCE_INCOMPLETE`

M11 Diagnostic Rule ID는 `star.validation.rust-style.toolchain-unresolved`, `component-unavailable`, `config-ambiguous`, `unstable-option-unsupported`, `coverage-incomplete`, `clippy-fix-not-allowed`, `clippy-suggestion-not-machine-applicable`, `side-effect-violation`, `non-idempotent`, `auto-scope-mismatch`로 고정한다. exact lint ID·package·feature·target는 typed parameter이고 Rule ID suffix로 만들지 않는다. allowlist 밖/비적용 suggestion을 skip한 사실은 보존하되 기존 project lint level이 허용한 Diagnostic을 M11이 임의 blocking error로 승격하지 않는다.

### 공통 검증·품질 Gate 대표 오류

[3단계 공통 검증·품질 Gate](../features/common-validation-gate.md)의 command·preflight failure는 다음 stable code로 정규화한다. test failure·architecture 위반·secret 후보 같은 프로젝트 문제는 ErrorEnvelope가 아니라 Diagnostic이다.

| code | 조건 | 기본 행동 |
|---|---|---|
| `VALIDATION_PLAN_INCOHERENT` | TaskSpec·ScopeRevision·ImpactAnalysis·ChangeSet·ChangePlan·ValidationPlan ref/fingerprint 불일치 | process 시작 전 `block` |
| `VALIDATION_PROFILE_CLOSURE_STALE` | actual change class, Profile activation evidence, parent closure 또는 materialize된 required family가 plan fingerprint와 다름 | process 시작 전 중지·M2 재계획 |
| `VALIDATION_SUBJECT_CHANGED` | current ProjectRevision·WorkspaceSnapshot·actual ChangeSet이 plan subject와 다름 | M2 `replan_required` |
| `VALIDATION_CATALOG_STALE` | Rule·Check·GatePolicy·Validator Registry fingerprint 변경 | 실행 중단, plan refresh |
| `VALIDATION_TOOL_STALE` | ToolDescriptor·executable identity·version·hash 변경 | trust/describe 재확인 뒤 replan |
| `VALIDATION_REQUIRED_CHECK_UNRESOLVED` | required Check의 descriptor·tool·typed invocation을 실행 시점에 해석 못함 | `not_run`, `block` |
| `VALIDATION_CHECK_GRAPH_INVALID` | cycle, missing node, required dependency closure 또는 failure policy 불완전 | 실행 전 `block` |
| `VALIDATION_PERMISSION_BLOCKED` | required Check 실행에 필요한 permission·paid action 승인이 없음 | side effect 없이 `human_review` 또는 deny `block` |
| `VALIDATION_DIAGNOSTIC_MAPPING_FAILED` | 외부 결과 Schema·code·location을 공통 Diagnostic으로 완전히 변환 못함 | completeness 하향, `auto_pass` 금지 |
| `VALIDATION_UNDECLARED_SIDE_EFFECT` | CheckDescriptor에 없는 source·Git·external effect 관찰 | 실행 중단, outcome unknown이면 자동 retry 금지 |
| `VALIDATION_EVIDENCE_STALE` | evidence subject binding이 source·plan·config·Catalog·Tool·execution environment current probe와 다름 | positive evidence에서 제외 |
| `VALIDATION_EVIDENCE_INCOMPLETE` | required output·scope가 partial·unverified 또는 잘림 | pass 금지, review/block |
| `VALIDATION_TIME_UNVERIFIED` | expiry/freshness boundary가 있는데 evaluation clock을 검증하지 못함 | `auto_pass` 금지, review/block |
| `VALIDATION_GATE_EXPIRED` | current time이 GateDecision `valid_until` 이상 | 기존 decision 재사용 금지, 새 probe·Gate |
| `VALIDATION_GATE_INPUT_INVALID` | Baseline·Suppression·Claim·DiagnosticEvaluation reference/invariant 오류 | GateDecision publish 금지 |

process를 시작하지 못한 required Check는 위 ErrorEnvelope와 `ValidationRun.outcome=not_run`을 함께 만들 수 있다. Error code가 있다고 해당 Check를 `not_applicable`로 바꾸지 않는다.

## Diagnostic 계약

P0/v1의 bare `rule_id`와 optional suppression projection은 읽기 compatibility를 위해 보존한다. M3 target `star.diagnostic` v2는 producer가 관찰 사실만 만들도록 typed `rule_ref`, producer provenance와 fingerprint contract를 필수화하고 baseline·suppression·gate effect는 [GateDecision의 DiagnosticEvaluation](validation-and-evidence.md#diagnosticevaluation)로 분리한다. 이 target은 현재 Schema·migration·제품 code에 구현됐다는 뜻이 아니다.

| 필드 | 형식 | 의미 |
|---|---|---|
| `diagnostic_id` | DiagnosticId | 한 발견 instance |
| `sequence`, `observed_at` | non-negative integer, UTC | append-only 관찰 순서와 시각 |
| `rule_id` | stable Catalog ID | v1 발견 규칙 ID |
| `rule_ref` | RuleRef | M3 필수 Rule ID·version·definition fingerprint·fingerprint contract version |
| `producer` | ProducerRef | built-in validator 또는 external Tool·Check·normalizer identity |
| `title` | string | 짧은 문제 이름 |
| `message` | string | 영향과 관찰 사실 |
| `severity` | enum | `info`, `warning`, `error`, `critical` |
| `confidence` | enum | `low`, `medium`, `high` |
| `status` | enum | v1 read compatibility의 `confirmed\|suspected\|unverified\|suppressed\|resolved` |
| `observation_status` | enum | M3 writer 필수 `confirmed\|suspected\|unverified`. suppression·resolved는 관찰 사실이 아님 |
| `scope` | object | Goal·Run·Stage·ValidationRun revision |
| `locations` | LocationRef array | 파일·line·symbol 등 |
| `evidence_refs` | EvidenceRef array | Location·ChangeSet·ValidationRun·Finding·Artifact 등 typed 판단 근거 |
| `subject_binding_fingerprint` | optional SHA-256 | M3 Validation evidence subject identity |
| `fingerprint` | SHA-256 | 같은 문제 중복 묶기 |
| `remediation` | optional object | 안전한 다음 행동과 자동 수정 가능성 |
| `suppression` | optional SuppressionRef | v1 applied projection. M3 producer는 설정하지 않고 DiagnosticEvaluation이 소유 |
| `first_seen_at`, `last_seen_at` | UTC timestamp | v1 lifecycle projection. M3 raw observation에서는 `observed_at`과 같음 |

`RuleRef`는 `rule_id`, SemVer `rule_version`, `definition_fingerprint`, `fingerprint_contract_version`을 가진다. external tool code는 producer provenance와 mapping input일 뿐 Rule ID를 대신하지 않는다.

`EvidenceRef`는 허용 variant를 tagged union으로 정의한다. raw arbitrary path·URL·JSON pointer를 자유 형식 map으로 넣지 않는다. 큰 byte와 external raw report는 계속 ArtifactRef다.

공개 Rust type과 `star.diagnostic` JSON Schema는 `crates/foundation/star-contracts`가 소유한다. adapter는 severity, confidence, status 의미를 다시 정의하거나 baseline·suppression으로 원래 severity·confidence·status를 변경하지 않는다.

M3 Diagnostic은 한 ValidationRun·ScanRun에서 생긴 immutable raw observation이다. 같은 fingerprint를 다음 실행에서 다시 관찰해도 새 Diagnostic ID·sequence를 만들고 이전 byte를 갱신하지 않는다. first/last seen, open/resolved와 occurrence count는 관리 DB의 재구축 가능한 issue lifecycle projection이며 Gate evidence의 raw Diagnostic을 대체하지 않는다.

### severity와 confidence

- severity는 맞다고 가정했을 때의 영향이다.
- confidence는 그 판단이 사실일 가능성이다.
- `critical + low confidence`를 숨기지 않고 확인이 필요한 중대한 의심으로 표시한다.
- parser 실패나 증거 부족을 자동으로 `confirmed`로 올리지 않는다.
- baseline·suppression과 Gate threshold는 severity가 아니며 raw Diagnostic 생성 뒤 별도로 평가한다.

### 위치와 fingerprint

LocationRef는 ProjectId, project-relative path, 1-based start, exclusive end, optional symbol을 가진다. 절대 경로만 있는 외부 자료는 persisted Diagnostic에 raw path를 넣지 않고 opaque LocalPathRef와 redacted label로 분리한다.

fingerprint는 Rule ID·fingerprint contract version, ProjectId, 정규화된 ownership anchor와 문제의 안정 특징으로 만든다. line 이동·message wording·timestamp·tool render 순서만으로 새 문제처럼 폭증하지 않도록 rule별 fingerprint input을 Catalog에 선언한다. Rule definition 또는 fingerprint contract가 바뀌면 compatible migration이 없는 이전 Baseline·Suppression을 자동 적용하지 않는다.

### suppression과 해결

- Suppression의 stable ID, shared/local 정본, revision·stale 규칙은 [공통 개발 관리 계약](development-management.md)이 소유한다. v1 Diagnostic은 적용된 SuppressionRef를 가질 수 있고 M3는 GateDecision의 DiagnosticEvaluation에서 이를 연결한다.
- suppression은 Diagnostic을 삭제하거나 severity를 바꾸지 않는다.
- 대상 fingerprint 또는 rule, project scope, 이유, ActorRef, 생성·만료 시각을 가진다.
- 코드·설정 revision이 달라져 scope hash가 맞지 않으면 다시 검토한다.
- 다음 complete current 검사에서 관찰되지 않으면 기존 Diagnostic을 수정하지 않는다. BaselineEntry를 대상으로 한 DiagnosticEvaluation `not_observed`와 별도 lifecycle projection만 갱신한다.

M3 producer는 suppression을 적용하기 전 raw Diagnostic을 commit한다. Gate engine은 별도 DiagnosticEvaluation에서 `active|expired|stale|revoked|invalid`를 계산한다. v1 `status=suppressed|resolved`를 읽을 때는 원래 관찰 status와 SuppressionRef 또는 lifecycle 근거를 복구할 수 있는 경우에만 v2 observation+projection을 만들고, 복구할 수 없으면 `unverified`와 migration Diagnostic을 남긴다.

### remediation

remediation은 다음 typed field만 사용한다.

- `action_kind=edit_source|edit_test|update_contract|update_documentation|update_schema|migrate_consumer|regenerate|rerun|replan|review|review_prerequisite|inspect_environment|remove_secret|restore_policy|prepare_reproduction|request_external_refresh|prepare_dependency_patch|request_patch_approval|plan_rollback|plan_restore`
- project-relative target selector 또는 Catalog action ref
- 적용 전 precondition과 필요한 permission
- 수정 뒤 다시 실행할 Check/Rule family
- 자동 수정 가능 여부와 `safe_auto_fix=false`가 된 이유

raw replacement text, shell command와 secret을 remediation에 넣지 않는다. 자동 수정 가능 표시는 Patch engine의 pre/post Gate와 별도 PermissionPlan을 생략할 권한이 아니다.

doctor·clean-room Diagnostic의 `review_prerequisite`와 `inspect_environment`는 사람이 수행할 설치·다운로드·system setting 변경의 필요성을 설명할 수 있지만 실행 가능한 command나 action token을 포함하지 않는다. 6단계에서는 항상 `safe_auto_fix=false`다.

M7의 `request_external_refresh`와 `request_patch_approval`은 승인 요청을 설명할 뿐 승인 token이 아니다. `prepare_dependency_patch`는 isolated preview와 immutable PatchSet 생성까지만 허용하고, `plan_rollback|plan_restore`는 실행을 내포하지 않는다. network/download/dependency change·민감 dump가 필요한 remediation은 항상 `safe_auto_fix=false`다.

새 remediation action kind는 아직 미구현인 M3 Diagnostic v2 목표 Schema에 함께 포함한다. v2가 배포된 뒤 action enum을 추가해야 한다면 old reader의 unknown-enum 처리를 먼저 비교하고 [Version과 Migration 계약](versioning-and-migrations.md)에 따라 새 schema version 또는 명시적 compatibility entry를 발행한다.

## 사용자 메시지 규칙

stable error code와 Diagnostic Rule ID는 기계 계약이고 display `message`는 사용자 표현이다. 오탈자·문장 개선·지역화만으로 code를 바꾸지 않는다. 기존 code의 의미·owner·복구 행동이 달라지면 같은 code에 새 뜻을 덮지 않고 새 ManagedDeclaration과 새 code를 만든다. deprecated code는 bounded alias와 consumer 전환 근거가 있을 때만 호환 입력으로 받을 수 있고, removed 뒤 code·alias·tombstone을 재사용하지 않는다.

CLI process exit code는 error/diagnostic ID와 다른 관리 kind다. 첫 수직 Slice에는 포함하지 않고 후속 지원 순서에서 별도 uniqueness·lifecycle을 적용한다.

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

M3 validation을 기다리는 CLI는 `auto_pass`이면서 EvidenceBundle·ReviewPack packaging이 complete일 때만 0, `human_review`는 3, `block`은 5를 반환한다. required Check launch/adapter 실패로 Gate가 block이면 사용자용 최종 code는 5이고 원인 ErrorEnvelope의 code 4 정보를 함께 보존한다. Gate engine invariant는 7, decision 뒤 evidence packaging I/O 실패는 4이며 기존 decision을 성공 완료로 출력하지 않는다.

M6 `contract.compare`, `docs.check`, `project.doctor`와 `clean-room.readiness`를 Gate mode로 실행하면 blocking Diagnostic/not-ready는 5, 의미 판단 대기는 3, complete하고 blocking issue가 없을 때만 0이다. manifest·baseline·input 계약 오류는 2, probe/adapter/report 생성 실패는 4다. `contract.snapshot`, `config.trace`, `environment.fingerprint`의 조회 mode는 complete snapshot을 만들면 finding이 있어도 0을 반환할 수 있지만 `completeness=partial|unverified`를 0으로 숨기지 않고 4를 사용한다. JSON body의 stable code·Diagnostic·completeness가 process exit code보다 상세한 정본이다.

M7 `failures inspect`, `security inspect`, `deps scan|status`와 `maintenance radar` 조회 mode는 complete snapshot을 만들면 finding이 있어도 0일 수 있다. stale·partial·unverified input 때문에 snapshot 자체를 완성하지 못하면 4이며 current clean으로 출력하지 않는다. `failures reproduce`의 승인/외부 조건 대기, `security refresh`와 `deps prepare|apply`의 network·download·dependency/PatchSet 승인 대기는 3이다. adapter 실행 실패는 4, M3 Gate block은 5, invalid pack·source descriptor·update plan은 2다. `deps prepare`가 PatchSet을 만들고 `awaiting_apply_approval`에 도달한 것은 command 성공 0일 수 있지만 dependency update 적용·검증 완료로 표시하지 않는다.

M8 `migration inspect|plan|status`, `performance compare`와 `language migration plan|status` 조회 mode는 complete typed result를 만들면 비성공 domain state가 있어도 0일 수 있다. 이때 `partially_succeeded`, `no_measurement`, `not_comparable`, `inconclusive`, `equivalence_incomplete`를 성공·개선·동등으로 표시하지 않는다. plan이 effect 승인 대기에 도달한 것 역시 planning command 성공 0일 수 있지만 `execute|resume|rollback|run|cutover`가 exact 승인을 기다리면 3이다. contract/plan/version 입력 오류는 2, adapter·tool·measurement/report 생성 실패는 4, M3 Gate block은 5, 내부 state projection invariant 위반은 7이다. `outcome_unknown|rollback_failed`는 자동 재실행하지 않고 원인 4와 최종 Gate 5를 함께 보존한다.

M9 `change-bundle plan|show|preflight|status|conflicts|release-handoff plan` 조회 mode는 요청한 typed document·limitation을 완전하게 만들면 participant가 `awaiting_apply|partially_applied|rollback_required|held|outcome_unknown`이어도 조회 자체는 0일 수 있다. 이때 bundle 완료나 전체 성공으로 렌더링하지 않는다. local/remote effect command가 exact approval을 기다리면 3, Git·worktree·remote adapter 실행 실패나 effect command의 미확정 결과는 4, project/Goal Gate block은 5, Schema/version 불일치는 6, state reducer·receipt invariant 위반은 7이다. dependency cycle·stale participant·conflict review 때문에 effect를 시작하지 않은 경우 machine body의 stable error와 `pending_action`을 보존하고, 입력을 고쳐야 하면 2, 승인·사용자 선택·재계획을 기다리면 3을 사용한다. 이후 `status`가 current `outcome_unknown`을 정확히 조회해 0을 반환하더라도 원래 effect attempt의 4를 바꾸지 않는다.

M10 `release plan|preflight|status|evaluate|package dry-run` 조회·계획 명령은 complete typed result를 만들면 `blocked|ready|needs_review|publish_outcome_unknown|rollback_required`여도 조회 자체는 0일 수 있으나 approved/published/accepted로 표시하지 않는다. required input·metadata·policy 오류는 2, publish·deploy·withdrawal·rollback 승인 대기는 3, build/installer/provider adapter 실패와 미확정 remote outcome은 4, release 또는 validator Gate block은 5, Schema/version 불일치는 6, artifact/status/evaluation reducer invariant 위반은 7이다. `ready` 생성은 0일 수 있지만 승인이나 공개 성공이 아니다. `release approve`는 exact approval 기록만 만들며 remote effect를 실행하지 않고, `release publish|deploy|withdraw|rollback`은 별도 action 승인을 요구한다. after RemoteStateSnapshot이 exact artifact digest를 확인하기 전에는 effect command가 provider receipt를 받았어도 published 성공 0으로 종결하지 않는다.

M11 `style rust inspect`는 complete discovery snapshot을 만들면 unpinned toolchain limitation이 있어도 조회 0일 수 있으나 auto-apply 가능으로 표시하지 않는다. `check`는 requested fmt/Clippy coverage와 Diagnostic report가 complete하면 style drift/허용되지 않은 lint가 있어도 typed result와 Gate mode에 따라 0 또는 5를 사용한다. incomplete/unavailable tool output은 4이고 config/scope 입력 오류는 2다. `prepare`는 no-op 또는 immutable PatchSet 생성까지가 성공 0이며 apply 완료가 아니다. `safe_default` exact 승인 대기와 `personal_auto` scope mismatch의 사용자 선택 대기는 3, candidate/pre/post Gate block은 5, process failure는 4다. `auto-apply`는 actual-after post Gate와 EvidenceBundle packaging이 complete할 때만 0이며 partial apply·post failure·recovery required를 rollback 성공으로 숨기지 않는다.

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
