# 검증과 개발 보조 기능

상위 범위와 공통 선정 기준은 [구현 대상 기능](README.md)에서 확인한다.

## 3단계 공통 기반

B01~B08은 서로 다른 성공 판정과 증거 형식을 만들지 않는다. 실행 의미, baseline ratchet, suppression, validator guard와 4단계 적용 전·후 Gate는 [3단계 공통 검증·품질 Gate 상세 설계](common-validation-gate.md)가 소유하고, wire field는 [검사·완료·증거 계약](../contracts/validation-and-evidence.md)이 소유한다. 8단계 migration·performance·language/platform도 이 Gate에 phase와 evidence ref를 추가할 뿐 별도 완료 engine을 만들지 않는다.

공통 흐름은 `M2 ready ValidationPlan 검증 -> actual ChangeSet 재수집 -> 등록된 ToolDescriptor 실행 -> Diagnostic 정규화 -> baseline·suppression 평가 -> GateDecision -> EvidenceBundle·ReviewPack`이다. wire 값 `auto_pass|human_review|block`은 화면에서 `AUTO_PASS|HUMAN_REVIEW|BLOCK`으로 표시한다.

- required Check의 `not_run`, `partial`, `unverified`, stale와 flaky는 `pass`가 아니다.
- evidence의 source revision·WorkspaceSnapshot·ValidationPlan·config·Catalog·Check·Tool fingerprint가 current subject와 다르면 자동 통과할 수 없다.
- 기본 ratchet은 기존 부채 전체를 한 번에 막지 않고 `new|worsened`를 차단하며 `existing_unchanged`를 숨기지 않는다.
- active·expired·stale suppression과 false-positive·flaky 판단은 원래 Diagnostic과 실행 결과를 바꾸지 않는다.
- runner는 M2가 고른 Check family·scope를 다시 선택하지 않고 raw shell 대신 등록·신뢰된 ToolDescriptor만 실행한다.
- CLI-only에서 결정적 검사가 끝내지 못한 의미 검토는 Codex·AI 호출 없이 `HUMAN_REVIEW`로 남긴다.

## B01. 실제 변경·범위·주장·증거 검증

Codex의 설명을 그대로 믿지 않고 저장소의 실제 상태와 실행 증거를 기준으로 완료 여부를 판단한다.

- 작업 계약의 허용 범위와 실제 Git diff 비교
- 보고된 변경 파일과 실제 add, modify, delete, rename 비교
- planning-baseline과 Gate 시점 `observed_after_change` ChangeSet을 다시 수집해 preexisting·task-declared·tool-applied 변경 분리
- 4단계 `recipe_preview` ChangeSet·immutable PatchSet operation과 apply 뒤 actual operation manifest를 비교하고 preview 보고만으로 실제 변경을 합성하지 않음
- 요청과 무관한 변경, 빠진 필수 변경, 생성 파일 직접 수정 탐지
- 필수 검사 명령의 실행 여부, revision, 종료 코드와 결과 확인
- "고쳤다", "검사했다", "호환된다" 같은 완료 주장과 근거 연결
- 근거가 없거나 오래됐거나 다른 revision의 결과이면 미확인으로 표시
- 진단을 규칙, 심각도, 확신도, 위치, 근거, fingerprint, 조치로 정규화
- 결과를 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK`으로 구분
- raw ValidationRun outcome과 Gate의 `clean_pass|ratchet_satisfied|unsatisfied|waived_for_review`를 분리
- 변경 요약, 위험, 계약·의존성·테스트 변화, 미확인 사항, 질문을 Review Pack으로 묶음
- Recipe ID·version, selector resolution, transformer·Tool version/hash, idempotence, WorktreeDecision·partial/recovery 상태를 current evidence와 함께 Review Pack에 표시
- 실패 이유와 필요한 수정만 담은 재작업 지시 생성
- 판단과 근거의 출처를 추가 전용 기록으로 보존

## B02. 테스트 신뢰성 검증

테스트가 실행됐다는 사실뿐 아니라 이번 변경을 실제로 증명하는지 확인한다.

- 프로젝트 테스트 목록과 변경에 관련된 테스트 연결
- M2 selected related Check와 `tests` edge·fallback 근거를 보존하고 runner가 임의로 재선택하지 않음
- 테스트 파일 삭제, 사례 삭제, assertion·expected value 약화 탐지
- skip, ignore, only, timeout·retry 증가와 대규모 snapshot 갱신 표시
- 버그 수정은 수정 전 실패와 수정 후 성공을 재현하는 회귀 증거 요구
- before/after evidence의 test identity·input·environment 호환성과 after evidence의 current·complete·stable 상태 확인
- AI가 만든 테스트가 구현을 그대로 복사하거나 같은 잘못된 가정을 공유하는지 검토
- 실패한 seed, 입력, 명령, 환경과 결과를 재실행 가능하게 보존
- 변경 성격과 위험에 맞는 단위·통합·계약·end-to-end 검사 선택
- Recipe preview가 test·fixture·snapshot·harness를 건드리거나 symbol/contract change가 test edge를 만들면 M2 selected related Check를 post-apply exact WorkspaceSnapshot에서 실행
- coverage 수치만으로 통과시키지 않고 중요한 경로와 실패 조건을 함께 확인
- 프로젝트에 필요할 때 property, invariant, differential, metamorphic, fuzz, sanitizer, mutation 검사를 외부 도구로 연결

## B03. 검증기 보호와 회귀 Corpus

검사를 통과시키기 위해 검사 자체를 약하게 바꾸는 일을 별도로 막는다.

- validator, policy, test harness와 CI 검사 경로를 보호 대상으로 등록
- 규칙 삭제, 심각도 하향, allowlist 확대, 필수 명령 제거와 우회 조건 탐지
- pre-change trusted snapshot과 current candidate를 모두 보는 two-snapshot guard로 변경된 validator의 자기 판정만 사용하지 않음
- 검증기 변경에 별도 승인과 실행 context에 맞는 의미 검토 적용. CLI-only에서는 AI 독립 검토를 요구하지 않고 필요한 판단을 `HUMAN_REVIEW`로 남김
- 새 규칙마다 positive, negative, edge, regression fixture와 stable 기대 Diagnostic·GateDecision 요구
- 실제 결함과 공격적 우회 사례를 회귀 Corpus로 축적
- 진단 억제에는 이유, 대상 fingerprint, 만료 시점과 승인자 기록
- 기존 부채는 baseline으로 고정하고 새 악화를 막는 ratchet 적용
- 규칙별 실행 시간, 실제 발견, 거짓 경고, 놓친 결함과 흔들림 측정
- 가능한 판단은 결정적 도구를 우선하고 Codex 평가는 제한된 보조 근거로 사용

## B04. 계약·구조·설정·마이그레이션 검증

프로젝트가 외부나 내부에서 지켜야 할 약속을 등록하고 변경의 파급을 확인한다.

- 공개 API, CLI, 설정, Schema, 직렬화, 파일 형식, 오류 코드와 DB migration 계약 등록
- 기준 계약과 현재 결과 비교, 선언과 실제 구현의 어긋남 표시
- explicit immutable baseline과 exact current snapshot을 비교하고 `unchanged`, `compatible`, `additive`, `breaking`, `unknown`을 kind별 규칙으로 판정
- declared·observed·unresolved consumer를 분리하고 consumer별 `none`, `recommended`, `required`, `blocked_unknown` migration 필요를 기록
- component, layer, 허용·금지 의존, cycle과 공개 경계 규칙 선언
- 구조 위반, 경계 침범, 순환 의존과 의도치 않은 공개 표면 확대 탐지
- package dependency와 금지 import는 current graph·declared policy로 검사하고 text-only candidate를 확정 위반으로 승격하지 않음
- generated source와 원본의 drift, 생성 파일 직접 편집 탐지
- Managed Registry manifest의 duplicate stable ID·public value, namespace collision, owner/type 충돌, alias cycle·window 오류와 removed/reserved ID 재사용 탐지
- Git Registry manifest와 DB ManagedRegistrySnapshot이 다르면 source를 우선하고 Index를 stale로 판정
- declaration definition·reference·Schema·문서·generated output binding과 consumer 최소 지원 version·전환 상태 검증
- hardcoding Finding candidate와 정본·config·Schema drift의 확정 Diagnostic을 구분
- 계약·Schema·migration 변경 시 영향 대상, 호환성, 이행 계획과 승인 요구
- 계약 source·Schema·generated reference·문서·compatibility metadata·필요한 consumer migration guide의 동시 변경을 post Gate에서 요구
- 확인할 수 없는 언어 의미나 동적 경로는 확정하지 않고 미확인으로 표시
- compiler, LSP, Schema validator, contract test와 migration tool 결과를 adapter로 수집
- 데이터·설정·DB 이동은 명시적 version, 단계 사슬, 사본 또는 원자적 교체, rehearsal, invariant, 재개와 복구 증거를 요구
- text·syntax·symbol-aware·codegen rewrite의 보장 수준을 구분하고 text-only candidate를 semantic-safe rewrite로 승격하지 않음
- symbol-aware rewrite는 current·complete definition/reference coverage와 post-apply re-index를, codegen은 authoritative input·generator version/hash·declared output manifest와 replay 재현성을 요구
- raw literal equality만으로 contract·config·error code·managed declaration의 동일 ownership을 만들지 않음
- generated source 직접 편집, scope 밖 codemod output과 unexpected public surface 확대를 PatchSet preview와 actual ChangeSet 양쪽에서 검사

Star-Control이 자체 parser, type checker, DB engine이나 범용 정적 분석기를 만드는 것은 이 기능에 포함하지 않는다.

4단계 자동 수정 뒤 어떤 format·build·test·contract 검사를 실행할지는 Recipe가 직접 command를 저장해 결정하지 않는다. Recipe는 required family floor만 선언하고 M2가 ImpactAnalysis·RiskPath·Profile closure로 exact CheckPlan·scope를 materialize하며 M3가 이를 재선택 없이 pre/post Gate에서 실행한다. 상세 mutation protocol은 [안전한 Patch·Refactor·codemod 엔진 계약](../contracts/safe-patch-and-codemod.md)이 소유한다.

5단계 Registry 변경도 같은 흐름을 사용한다. `active→deprecated→removed` 전이, bounded alias, consumer migration과 tombstone을 검증하고 미전환·unverified consumer가 있으면 removal을 차단한다. error display message만 바뀐 경우 stable code 변경으로 오인하지 않고, 의미가 달라진 code는 새 declaration을 요구한다. exact Rule ID와 판단 표는 [Managed Registry 정본](../contracts/managed-symbol-registry.md)이 소유한다.

6단계 B04는 Registry identity·lifecycle을 복제하지 않고 [계약 호환성·문서·설정·개발 환경 관리](../contracts/contract-compatibility-and-environment.md)의 `ProjectContractManifest`, baseline/current `ContractSurfaceSnapshot`, `CompatibilityReport`를 사용한다. baseline 부재·required coverage 부족·모호한 동적 소비자는 compatible로 간주하지 않는다. evidence 누락은 block, 결정적 도구가 확정할 수 없는 의미 판단은 CLI-only `HUMAN_REVIEW`다. public surface 확대가 binary compatible이어도 `ChangePlan`에 의도가 없으면 별도 blocking Diagnostic으로 유지한다.

8단계 B04는 [Migration·성능·언어·플랫폼 계약](../contracts/migration-performance-and-platform.md)의 `ProjectMigrationManifest`, `MigrationPlan`, `MigrationCheckpoint`, `MigrationAttempt`, `MigrationValidationReport`와 `RestoreVerificationRecord`를 사용한다. 한 Project·한 target의 explicit version source와 연속 chain, dry-run, consistent backup, restore rehearsal, migration rehearsal, exact approval, execute/resume, invariant·consumer validation과 rollback을 독립 phase로 검증한다. `succeeded|partially_succeeded|failed|outcome_unknown|rolled_back|rollback_failed`를 뭉치지 않으며 live partial·outcome unknown·rollback failure를 통과로 만들지 않는다.

backup file과 hash가 있다는 사실은 `integrity_verified`까지만 증명할 수 있다. 별도 사본 환경에서 실제 restore와 structural·behavior Check를 통과한 `RestoreVerificationRecord`가 있어야 `restore_rehearsed|restore_validated`를 주장할 수 있다. unknown field·extension을 보존하지 못하는 step은 destructive로 분류하고 exact 승인 전 실행을 차단한다.

## B05. 보안·의존성·공급망 검증

혼자 개발할 때 놓치기 쉬우면서 사고 비용이 큰 변경을 공통 관문에서 확인한다.

- source, config, 문서, log와 결과물의 secret·token·개인정보 후보 탐지 및 가림
- manifest·lockfile diff와 새 의존성의 목적, 출처, version, license와 위험 확인
- 취약점, license, SAST 결과를 프로젝트 도구에서 수집하고 중복 진단 통합
- auth, session, token, permission, crypto와 위험 API 변경 표시
- GitHub workflow 권한, 외부 action 고정 여부와 실행 조건 검토
- 배포 대상의 file list, digest, manifest와 package dry-run 확인
- 공개 배포가 있는 프로젝트만 SBOM, provenance, 서명과 검증 절차 연결
- 진단 출처와 갱신 시점 기록, 예외는 이유·범위·만료 시점과 함께 관리
- 외부 scanner의 database·Rule·tool version이 stale이면 current 보안 통과 근거로 사용하지 않음

7단계에서는 M1 dependency relation과 M6 `DependencySecurityInputManifest`를 exact revision에 결합한다. 외부 advisory·license·available-version 자료는 source URL, dataset/query, schema/API version, published/modified/fetched 시각, content digest, coverage, tool identity와 `valid_until`을 가진 `ExternalDataSnapshot`으로 기록한다. 자료가 `stale|unknown|unavailable`이면 warning을 만들며 required security Check의 clean/pass 근거로 사용할 수 없다.

manifest·lockfile diff는 dependency 목적, source, requested/resolved version, direct/transitive/internal relation, license와 vulnerability evidence에 연결한다. workflow는 effective permission 확대와 외부 action의 immutable pin 여부를, release는 file list·digest·manifest와 이미 존재하는 SBOM·provenance·signature evidence를 관찰한다. Star-Control이 SBOM이나 서명을 만들었다고 추정하지 않는다.

scanner별 별도 Finding·DB를 만들지 않는다. adapter raw result는 ArtifactRef로 보존하고 공통 Diagnostic·Finding에 정규화하며, 여러 producer의 같은 현상은 evidence를 잃지 않은 correlation으로만 묶는다. Star-Control은 자체 취약점 DB, 보안 scanner, license DB, package registry 또는 공개키 기반 시설을 운영하지 않는다. exact Schema와 freshness 규칙은 [7단계 정본](../contracts/failure-security-and-dependency-maintenance.md)이 소유한다.

## B06. 실패 분석·재현·대상 프로젝트 복구

실패를 다시 만들 수 있는 자료와 수정 후 재발하지 않았다는 증거를 남긴다.

- compile, test, runtime와 운영 실패를 공통 형식으로 정리
- 연쇄 오류 중 첫 원인 후보와 동일 실패 fingerprint 식별
- revision, 환경, 명령, 입력, seed, stdout·stderr와 관련 artifact 묶음
- 최소 재현 절차와 재현 가능 여부 검사
- rerun, 입력 축소, Git bisect와 기존 debugger·trace 도구를 adapter로 연결
- 알려진 실패와 임시 회피책에 근거와 만료 조건 기록
- 수정 전 실패와 수정 후 성공, 관련 회귀 검사 연결
- 같은 failure/test identity·input·environment가 아니거나 after 결과가 flaky이면 회귀 성공으로 표시하지 않음
- rollback, roll-forward, restore 순서와 사전 rehearsal 증거
- 민감한 dump·log의 가림, 접근 범위와 보존 기간 관리

7단계 failure identity는 revision을 넘어 재발을 묶는 `family_fingerprint`와 exact revision·structured args·input·seed·environment·tool을 묶는 `occurrence_fingerprint`를 분리한다. 첫 원인은 확정값이 아니라 evidence와 confidence가 있는 `root_candidate`이며, 연쇄 오류는 cycle 없는 causality edge로 연결한다.

일반 log는 한 run의 시간순 출력이고 `ReproductionPack`은 재현에 필요한 최소 manifest다. pack은 exact subject, registered command와 structured args, logical cwd, environment fingerprint, input·seed, expected/actual result, attempt, redacted stdout·stderr와 artifact ref, 외부 조건, minimization과 conclusion만 선별한다. `quarantined|unknown` artifact는 default report에서 제외한다. 재현할 수 없는 service·device·clock·network 조건은 `blocked_external` 또는 `unverified`이며 해결로 취급하지 않는다.

수정 전 verified failure와 호환 가능한 수정 후 complete·stable pass를 `RegressionRecord`로 연결하고, 이후 같은 family가 호환 scope에서 다시 나타날 때만 `regressed`로 판정한다. rollback은 변경 전 상태 복귀, roll-forward는 전진 correction·migration, restore는 backup·snapshot 복구이며 하나의 성공으로 뭉치지 않는다.

여기서는 작업 대상 프로젝트의 실패를 다룬다. Star-Control 자신의 중단 복구는 A07이 담당한다. 자체 debugger, dump analyzer나 tracing backend는 만들지 않는다. rerun·reducer·bisect·debugger·trace는 등록 adapter이며 최종 완료는 M3 core Gate만 판정한다. 세부 계약은 [7단계 정본](../contracts/failure-security-and-dependency-maintenance.md)이 소유한다.

8단계 migration 실패는 M7 failure identity·ReproductionPack·RecoveryPlan을 재사용하되 step checkpoint와 target version을 추가로 결합한다. resume 전 actual target이 checkpoint before/expected-after 중 어디와 일치하는지 재관찰하고, 어느 쪽도 아니거나 non-replay-safe step의 commit 여부가 불명확하면 `outcome_unknown`으로 차단한다. rollback, roll-forward와 restore는 각각 새 attempt·post-recovery invariant·Gate가 있어야 하며 이전 active state로 보인다는 이유만으로 성공을 합성하지 않는다. exact 상태·reconcile 표는 [8단계 정본](../contracts/migration-performance-and-platform.md#partial-migration-재시작과-resume)이 소유한다.

## B05·B06 공통 유지보수 Radar

`MaintenanceRadarSnapshot`은 새 진단 저장소가 아니라 공통 Finding·Diagnostic·Suppression, DependencySnapshot, RegressionRecord와 contract·docs·environment drift를 결합한 파생 view다.

- 재발 실패와 verified regression
- 만료·stale suppression
- outdated 또는 freshness가 stale/unknown인 dependency
- unresolved security finding
- flaky required test
- 문서·config·environment drift
- rollback·restore 근거가 없는 high-risk 변경

정렬은 `blocking/protected risk → severity/risk → freshness → regression/recurrence → evidence completeness → due/age → stable ID`의 결정적 tuple을 사용한다. optional AI는 설명만 만들 수 있고 priority, GateDecision과 approval state를 바꾸지 못한다. Radar의 `valid_until`은 suppression expiry, 외부 자료, Project/Code Index와 Gate time boundary 중 가장 이른 값이며, 경계를 넘으면 stale로 재평가한다.

## B07. 문서·설정·개발 환경 일치 검증

새 컴퓨터나 깨끗한 환경에서도 같은 작업을 재현할 수 있도록 코드 밖의 개발 계약을 확인한다.

- README, 운영 문서, 설정 예시와 정본 문서를 Documentation Registry에 등록
- 문서의 명령, code snippet, 링크, anchor와 config example 실행·존재 검사
- 문서 command text를 raw shell로 실행하지 않고 typed candidate가 등록된 ToolDescriptor와 exact match할 때만 실행
- CLI·Schema·생성 문서와 실제 동작의 drift 탐지
- ManagedDeclaration의 documentation·Schema·language binding과 generated output provenance drift를 `RegistryConsistencyRecord`로 정규화
- deprecated/removed ID의 문서 snippet·config example·consumer reference와 alias window 만료 탐지
- config key, 기본값, 필수 환경 변수, secret, local override 경계 확인
- config의 `declared` 존재 여부, M5 lifecycle `active→deprecated→removed`와 `documented`, `read`, `overridden` 관찰을 분리해 `ConfigKeyTrace`로 연결
- complete semantic reader coverage에서만 사용되지 않는 key를 확정하고, 문서 없는 환경 변수는 이름·presence만 진단하며 값을 수집하지 않음
- toolchain, package manager, lockfile와 프로젝트 task 명령 발견
- 처음 설치 절차, project doctor와 누락 도구 진단
- project doctor는 등록된 read-only probe만 사용하고 network download, package 설치·update, source/config write와 시스템 설정 변경을 수행하지 않음
- line ending, encoding, 대소문자, drive·UNC·junction, 경로 길이와 Windows·CI 차이를 redacted environment fingerprint로 표시
- clean-room 명세·readiness와 실제 disposable 환경 검사를 분리하고, 누락 도구를 자동 설치하지 않은 채 재현성 확인
- 문서가 가리키는 file·command·version·지원 platform/environment를 explicit `AssumptionSpec`과 current observation으로 비교
- 필요한 프로젝트만 reproducible build 여부를 별도 검사

Star-Control이 package manager, container runtime 또는 언어 version manager를 대신 만들지는 않는다.

6단계 drift 검사는 Managed Registry source와 current M1 Index를 입력으로 사용한다. DB row를 기대값으로 삼지 않고, `stale_registry_index`, `missing_binding`, `value_mismatch`, `type_mismatch`, `symbol_name_mismatch`, `consumer_transition_incomplete`, `generated_output_stale`, `docs_schema_drift` 같은 stable 상태로 source·관찰·호환 판정을 분리한다. command는 typed candidate와 registered descriptor가 exact match할 때만 safe probe를 실행하며, snippet은 language·wrapper·expected result가 선언된 경우에만 검증한다. 세부 판정표, doctor 금지 동작, `CleanRoomSpecification`과 후속 dependency·security 입력은 [6단계 정본](../contracts/contract-compatibility-and-environment.md)이 소유한다.

## B08. 성능·자원·빌드 효율 검증

프로젝트가 중요하다고 선언한 경로만 안정된 조건에서 비교한다.

- 사용자 또는 reviewed `PerformanceWorkloadSpec`이 중요하다고 선언한 사용자 체감 경로·개발자 build 경로만 활성화
- workload ID/version, structured benchmark 명령, input manifest·seed와 tool version/hash 보존
- baseline/candidate cohort별 exact ProjectRevision·WorkspaceSnapshot·EffectiveConfig·Catalog·environment fingerprint 고정
- 같은 workload·input·tool·environment·build/cache mode를 요구하고, revision 차이는 의도된 ChangeSet/PatchSet으로만 제한
- warmup과 measured attempt를 분리하고 minimum 3회, 기본 5회 measured run의 raw result를 모두 보존
- noise threshold, 추가 실행 상한과 outlier detector를 첫 measured run 전에 고정
- outlier sample을 삭제하지 않고 포함/제외 통계를 모두 보고하며 minimum sample 미달·high noise는 `inconclusive`
- wall/CPU time, 명시된 memory metric, artifact size와 throughput을 numeric value·unit·collector provenance가 있을 때만 기록
- clean, incremental, cache hit·miss build를 별도 comparison item으로 유지하고 한 mode의 개선으로 다른 mode 악화를 상쇄하지 않음
- profiler와 build analyzer는 registered external adapter로 연결하고 hotspot을 causal proof나 GateDecision으로 승격하지 않음
- 최적화 뒤 M3 correctness·contract·test Gate와 memory·artifact size·maintainability trade-off 재검사
- 수치, unit 또는 comparable cohort가 없으면 0·이전 값·추정치를 채우지 않고 `no_measurement|not_comparable|inconclusive`로 종료

모든 작업에 강제하지 않는다. 중요 경로나 반복 병목 선언이 없으면 `not_declared|not_applicable`이며 임의 benchmark를 만들지 않는다. 자체 profiler, benchmark engine, build analyzer나 build cache를 만들지 않으며 측정·비교·Gate 상세는 [8단계 정본](../contracts/migration-performance-and-platform.md#성능build-측정-계약)이 소유한다.

## B09. CI·Release·배포 준비 검증

로컬에서 검증한 대상을 같은 식별자로 CI와 배포 단계까지 추적한다. 실행·상태·평가의 상세 정본은 [10단계 CI·Release·평가 계약](../contracts/ci-release-evaluation-and-product-completion.md)이 소유한다.

검사 계층은 다음 네 단계다.

| 계층 | 목적 | release evidence 여부 |
|---|---|---|
| `local_quick` | 편집 중 format·link·Schema와 직접 affected Check의 빠른 feedback | 직접 사용 불가 |
| `target` | M2가 선택한 affected test·build·contract·security Check | full 승격 입력 |
| `full` | 깨끗한 Windows에서 전체 build·test·lint·docs·validator guard | release 전 필수 |
| `release` | 봉인된 artifact의 package·metadata·install/update/rollback/uninstall·publish preflight | `ready` 직접 근거 |

모든 계층은 같은 Task ID, project별 source revision, config fingerprint, Catalog, logical Tool ID/version/descriptor set과 resolved Profile fingerprint를 사용한다. architecture별 ToolRegistrySnapshot과 executable hash는 declared platform artifact로 따로 기록하되 logical version은 같아야 한다. branch·workflow·CI run 이름은 이 identity를 대신하지 않는다. 허용한 architecture delta 밖의 입력 하나라도 달라지면 이전 결과는 stale이고 새 candidate를 만든다.

B09의 필수 불변식은 다음과 같다.

- 깨끗한 Windows 11 24H2 build 26100 이상에서 x64·ARM64 build·test·package evidence 생성
- ARM64 지원은 cross-build만이 아니라 native runtime·install lifecycle evidence 필요
- source revision과 final artifact SHA-256·artifact set digest 연결
- 한 번 build·package해 봉인한 artifact byte를 검증한 뒤 같은 digest로 승격; release 재build 금지
- version, changelog, package metadata, license·third-party notice, 실제 포함 file list와 package dry-run 확인
- SBOM·provenance·signing을 release policy별 `required|not_required|unavailable|incomplete|complete`로 판정
- install, `safe_default` first run, update, rollback, uninstall과 user data 보존 확인
- data·API·config·state compatibility와 migration·restore·rollback 순서 관문
- `ready`, `approved`, `published` 상태 분리와 exact approval staleness 확인
- publish·deploy·withdrawal·원격 변경·유료 행동의 action별 명시적 승인
- adapter receipt 뒤 provider after snapshot에서 exact version·source·artifact digest 확인
- smoke·관찰 시간·rollback trigger를 publish 전에 versioned policy로 고정

`ready`는 공개됐다는 뜻이 아니고, `approved`는 원격 effect가 성공했다는 뜻이 아니다. 실제 publication 결과를 확인하지 못하면 `publish_outcome_unknown`이며 `published`로 표시하지 않는다. deploy target은 별도 `remote_actions[]`의 `verified|outcome_unknown|rollback_required`를 사용하고 top-level publication 상태를 되감지 않는다.

GitHub Actions, package registry, installer, signing service와 cloud CLI는 registered adapter로 결과·artifact·receipt만 반환한다. Star-Control은 자체 CI/CD runner, build system, installer, signing/PKI, artifact registry 또는 배포 서비스를 만들지 않는다.
