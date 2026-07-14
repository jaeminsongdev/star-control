# 실패 재현·보안·의존성 유지보수 계약

## 상태와 문서 소유권

이 문서는 Star-Control 사용자 로드맵 **7단계**의 의미 정본이다. 제품 로드맵의 `P7. 원격 저장소 연동`과 번호가 같아 보일 수 있으므로, 이 문서와 관련 Schema·CLI에서는 `M7`을 안정된 단계 식별자로 사용한다.

현재 상태는 **문서 설계 확정 대상·제품 구현 전**이다. debugger, trace 수집기, secret scanner, vulnerability scanner, license scanner, dependency updater, package resolver, network client, 자체 취약점 DB와 PKI가 구현됐다는 뜻이 아니다.

상위 소유권은 다음과 같다.

| 관심사 | 정본·소유자 | M7의 역할 |
|---|---|---|
| Finding·Occurrence·Evidence·Suppression·Disposition | [공통 개발 관리 계약](development-management.md)과 [검증·증거 계약](validation-and-evidence.md) | 재사용한다. 별도 security/failure DB나 진단 모델을 만들지 않는다. |
| Project·package·dependency relation, manifest·lockfile 관찰 | [Project Catalog·Code Index 계약](project-catalog-and-code-index.md) | current snapshot을 입력으로만 사용한다. |
| TaskSpec·ChangePlan·ValidationPlan | [변경 계획·영향 분석 계약](change-planning-and-impact.md) | 재현·update 범위와 검사 계획을 materialize한다. |
| PatchSet·PatchApplication·rollback | [안전한 Patch·codemod 계약](safe-patch-and-codemod.md) | package manager 결과를 immutable PatchSet으로 제안하고 즉시 적용하지 않는다. |
| GateDecision·EvidenceBundle·ReviewPack | [검증·증거 계약](validation-and-evidence.md) | 완료 판단을 위임한다. scanner·debugger·package manager adapter는 Gate를 판정하지 않는다. |
| 계약·문서·config·환경 drift와 M7 입력 | [계약·호환성·환경 계약](contract-compatibility-and-environment.md) | `DependencySecurityInputManifest`와 drift evidence를 소비한다. |
| M7의 실패·공급망·dependency·Radar 의미 | 이 문서 | persisted projection과 결정 규칙을 정의한다. |

## 목표와 제외 범위

### 목표

1. compile, test, runtime, tool, environment 실패를 같은 identity 규칙과 evidence 구조로 기록한다.
2. 첫 원인 후보와 연쇄 오류를 구분하고, 수정 전 실패·수정 후 성공·재발을 연결한다.
3. 재현에 필요한 최소 자료를 일반 실행 log와 분리한 `ReproductionPack`으로 만든다.
4. secret·token·개인정보, auth·permission·crypto·workflow·release 위험을 공통 Finding과 Evidence에 정규화한다.
5. dependency 관계, 상태, 외부 advisory·license provenance와 freshness를 명시한다.
6. dependency 변경을 사용자가 검토·승인할 수 있는 `PatchSet`으로 준비하고, package manager가 lockfile을 소유하게 한다.
7. 재발 실패, 만료 suppression, 오래된 dependency, 미해결 security finding, flaky test, 문서·환경 drift를 결정적 `MaintenanceRadarSnapshot`으로 정렬한다.
8. 다음 사용자 8단계 migration·performance가 사용할 reproduction·before/after·rollback·restore 근거를 남긴다.

### 제외 범위

- debugger, tracer, profiler, scanner, package manager 또는 updater 자체 구현
- vulnerability·license·package registry의 원본 DB 복제·재배포
- CVE/CPE/OSV/SPDX ID 발급, 서명, 인증서 발급, key 보관과 PKI
- 승인 없는 network read, package download, dependency 추가·변경, lockfile 갱신
- package manager resolution algorithm의 재구현 또는 lockfile 역산
- AI 위험 점수, AI-only root cause 판정, AI-only 완료 판정
- source tree 자동 수정, update 자동 적용, release 발행

## 선행조건과 preflight

M7 command는 다음 입력을 exact subject 기준으로 preflight한다.

| 입력 | 필수 조건 | 실패 시 |
|---|---|---|
| `ProjectCatalogSnapshot`·`CodeIndexSnapshot` | target Project·Checkout·Workspace와 dependency partition이 current | `DEPENDENCY_INPUT_STALE`, `BLOCK` |
| 공통 Finding·Evidence store | v2 Diagnostic, EvidenceSubjectBinding, suppression·baseline semantics 사용 가능 | `MAINTENANCE_STORE_INCOMPATIBLE`, `BLOCK` |
| `ValidationPlan`과 M3 Gate | required Check·Rule과 completion policy가 materialize됨 | `VALIDATION_PLAN_REQUIRED`, `BLOCK` |
| `DependencySecurityInputManifest` | exact revision, manifest·lockfile hash·relation, package manager·toolchain·environment, coverage·freshness 포함 | `DEPENDENCY_INPUT_INCOMPLETE`, `BLOCK` |
| contract·environment drift 결과 | unresolved protected drift가 숨겨지지 않음 | current Finding으로 승격하고 Gate가 판정 |

preflight는 source·manifest·lockfile을 쓰지 않으며 network에 접근하지 않는다. 입력이 없거나 stale이면 빈 결과를 `clean`으로 만들지 않고 `unknown` 또는 `unverified`로 끝낸다.

## 공통 식별자와 persisted 계약

모든 최상위 문서는 [계약 지도](README.md)의 공통 Envelope를 사용한다.

| 계약 | `schema_id` | 안정 ID 예 | 역할 |
|---|---|---|---|
| `FailureRecord` | `star.failure-record` | `flr_<base32-sha256>` | 한 실패 occurrence와 원인·cascade 관계 |
| `ReproductionPack` | `star.reproduction-pack` | `rpk_<base32-sha256>` | 재현에 필요한 최소 manifest와 redacted artifact 참조 |
| `RegressionRecord` | `star.regression-record` | `rgr_<base32-sha256>` | before failure, after pass, 이후 재발의 호환 가능한 쌍 |
| `RecoveryPlan` | `star.recovery-plan` | `rcp_01J...` | rollback·roll-forward·restore 절차와 검증 |
| `DependencySnapshot` | `star.dependency-snapshot` | `dps_<base32-sha256>` | exact subject의 dependency와 내부 package 관계 |
| `SupplyChainSnapshot` | `star.supply-chain-snapshot` | `scs_<base32-sha256>` | dependency·workflow·release·외부 자료 관찰 |
| `ExternalDataSnapshot` | `star.external-data-snapshot` | `eds_<base32-sha256>` | 외부 advisory·license·version 자료의 provenance와 freshness |
| `DependencyUpdatePlan` | `star.dependency-update-plan` | `dup_01J...` | 후보·영향·승인·PatchSet 상태 |
| `MaintenanceRadarSnapshot` | `star.maintenance-radar-snapshot` | `mrsd_<base32-sha256>` | 기존 자료를 정렬한 파생 유지보수 view |

이 계약은 별도 DB를 요구하지 않는다. 구조화 문서는 공통 관리 repository의 document/event 저장 계층에 저장하고, 큰 log·dump·trace·diff는 `ArtifactRef`로만 연결한다. Radar와 dashboard는 위 문서와 공통 Finding·Suppression을 다시 읽어 계산 가능한 projection이다.

## 실패 identity

### FailureRecord

`FailureRecord`의 최소 필드는 다음과 같다.

| 필드 | 필수 | 의미 |
|---|---:|---|
| `failure_record_id`, `occurrence_id` | 예 | 문서와 공통 Occurrence identity |
| `diagnostic_refs`, `finding_refs` | 예 | 공통 raw observation·issue lifecycle 정본 |
| `subject_binding` | 예 | Project·Checkout·WorkspaceSnapshot·ProjectRevision·ChangeSet·ValidationRun |
| `failure_kind` | 예 | `compile\|test\|runtime\|tool\|environment` |
| `family_fingerprint` | 예 | revision을 넘어 같은 실패 계열을 묶는 fingerprint |
| `occurrence_fingerprint` | 예 | exact revision·command·input·seed·environment의 실행 occurrence |
| `primary_symptom` | 예 | 정규화한 code, message template, owner anchor, stack/test/tool signature |
| `causality_role` | 예 | `root_candidate\|cascade\|independent\|unknown` |
| `root_candidate_refs` | 조건부 | candidate와 confidence·evidence·reason |
| `cascade_parent_refs` | 조건부 | 연쇄 오류 DAG edge. cycle 금지 |
| `invocation` | 예 | command descriptor, executable/tool ID, structured args, logical cwd |
| `environment_fingerprint` | 예 | 호환성 class와 exact fingerprint를 함께 보유 |
| `input_refs`, `seed` | 조건부 | 민감 원문이 아닌 ArtifactRef·content fingerprint·seed |
| `stdout_ref`, `stderr_ref`, `artifact_refs` | 조건부 | redaction·retention이 적용된 evidence |
| `observed_at`, `attempt_id` | 예 | occurrence 시점과 실행 attempt |
| `verification_state` | 예 | `verified\|partially_verified\|unverified\|contradicted` |

`root_candidate`는 원인을 확정했다는 뜻이 아니다. compiler/test runner/tool이 제공한 causal code, 가장 이른 독립 실패, dependency DAG 또는 debugger evidence처럼 명시적 근거가 있을 때만 candidate가 된다. 단순 출력 순서만으로 root를 확정하지 않는다. 확정할 수 없으면 복수 candidate와 confidence를 남기고 `HUMAN_REVIEW`로 보낸다.

`FailureRecord`는 두 번째 Diagnostic이 아니다. severity, message parameter, location, suppression, disposition, open/resolved lifecycle과 Gate effect는 공통 Diagnostic·Finding 계약만 소유한다. `primary_symptom`은 family fingerprint를 재현하기 위한 redacted projection이고 원본 observation은 `diagnostic_refs`로 추적한다.

### 두 fingerprint

같은 fingerprint의 재발과 exact 재현을 동시에 지원하기 위해 identity를 둘로 나눈다.

`family_fingerprint`는 canonical JSON을 SHA-256한 값이며 최소 다음을 포함한다.

- contract version과 `failure_kind`
- producer의 stable Rule·Check·external diagnostic code
- 경로·주소·PID·timestamp·임시 ID를 제거한 primary message template
- test name, symbol, package, module 또는 tool phase 같은 logical owner anchor
- command descriptor와 의미 있는 structured arg shape
- environment compatibility class와 tool compatibility class

다음은 `family_fingerprint`에서 제외한다.

- source revision, wall-clock time, random process ID
- username, home/temp 절대 경로, secret·token·개인정보
- stack address, allocator address, 임시 file name
- raw stdout·stderr bytes

`occurrence_fingerprint`는 `family_fingerprint`에 다음 exact binding을 더해 계산한다.

- ProjectRevision·WorkspaceSnapshot·ChangeSet
- normalized structured args와 logical cwd
- full environment fingerprint와 tool identity/version
- input content fingerprint와 seed
- relevant manifest·lockfile fingerprint

정규화기는 원문을 보관하기 전에 redaction하고, normalization rule version을 fingerprint 입력에 포함한다. rule version이 다르면 자동으로 같은 family라고 병합하지 않는다.

### causality와 dedup

1. adapter는 raw diagnostic을 손실 없이 공통 `Diagnostic`으로 정규화한다.
2. core는 `family_fingerprint`가 같아도 occurrence를 삭제하지 않는다.
3. root candidate와 cascade는 `causality_edges`로 연결한다.
4. 여러 scanner·runner가 같은 현상을 보고하면 producer별 evidence를 유지하고 `correlated_finding_refs`로만 연결한다.
5. 서로 다른 root candidate를 하나의 finding으로 강제 병합하지 않는다.

## ReproductionPack

### 일반 log와의 구분

일반 log는 한 ValidationRun 또는 ToolInvocation의 시간순 출력이다. `ReproductionPack`은 그중 실패를 다시 만들거나 재현 불가를 검증하는 데 필요한 최소 입력·명령·환경·예상 결과만 선별한 immutable manifest다.

- pack은 전체 run directory를 암묵적으로 포함하지 않는다.
- stdout·stderr·dump·trace는 pack 본문이 아니라 redacted `ArtifactRef`다.
- 같은 artifact를 참조할 수 있지만 `artifact_role`을 `general_log`와 `reproduction_required`로 구분한다.
- default report와 ReviewPack에는 `safe_for_default_report=true`인 요약과 artifact metadata만 포함한다.
- `quarantined`, `unknown` redaction 상태의 artifact는 default report에서 제외한다.

### 최소 구성

| 영역 | 최소 필드 |
|---|---|
| identity | `reproduction_pack_id`, failure family·occurrence ref, schema·normalization version |
| subject | Project·Checkout·WorkspaceSnapshot·ProjectRevision·ChangeSet, dirty state |
| invocation | registered Tool·Task·Check ID, executable identity, structured args, logical cwd, timeout·resource limits |
| environment | compatibility class, exact redacted fingerprint, relevant toolchain/runtime/package manager, manifest·lockfile refs |
| input | input ArtifactRef 또는 deterministic generator descriptor, content fingerprint, seed |
| expectation | expected result·exit class·assertion과 observed result |
| attempts | rerun attempt, result, duration, fingerprint, variance와 raw evidence refs |
| artifacts | stdout·stderr·dump·trace·core·screenshot·generated result의 role, redaction, retention |
| external conditions | service/version/state, clock/network/device 같은 조건과 검증 여부 |
| minimization | reducer identity, original/reduced input fingerprints, semantic preservation evidence |
| conclusion | `reproduction_state`, confidence, limitations, next action |

`reproduction_state`는 다음 중 하나다.

- `reproduced`: compatible environment에서 같은 family가 최소 한 번 재발
- `partially_reproduced`: symptom 일부만 같거나 environment compatibility가 제한됨
- `not_reproduced`: 계획된 attempt가 모두 실행됐으나 같은 family가 관찰되지 않음
- `blocked_external`: 외부 서비스·device·clock·권한 등 재현할 수 없는 조건이 막음
- `unverified`: required input/evidence가 없거나 attempt가 실행되지 않음

`not_reproduced`는 성공이나 해결을 의미하지 않는다. 외부 조건을 local evidence로 확인할 수 없으면 반드시 `blocked_external` 또는 `unverified`이며 Gate는 이를 pass 근거로 쓰지 않는다.

### 재현 절차

M7 orchestration은 다음 순서를 사용한다.

1. exact subject·manifest·lockfile·environment freshness를 preflight한다.
2. 기존 command를 string으로 재조립하지 않고 registered invocation의 structured args를 사용한다.
3. 동일 input·seed·resource limit으로 bounded rerun을 수행한다.
4. 필요하면 registered reducer adapter로 input reduction을 시도한다. 원본과 축소본을 모두 보존하고 동일 family evidence를 요구한다.
5. revision 범위가 명시됐을 때만 registered VCS bisect adapter를 사용한다. 각 revision의 checkout·build prerequisite·result·skip reason을 기록한다.
6. debugger·trace adapter는 사용자가 승인한 registered tool로만 연결하며 output을 ArtifactRef로 정규화한다.
7. 수정 전 실패와 수정 후 성공은 `RegressionRecord`로 연결한다.

rerun·reducer·bisect·debugger·trace의 각 실행은 독립 `ToolInvocation`과 PermissionDecision을 가진다. adapter가 “fixed”나 “passed”를 반환해도 core Gate가 required evidence를 다시 판정한다.

## RegressionRecord와 재발

`RegressionRecord`의 최소 필드는 다음과 같다.

| 필드 | 의미 |
|---|---|
| `family_fingerprint` | 같은 실패 계열 |
| `before_failure` | verified failure occurrence와 exact subject |
| `after_success` | compatible input·environment·test identity의 complete·stable pass |
| `fix_change_set_ref` | failure와 pass 사이의 ChangeSet/PatchApplication |
| `compatibility_evidence` | command·input·seed·environment·tool의 비교 |
| `later_occurrences` | 이후 같은 family의 occurrence 목록 |
| `regression_state` | `candidate\|verified_fixed\|regressed\|resolved\|unverified` |

`verified_fixed`는 before failure와 after success가 모두 current evidence이고 호환 가능할 때만 설정한다. 이후 같은 `family_fingerprint`가 호환 가능한 scope에서 verified되면 `regressed`다. fingerprint만 같지만 input·environment 호환성이 불명확하면 `candidate` 또는 `unverified`로 유지한다.

flaky는 별도 성공 상태가 아니다. 동일 subject·input·seed에서 attempt 결과가 갈리면 `stability_state=flaky`이고 required evidence라면 최소 `HUMAN_REVIEW`, protected correctness path면 `BLOCK`이다.

## RecoveryPlan

복구 방식은 다음 세 종류를 섞지 않는다.

| kind | 의미 | 대표 예 |
|---|---|---|
| `rollback` | 변경 전 검증된 code/config/artifact 상태로 되돌림 | PatchSet reverse, 이전 lockfile 복원 |
| `roll_forward` | 새 correction·migration을 앞으로 적용해 정상 상태 도달 | 후속 migration PatchSet |
| `restore` | backup·snapshot·export에서 data/runtime state 복구 | DB snapshot restore, artifact restore |

`RecoveryPlan`은 `recovery_kind`, exact subject, prerequisite, ordered step, destructive effect, permission requirement, expected checkpoint, validation Check, stop condition, fallback, owner와 evidence slot을 가진다. 복수 방식이 필요하면 하위 plan으로 분리하고 순서를 DAG로 표현한다.

실행하지 않은 복구 절차는 `planned_unverified`다. rehearsal 또는 실제 수행은 `RecoveryAttempt`로 분리하고, 복구 전·후 상태와 M3 Gate evidence를 기록한다. rollback 성공이 data restore 성공을 의미하지 않으며 반대도 같다.

## 민감 자료 redaction과 retention

### 수집 전 차단

다음 원문은 persisted 계약, fingerprint, log bundle에 넣지 않는다.

- secret, password, API key, access/refresh token, private key와 session material
- 주민번호·결제정보·인증정보 등 정책상 개인정보
- username·home path·임시 path처럼 개인을 식별할 수 있는 절대 경로
- memory dump·environment dump에 포함된 위 값

탐지 결과는 값 대신 `candidate_kind`, redacted location, detector ID/version, confidence와 count만 남긴다. secret 후보의 raw value나 그 hash를 identity로 사용하지 않는다.

### artifact 처리

| 상태 | 저장·보고 규칙 |
|---|---|
| `not_needed` | 민감 정보가 없음을 검증한 artifact만 일반 evidence로 사용 |
| `redacted` | detector·rule version, 변환 전후 byte count와 안전한 output hash 기록 |
| `quarantined` | redaction 판정 중인 bounded staging 또는 정책상 non-secret 민감 artifact만 제한된 local 경로·ACL·retention으로 격리, default report 제외 |
| `unknown` | 완료 근거와 default report에서 제외, 재검토 필요 |
| `dropped_sensitive` | bytes를 저장하지 않고 존재·kind·drop reason만 기록 |

retention은 공통 `temporary\|run\|evidence\|hold` class를 사용한다. unresolved regression·security finding·rollback 근거는 closure까지 `hold` 가능하지만, raw sensitive artifact는 hold를 자동 연장하지 않는다. 만료 시 artifact를 삭제해도 문서에는 tombstone, safe digest가 이미 있으면 그 값, redaction state, deletion reason과 시각을 남긴다. 삭제 자체는 별도 명시적 사용자 승인과 운영 정책을 따른다.

확인된 secret·token·PII bytes는 `quarantined`로 장기 보관하지 않고 persistence 전에 제거하거나 artifact 전체를 `dropped_sensitive`로 폐기한다. quarantine은 이 금지를 우회하는 secret vault가 아니다.

## 보안·공급망 관찰

### SupplyChainSnapshot

`SupplyChainSnapshot`은 exact ProjectRevision·WorkspaceSnapshot에 다음을 결합한다.

| 영역 | 최소 관찰 |
|---|---|
| secret·PII | candidate kind, redacted location, detector provenance, redaction 결과 |
| 민감 변경 | auth, session, token handling, permission, crypto, workflow 변경 marker |
| dependency | manifest·lockfile diff, dependency purpose·source·resolved/requested version·direct/transitive/internal |
| license | SPDX expression 또는 producer-native value, source, observation time, confidence |
| vulnerability | advisory ID/aliases, affected range, fixed version, severity source, match method, external snapshot ref |
| workflow | effective token/workflow permission, permission widening, external action ref와 immutable pin 여부 |
| release | release file list, logical path, media type, size, digest algorithm/value, manifest ref |
| provenance | SBOM·attestation·signature가 있으면 ArtifactRef와 verifier result; 없음을 Star-Control이 생성한 것으로 꾸미지 않음 |

auth·permission·crypto·workflow marker는 변경 사실과 검토 요구를 나타내며 취약점 확정을 뜻하지 않는다. exact semantic adapter가 없으면 `suspected` 또는 `unverified`다.

GitHub Action처럼 immutable revision을 지원하는 provider에서는 full commit digest 고정을 `pinned`로 본다. tag·branch만 있으면 `mutable_ref`다. 다른 provider는 Catalog의 provider-specific immutable identity 규칙을 사용하며 GitHub 규칙을 무리하게 적용하지 않는다.

release manifest는 Star-Control이 release를 만들거나 서명한다는 뜻이 아니다. 이미 생성된 file list·digest·manifest·SBOM·provenance를 관찰하고 Gate evidence로 묶는다.

### 외부 scanner와 DB 경계

scanner는 `ToolDescriptor`·`CheckDescriptor`로 등록된 adapter다.

- raw result는 ArtifactRef로 보존하고 common Diagnostic으로 정규화한다.
- scanner 자체 exit 0이나 “no findings”는 coverage·freshness가 complete일 때만 clean evidence 후보가 된다.
- alias가 같은 advisory라도 source별 원문과 observation time을 유지한다.
- Star-Control은 vulnerability DB, license DB, package registry, scanner engine, certificate authority를 만들지 않는다.
- 외부 DB snapshot은 source cache가 아니라 `ExternalDataSnapshot` metadata와 immutable input artifact reference다.

## ExternalDataSnapshot과 freshness

외부 vulnerability·license·available-version 자료마다 다음을 기록한다.

| 필드 | 의미 |
|---|---|
| `source_id`, `provider`, `source_url` | 데이터 출처와 공식 endpoint/document |
| `dataset_or_query` | dataset revision 또는 exact query identity |
| `schema_version` | source schema/API version |
| `published_at`, `modified_at` | source가 제공한 시간. 없으면 `unknown` |
| `fetched_at`, `observed_at` | adapter가 자료를 받은 시각과 core가 관찰한 시각 |
| `content_digest` | 수신한 immutable payload의 digest. secret payload 금지 |
| `tool_identity` | adapter·scanner name/version/config fingerprint |
| `network_mode` | `offline_cache\|approved_online` |
| `coverage` | ecosystem/package/query 범위, pagination 완료 여부, missing reason |
| `maximum_age`, `valid_until` | Catalog policy와 관찰 시각으로 계산한 유효 기한 |
| `freshness_state` | `current\|stale\|unknown\|unavailable` |

시간 우선순위는 source `modified_at`, source `published_at`, `fetched_at` 순이다. source가 시간을 제공하지 않으면 `fetched_at`만으로 content 자체 최신성을 확정하지 않고 `freshness_state=unknown`을 허용한다.

`valid_until`은 source descriptor의 `maximum_age`와 현재 Gate의 `evaluation_time`으로 결정한다. `stale`·`unknown`·`unavailable` 자료는 warning을 반드시 만들며, required security Check의 clean/pass 근거가 되지 못한다. offline 결과에는 마지막 snapshot 시각과 “현재 외부 상태를 확인하지 않음”을 표시한다.

refresh는 별도 `network_read` action이다. 현재 대화의 exact scope·source·비용·credential 사용 여부에 대한 사용자 승인 전에는 실행하지 않는다. refresh 실패 시 이전 snapshot을 덮어쓰지 않고 새 failed attempt와 이전 snapshot의 stale 상태를 함께 보존한다.

## DependencySnapshot

### 수집과 relation

M1 repository scan과 M6 input에서 다음 relation을 수집한다.

- Project → workspace/package
- package → direct external dependency
- package → transitive resolved dependency
- package → internal package/project dependency
- manifest → lockfile → package manager
- dependency → source/registry/git/path
- dependency → affected Project·Task·Check·runtime/release surface

`DependencySnapshot`은 exact manifest·lockfile hash, package manager identity/version, resolver mode, target/runtime 조건과 coverage를 가진다. manifest만 있고 lockfile이 없거나 transitive graph를 확인할 수 없으면 그 부분은 `unknown`이다.

이 snapshot은 M1 graph의 package·dependency edge를 복제해 새 정본으로 만들지 않는다. 각 entry는 current `CodeIndexSnapshot`의 entity/relation ref를 가리키고 M7은 그 exact graph에 currency·vulnerability·compatibility·external-data assessment를 덧붙인다. source graph가 바뀌면 snapshot은 stale이며 M7 DB row를 M1 graph로 되쓰지 않는다.

각 dependency는 최소 다음 축을 독립적으로 가진다.

| 축 | 상태 |
|---|---|
| currency | `current\|outdated\|unknown` |
| vulnerability | `not_affected\|vulnerable\|unknown` |
| compatibility | `compatible\|incompatible\|unknown` |
| relation | `direct\|transitive\|internal` |
| resolution | `resolved\|declared_only\|ambiguous\|unverified` |

`outdated`는 current external version evidence가 있을 때만 사용한다. 자료가 stale하면 `outdated`를 유지하더라도 `freshness_state=stale`을 함께 표시하고 current recommendation으로 승격하지 않는다. `not_affected`도 compatible package identity·version range·current advisory coverage가 필요하다.

dependency record는 purpose, ecosystem, canonical package identity, requested/resolved version, source, integrity/digest가 있으면 그 값, license evidence, advisory refs와 affected Project를 포함한다. purpose가 source에서 확인되지 않으면 추정하지 않고 `unknown`이다.

## DependencyUpdatePlan

### 후보 분류

`UpdateCandidate`는 다음을 가진다.

- candidate ID와 dependency identity
- current requested/resolved version과 proposed constraint/resolution
- `update_kind=patch\|minor\|major\|security\|internal`
- direct/transitive와 source 변경 여부
- reason과 source evidence/freshness
- affected Project·package·runtime·public contract·release surface
- expected manifest·lockfile ownership과 package manager adapter
- required ChangePlan·ValidationPlan·approval refs
- risk marker: API, auth, permission, crypto, workflow, migration, native/runtime

`security`는 SemVer 크기와 직교한다. 예를 들어 major security update는 `update_kind=security`와 `version_delta=major`를 함께 가진다. internal dependency는 registry version 비교 대신 exact ProjectRevision·public contract compatibility를 사용한다.

### 상태

`DependencyUpdatePlan.status`는 다음 상태 기계를 사용한다.

`observed → candidate → awaiting_refresh_approval → awaiting_patch_preparation_approval → patch_prepared → awaiting_apply_approval → applied → validated`

실패·대체 상태는 `blocked\|rolled_back\|superseded\|unverified`다.

- 외부 자료가 이미 current이고 offline 분석만 하면 `awaiting_refresh_approval`을 건너뛸 수 있다.
- `patch_prepared`는 source 적용 완료가 아니라 immutable PatchSet이 생성됐다는 뜻이다.
- 기본 Profile 종료점은 `awaiting_apply_approval`이다.
- `validated`는 M3 post-apply GateDecision만 설정할 수 있다.

### 변경 흐름

1. 사용자 변경 요청을 TaskSpec으로 만들고 exact dependency scope를 고정한다.
2. M1/M6 current snapshot과 external data freshness를 preflight한다.
3. M2가 affected Project, ChangePlan, ValidationPlan과 risk path를 계산한다.
4. network refresh가 필요하면 exact `network_read` approval을 기다린다.
5. package download·dependency 추가·dependency change·network access가 필요한 preview는 각각 효과를 선언하고 사용자 승인을 기다린다.
6. M4 isolated worktree에서 등록 package manager adapter가 manifest/lockfile을 생성·갱신한다.
7. 결과 byte를 before manifest·lockfile과 비교해 ChangeSet을 만들고 M2가 실제 diff로 재계획한다.
8. immutable PatchSet, previous lockfile ArtifactRef, rollback recipe와 ReviewPack을 만든다.
9. `awaiting_apply_approval`에서 멈춘다.
10. 사용자가 exact PatchSet을 승인한 뒤에만 M4가 적용하고 M3가 post-apply validation을 실행한다.

preview 자체가 download·dependency graph 변경을 일으키면 승인 대상이다. `--dry-run`처럼 도구가 쓰지 않는다고 주장해도 network·cache write·credential use가 있으면 해당 action approval이 필요하다.

### lockfile 소유권

- lockfile은 해당 ecosystem의 등록 package manager가 생성·갱신한다.
- Star-Control core는 lockfile graph를 읽고 diff하지만 resolver를 재구현하거나 version closure를 역산하지 않는다.
- text editor·codemod로 resolved entry를 직접 조작하지 않는다.
- package manager identity/version/config/source와 실행 structured args를 Evidence에 기록한다.
- 변경 전 manifest·lockfile bytes와 hash를 immutable ArtifactRef로 보존한다.
- package manager가 예상 밖 file을 쓰면 scope violation으로 `BLOCK`하고 PatchSet을 만들지 않는다.
- rollback은 보존한 before bytes를 M4 PatchSet 규칙으로 복원하고 같은 manager의 locked/offline 검증과 M3 Gate를 거친다.

package 추가는 update와 다른 effect다. 사용자가 “upgrade”를 승인했어도 새 direct dependency 추가를 승인한 것으로 보지 않는다.

## update dashboard

dashboard는 별도 mutable 진실 저장소가 아니라 `DependencySnapshot`, `DependencyUpdatePlan`, PatchSet, PermissionDecision, GateDecision을 결합한 projection이다. 최소 열은 다음과 같다.

- dependency·current/proposed version·update kind
- affected Project·package·risk marker
- currency/vulnerability/compatibility 상태
- 외부 source와 freshness·valid_until
- PatchSet ID·diff size·manifest/lockfile owner
- approval checkpoint와 현재 대기 이유
- last validation·rollback readiness·previous lockfile 보존 여부

기본 정렬은 unresolved security, incompatible, stale/unknown evidence, major/internal impact, 오래 대기한 approval 순이며 [Maintenance Radar](#maintenance-radar)의 결정 규칙을 재사용한다.

## Maintenance Radar

### 입력과 item

`MaintenanceRadarSnapshot`은 다음 자료를 읽는 파생 view다.

- 같은 `family_fingerprint`로 재발한 실패와 `RegressionRecord`
- 만료됐거나 곧 만료되는 Suppression
- outdated 또는 freshness가 stale/unknown인 dependency
- unresolved security Finding
- flaky required test
- contract·docs·config·environment drift Finding
- rollback·restore evidence가 없는 high-risk change

`RadarItem`은 새 Finding을 복제하지 않고 원본 `finding_refs`, `diagnostic_refs`, `dependency_refs`, `regression_refs`, `suppression_refs`와 evidence를 가리킨다. item은 category, affected Project, severity, protected-risk flag, recurrence, freshness, evidence completeness, age, due/expiry와 stable sort key를 가진다.

### AI 없는 결정적 정렬

AI 점수 없이 다음 tuple을 오름차순 priority number로 정렬한다.

1. `blocking_rank`: active BLOCK/protected invariant → required HUMAN_REVIEW → warning → info
2. `risk_rank`: critical → high → medium → low → unknown
3. `freshness_rank`: expired → stale → unknown/unavailable → current
4. `regression_rank`: verified regression → recurring → first seen → no failure relation
5. `evidence_rank`: contradicted → missing/unverified → partial → complete
6. `time_rank`: suppression expiry/due가 이른 순, 그다음 first-seen이 오래된 순
7. `stable_identity`: ProjectId, category, family/finding/dependency ID

같은 input snapshot과 `evaluation_time`이면 같은 순서가 나와야 한다. optional AI는 요약·설명만 만들 수 있고 priority, GateDecision, approval state를 바꾸지 못한다.

`risk_rank`는 common Rule severity, protected RiskPath와 Gate policy의 versioned mapping에서만 계산한다. 자연어 요약, model confidence, 사용자 활동량과 vendor marketing score를 숨은 가중치로 쓰지 않는다.

Radar의 `valid_until`은 입력 suppression expiry, external data valid_until, Project/Code Index freshness와 Gate time boundary 중 가장 이른 값이다. 경계를 넘으면 dashboard는 `stale` banner와 refresh/re-evaluate action을 표시하며 이전 snapshot을 current로 표시하지 않는다.

## Profile·CLI 의미

세 Profile의 canonical closure는 [Profile 계약](../features/profiles.md)에 있으며 M7 관점의 종료점은 다음과 같다.

| Profile | 기본 동작 | 성공 산출물 | 멈춤 조건 |
|---|---|---|---|
| `debug_recovery` | read-only preflight, bounded rerun, reproduction·recovery 계획 | ReproductionPack, RegressionRecord, RecoveryPlan, EvidenceBundle | 외부 조건·민감 artifact·debugger permission이면 승인/검토 대기 |
| `security_supply_chain` | offline/current 입력 분석, registered scanner 결과 정규화 | SupplyChainSnapshot, common Finding, freshness 상태 | stale external data는 refresh 승인 또는 Gate warning/block |
| `dependency_upgrade` | 후보·영향·검증·rollback 설계와 isolated PatchSet 준비 | DependencyUpdatePlan, PatchSet, ReviewPack | 기본 `awaiting_apply_approval`; 자동 적용 금지 |

목표 CLI surface는 다음 의미만 가진다. command spelling은 구현 단계에서 CLI 계약과 함께 동결한다.

- `star failures inspect|reproduce|compare|recovery-plan`
- `star security inspect|refresh-plan|release-manifest`
- `star deps scan|candidates|prepare|status|rollback-plan`
- `star maintenance radar`

`refresh-plan`과 `rollback-plan`은 계획을 만들 뿐 실행 승인을 내포하지 않는다. `deps prepare`도 required permission을 모두 받은 isolated preview만 수행하며 live apply하지 않는다.

## Permission과 효과

M7 Profile은 Profile metadata로 권한을 획득하지 못하며 더 엄격하게 만들 수만 있다.

| action | M7 기본 | 설명 |
|---|---|---|
| source·manifest·lockfile read | allow | scope 안 read-only scan |
| local derived document/evidence write | allow | redacted 공통 상태 저장 |
| `network_read` | prompt | advisory/version/license refresh |
| `network_download` | prompt | package·tool·large external artifact download |
| `dependency_change` | prompt | add/remove/update/lockfile change |
| debugger attach·process control | prompt | registered adapter와 exact process scope |
| sensitive dump capture | prompt + redact-before-persist | 안전한 redaction을 증명하지 못하면 bytes drop, default report 제외 |
| PatchSet live apply | exact PatchSet prompt | dependency Profile은 준비 뒤 중지 |

`personal_auto` 같은 상위 mode도 M7의 network/download/dependency change와 sensitive capture를 자동 승인하지 않는다. 승인에는 action, Project/Checkout, source/provider, package/candidate, PatchSet hash, 예상 file scope, credential·비용 여부와 expiry를 포함한다.

## M3 Gate 통합

adapter는 Diagnostic·ArtifactRef만 생산하고 M3 core가 다음을 판정한다.

### failure Rule family

- `star.validation.failure.reproduction-unverified`
- `star.validation.failure.identity-changed`
- `star.validation.failure.after-evidence-incompatible`
- `star.validation.failure.after-flaky`
- `star.validation.failure.recovery-plan-unverified`
- `star.validation.failure.sensitive-artifact-unsafe`

### security Rule family

- `star.validation.security.secret-candidate`
- `star.validation.security.redaction-failed`
- `star.validation.security.dangerous-command-candidate`
- `star.validation.security.dangerous-command-executable`
- `star.validation.security.workflow-permission-widened`
- `star.validation.security.external-action-mutable-ref`
- `star.validation.security.external-database-stale`
- `star.validation.security.external-scan-unverified`
- `star.validation.security.release-manifest-incomplete`

### dependency Rule family

- `star.validation.dependency.input-stale`
- `star.validation.dependency.status-unknown`
- `star.validation.dependency.lockfile-owner-unverified`
- `star.validation.dependency.unapproved-network-effect`
- `star.validation.dependency.unapproved-change`
- `star.validation.dependency.patch-replan-required`
- `star.validation.dependency.rollback-unverified`

다음이면 `AUTO_PASS`할 수 없다.

- required ReproductionPack가 `blocked_external|unverified`
- 민감 artifact의 redaction이 `unknown|quarantined`인데 default report에 포함됨
- required external data가 stale·unknown·unavailable
- vulnerability/license/version query coverage가 partial인데 clean으로 주장
- package manager가 소유하지 않은 lockfile diff
- user approval 전 dependency PatchSet apply
- before lockfile·rollback 근거가 없거나 after evidence가 다른 subject
- scanner/debugger/package manager가 직접 완료 상태를 주장

## 오류와 진단

오류 envelope·exit code는 [오류·진단 계약](errors-and-diagnostics.md)을 따른다. M7 대표 error code는 다음과 같다.

| code | 의미 | 기본 다음 조치 |
|---|---|---|
| `MAINTENANCE_STORE_INCOMPATIBLE` | 공통 Finding/Evidence 계약을 재사용할 수 없음 | migration/compatibility 확인 |
| `REPRODUCTION_INPUT_INCOMPLETE` | command·input·environment·revision 누락 | pack 입력 보완 |
| `REPRODUCTION_EXTERNAL_CONDITION_UNVERIFIED` | 외부 조건 재현 불가 | unverified 기록 후 human review |
| `RECOVERY_PLAN_INVALID` | rollback·roll-forward·restore 단계 모호/불완전 | plan 분리·검증 추가 |
| `SECURITY_DATA_STALE` | 외부 자료 유효 기한 경과 | 승인 후 refresh 또는 stale 상태 유지 |
| `SECURITY_REDACTION_FAILED` | 안전한 report 생성 실패 | artifact 격리·block |
| `DEPENDENCY_INPUT_STALE` | graph·manifest·lockfile 입력 stale | M1/M6 재수집 |
| `DEPENDENCY_MANAGER_UNREGISTERED` | lockfile owner adapter 없음 | Catalog 등록·human review |
| `DEPENDENCY_NETWORK_APPROVAL_REQUIRED` | network/download 승인 없음 | exact approval 대기 |
| `DEPENDENCY_PATCH_APPROVAL_REQUIRED` | apply 승인 없음 | PatchSet review 대기 |
| `DEPENDENCY_UPDATE_REPLAN_REQUIRED` | preview diff가 계획 범위를 바꿈 | M2 재계획 |
| `DEPENDENCY_ROLLBACK_BLOCKED` | before lockfile·validation 근거 부족 | 적용 금지·복구 계획 보완 |

## 저장·retention·감사

권장 logical layout은 [상태·산출물 아키텍처](../architecture/state-and-artifacts.md)가 소유하며 M7은 다음 분리를 요구한다.

- derived document: failure, regression, recovery, dependency, supply-chain, external-data, update-plan, radar
- run artifact: raw tool output, stdout, stderr, trace, dump
- curated reproduction artifact: ReproductionPack manifest와 `reproduction_required` refs
- review artifact: default-safe summary, PatchSet, ReviewPack, release manifest
- audit event: permission, refresh attempt, package manager invocation, PatchApplication, rollback attempt, GateDecision

같은 tool의 결과라도 별도 tool DB를 만들지 않고 producer metadata로 구분한다. 원본 artifact가 retention으로 사라져도 해당 evidence는 `expired|missing`이 되어 더 이상 current completion proof로 사용되지 않는다.

## 구현 순서

문서 확정 뒤 제품 구현은 다음 순서를 지킨다.

1. M7 contract type·Schema·minimal/full/invalid/future fixture와 fingerprint golden
2. failure normalization·family/occurrence fingerprint와 causality DAG pure function
3. ReproductionPack builder·redaction·retention policy
4. DependencySnapshot·ExternalDataSnapshot freshness pure function
5. SupplyChainSnapshot normalization과 common Diagnostic mapping
6. deterministic Maintenance Radar projection·time boundary test
7. read-only CLI inspect/status surface
8. registered runner/scanner/debugger/package manager adapter integration
9. approval-gated isolated dependency PatchSet preparation
10. M3 Gate·ReviewPack·rollback E2E와 Windows path/security corpus

1~6은 fake adapter와 fixture로 먼저 검증한다. 7 이전에는 network·process attach·package change adapter를 만들지 않고, 9 이전에는 dependency source write 경로를 만들지 않는다.

## 8단계 인계

다음 사용자 8단계 [migration·performance·language/platform 정본](migration-performance-and-platform.md)은 최소 다음 exact input을 받는다.

- compatible `FailureRecord`와 minimized `ReproductionPack`
- before failure·after success·재발을 연결한 `RegressionRecord`
- exact command·input·seed·environment·toolchain·manifest·lockfile fingerprint
- rollback·roll-forward·restore가 구분된 `RecoveryPlan`과 rehearsal evidence
- 이전 manifest·lockfile ArtifactRef, dependency PatchSet과 post-apply GateDecision
- SupplyChainSnapshot과 외부 data freshness·coverage·source
- deterministic Radar priority와 unresolved limitation

migration은 rollback·restore 가능성과 data state checkpoint를, performance는 고정 workload·seed·environment와 같은 family identity를 재사용한다. M7 evidence가 stale·partial이면 8단계가 이를 current baseline으로 승격하지 않는다.

## 공식 외부 자료

확인 날짜는 모두 **2026-07-14**다. 이 URL은 adapter 구현을 고정하는 dependency가 아니라 provenance·freshness·lockfile ownership 규칙을 검토한 공식 근거다.

- OSV API: https://google.github.io/osv.dev/api/
- OSV Schema: https://ossf.github.io/osv-schema/
- NVD developer documentation: https://nvd.nist.gov/developers
- SPDX License List: https://spdx.org/licenses/
- GitHub Actions secure use: https://docs.github.com/en/actions/reference/security/secure-use
- GitHub `GITHUB_TOKEN` permissions: https://docs.github.com/en/actions/security-for-github-actions/security-guides/automatic-token-authentication
- Cargo `cargo update`와 `--locked`/`--offline`: https://doc.rust-lang.org/cargo/commands/cargo-update.html
- npm `package-lock.json`: https://docs.npmjs.com/cli/v11/configuring-npm/package-lock-json/
- NuGet dependency locking: https://learn.microsoft.com/en-us/nuget/consume-packages/package-references-in-project-files#locking-dependencies
- Git `bisect`: https://git-scm.com/docs/git-bisect

외부 문서의 version·endpoint·정책은 변할 수 있다. adapter 구현 시 Catalog의 source URL·schema/API version·확인 시각·maximum age를 다시 검증하고, 이 문서의 확인 날짜를 runtime freshness로 사용하지 않는다.

## 완료 조건

- 실패 재현 자료가 일반 log와 artifact role·manifest·retention으로 구분된다.
- 민감 dump·log·secret 후보가 default report에 노출되지 않는다.
- 같은 failure family의 root candidate·cascade·before/after·재발 관계를 추적할 수 있다.
- 재현할 수 없는 외부 조건은 `unverified` 또는 `blocked_external`이다.
- security·license·version 외부 자료에 source·coverage·freshness·valid_until이 있다.
- dependency update의 기본 종료점이 승인 대기 immutable PatchSet이다.
- package manager가 lockfile을 소유하고 core가 resolution을 역산하지 않는다.
- network·download·dependency 추가/변경은 exact 사용자 승인 없이는 실행되지 않는다.
- scanner·debugger·package manager는 adapter이고 M3 core Gate가 완료를 단독 판정한다.
- Radar가 AI 없이 같은 input·evaluation time에서 같은 순서를 만든다.
- 8단계가 사용할 reproduction·rollback·restore·이전 lockfile 근거가 있다.
