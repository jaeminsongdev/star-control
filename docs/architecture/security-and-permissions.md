# 승인·권한·안전

## 기본 생각

승인 질문은 많을수록 안전한 것이 아니다. 너무 자주 묻으면 사용자가 내용을 읽지 않게 된다.

Star-Control은 모든 동작의 승인 여부를 설정할 수 있게 하고, 공개 배포용 안전 기본값과 개인용 자동 기본값을 분리한다.

## 기본 프로필

### safe_default

공개 배포의 기본값이다.

- 읽기와 일반 조사는 자동
- 승인된 계획 안의 파일 수정은 자동
- 파일 삭제와 대량 이동은 질문
- 새 도구와 프로그램 추가는 질문
- 시스템 설정 변경은 질문
- 원격 업로드, 검토 요청, 병합은 질문
- 외부 계정 변경은 질문
- 유료 동작은 질문

### personal_auto

사용자가 원하는 개인 기본값이다.

- 유료 동작만 반드시 질문
- 나머지는 승인된 계획과 설정 범위 안에서 자동
- 되돌릴 수 있는 기록과 검사 의무는 유지
- Codex 또는 관리자가 요구하는 승인은 그대로 유지
- 9단계 ChangeBundle의 remote upload·PR 생성/수정·remote merge·publish는 예외로 action별 현재 사용자 승인을 요구

### custom

행동 종류별로 자동, 질문, 금지를 직접 설정한다.

## 판단할 행동 종류

- 파일 읽기
- 파일 수정
- 파일 삭제
- 파일 대량 이동
- 명령 실행
- 새 프로그램 또는 의존 항목 추가
- 인터넷 접근과 다운로드
- 컴퓨터 설정 변경
- 로컬 변경 기록 생성
- 원격 저장소 업로드
- 검토 요청 생성과 수정
- 병합
- 배포와 공개
- 외부 계정 수정
- 유료 서비스 사용

## 유료 동작

다음 중 하나면 유료 가능성이 있는 것으로 본다.

- 도구나 서비스가 유료임을 명시함
- 사용량에 따라 요금이 발생함
- 유료 계정 자원을 생성하거나 변경함
- 비용 여부를 확실히 알 수 없음

비용을 확실히 알 수 없으면 실행 전에 사용자에게 묻는다.

## 계획과 권한의 관계

승인이 없다는 것은 무엇이든 해도 된다는 뜻이 아니다.

- 활성 목표와 단계가 있어야 한다.
- 단계 목적과 관련된 동작이어야 한다.
- 사용자가 금지한 경로를 지켜야 한다.
- 실행 전후 기록을 남겨야 한다.
- 실패를 숨기거나 검사를 약화하면 안 된다.
- Star-Control은 Codex의 더 강한 제한을 약화할 수 없다.

## 외부 개발 도구 신뢰

ToolPackageManifest 등록과 매 실행 승인은 다른 경계다.

- TOML이 EXE를 가리킨다는 사실만으로 검증·권한·비용 판단을 건너뛰지 않는다.
- manifest, Schema, update policy, 허용 path와 executable identity를 한 묶음으로 trust한다.
- `safe_default`는 새 user package·path를 처음 쓸 때 확인하고, `personal_auto`는 사용자가 관리 root에 직접 저장한 valid user manifest를 등록 의도로 볼 수 있다.
- project manifest는 `personal_auto`에서도 사용자가 한 번 명시적으로 trust해야 하며 `pinned_hash`만 허용한다.
- user manifest의 `version_compatible`은 valid Authenticode subject와 probe·interface·product version 범위를 모두 통과한 EXE만 자동 반영한다.
- user manifest의 `follow_path`는 허용 path의 현재 EXE를 쓸 수 있지만 매 실행 identity·hash·version을 기록하며 path 범위와 permission은 넓히지 않는다.
- trust한 뒤의 반복 실행은 ToolDescriptor의 Permission ActionId와 현재 정책 Profile을 따른다.
- manifest, Schema, permission 분류, protocol 또는 허용 path가 바뀌면 기존 trust를 재사용하지 않는다. executable byte 변경은 선택한 update policy에 따라 거부하거나 호환 검증한다.
- describe가 정한 risk lane보다 낮은 MCP lane으로 호출하면 side effect 전에 거부한다.
- TOML이 자체적으로 `trusted=true`, `approval=auto`를 주장하는 field는 인정하지 않는다.

상세 형식은 [외부 Tool Registry 계약](../contracts/external-tool-registry.md)이 소유한다.

ToolPackageManifest의 read·write·network 표시는 임의 EXE의 실제 행동을 스스로 제한하지 못한다. Job Object는 timeout·resource·process tree를 관리하지만 보안 sandbox가 아니다. 일반 CLI의 `trusted_desktop`은 현재 사용자 권한으로 실행할 local code를 신뢰하는 결정이다. OS 수준 접근 제한은 materialized artifact만 다루는 호환 `appcontainer_adapter`에서만 주장한다. `restricted_token` profile은 실제 project path·network 제한을 과장할 수 있어 지원하지 않으며, Codex sandbox가 Controller child에도 자동 적용된다고 간주하지 않는다.

## 범위가 늘어났을 때

예상하지 못한 파일을 건드렸다는 이유만으로 즉시 중단하지 않는다.

1. 현재 단계에 꼭 필요한 변경인지 판단한다.
2. 안전하고 같은 성격의 작업이면 범위를 넓히고 이유를 기록한다.
3. 다른 모델, 다른 권한, 유료 동작, 큰 위험이 필요하면 새 단계로 분리한다.
4. 사용자 파일인지 판단할 수 없거나 안전하게 진행할 수 없으면 중단한다.

safe_default는 의심스러운 범위 확대를 더 자주 질문하고, personal_auto는 기록 후 진행을 우선한다.

## 작업 시작 전 기존 변경

- 현재 변경 상태를 기준선으로 저장한다.
- 사용자가 이미 바꾼 파일을 Star-Control 변경으로 오인하지 않는다.
- 같은 파일을 수정해야 하면 기존 내용을 보존한 상태에서 작업한다.
- 자동으로 되돌리거나 덮어쓰지 않는다.
- 충돌 가능성이 크면 새 작업 복사본을 사용하거나 중단한다.

## 4단계 Patch·codemod source mutation

[4단계 안전한 Patch·Refactor·codemod 엔진](../contracts/safe-patch-and-codemod.md)은 일반적인 “승인된 계획 안의 파일 수정”보다 더 강한 mutation protocol을 사용한다.

1. `change prepare`는 target source·Git metadata를 바꾸지 않는 dry-run이며 PatchSet·diff·영향·검사·rollback을 먼저 표시한다.
2. `patch apply`는 별도 command다. Recipe를 선택했다는 사실이나 prepare 성공을 apply 승인으로 해석하지 않는다.
3. exact PatchSet fingerprint, Project·Checkout, base revision·dirty manifest, action set, worktree strategy와 approval expiry를 하나의 scope로 묶는다.
4. source·plan·Recipe·config·Catalog·Index·Tool·approval binding 중 하나가 달라지면 기존 pre Gate와 승인은 stale이다.
5. source-write port는 `patch_pre_apply` Gate와 single-use in-memory permit 뒤에만 열린다. persisted approval·Gate ID만으로 직접 열지 않는다.
6. apply 뒤 실제 WorkspaceSnapshot·ChangeSet과 M2 selected Check를 `patch_post_apply`에서 검증한다.

행동별 최소 permission은 다음과 같다.

| 행동 | permission 경계 |
|---|---|
| Recipe 조회·selector resolution | `local_read` |
| materialized preview | Controller-owned preview root의 `local_write`; target effect 없음 |
| external codemod preview | `process_run`, 필요 시 isolated worktree `local_write`, trusted ToolDescriptor |
| isolated Git worktree 생성 | exact Project/base의 `local_write`, `process_run`, `plan_execute` |
| Patch apply | `local_write`와 operation별 `local_delete\|local_mass_move\|dependency_change` 등 |
| reverse PatchSet | forward와 동등한 action + 새 current precondition |
| isolated worktree 폐기 | Star-Control owned exact root에 대한 `local_delete` |

external mutating codemod·formatter·generator는 live target checkout에서 실행하지 않는다. target root absolute path를 input·cwd·environment로 전달하지 않고 exact base의 격리 preview worktree에서만 실행한다. Tool manifest가 read/write scope를 선언해도 arbitrary EXE를 filesystem sandbox에 가둔다는 뜻은 아니므로 local code trust와 격리 root 제한을 함께 요구한다.

`safe_default`와 `personal_auto`는 추가 prompt 여부가 다를 수 있지만 다음을 자동 완화하지 못한다.

- raw literal-only global replacement 금지
- current/partial dirty overlap의 overwrite 금지
- Recipe replay idempotence와 PatchSet fingerprint 확인
- pre/post Gate, forward/reverse artifact와 operation receipt
- timeout·cancel·malformed output·outcome unknown의 fail-closed 처리
- single Project·single Checkout write 경계

partial apply나 Gate 실패는 primary checkout 삭제·`git reset --hard`·`checkout`의 권한이 아니다. rollback은 exact reverse PatchSet 또는 Star-Control이 소유한 isolated worktree의 승인된 폐기이며, reverse precondition이 깨졌으면 byte를 덮지 않고 recovery review로 남긴다.

## 7단계 재현·보안·dependency 경계

[7단계 실패 재현·보안·의존성 유지보수](../contracts/failure-security-and-dependency-maintenance.md)는 `personal_auto`에서도 다음 action을 자동 승인하지 않는다.

| 행동 | 최소 경계 | 승인 범위 |
|---|---|---|
| advisory·license·available-version refresh | `network_read=prompt` | provider/source, query scope, credential·비용, expiry |
| package·tool·외부 artifact download | `network_download=prompt` | URL/provider, expected digest/size, 목적 |
| dependency add/remove/update·lockfile 생성 | `dependency_change=prompt` | Project, package/candidate, manager operation, 예상 file scope |
| debugger attach·process control | `process_run=prompt` | process identity, duration, adapter, effect |
| memory/core dump·민감 trace capture | `secret_access=prompt` + redact-before-persist | artifact kind, bounded staging, retention; 안전한 redaction 불가 시 bytes drop |
| dependency PatchSet apply | exact PatchSet prompt | Project·Checkout, base, PatchSet hash, before lockfile, rollback |

Profile과 ToolDescriptor는 필요한 action을 선언할 뿐 권한을 부여하지 않는다. 승인 전 offline/current cached input만 분석할 수 있으며, 자료가 stale하면 stale/unknown을 보고한다. scanner·debugger·package manager가 success를 반환해도 M3 core Gate만 완료를 판정한다.

### adapter 신뢰와 효과

- scanner adapter는 raw report와 공통 Diagnostic mapping을 제공하며 vulnerability DB를 Star-Control DB로 복제하지 않는다.
- debugger·trace adapter는 registered structured invocation만 받고 target process·dump path·timeout을 exact scope로 묶는다.
- package manager adapter는 manifest·lockfile을 소유한다. core와 codemod가 resolved entry를 직접 편집하거나 version closure를 역산하지 않는다.
- 외부 자료 adapter는 source/query/schema, published/modified/fetched 시각, coverage, digest와 maximum age를 제공한다.
- declared effect와 실제 write/network/cache/process effect가 다르면 즉시 중단하고 outcome을 unknown으로 보존한다.

### 민감 재현 자료

ReproductionPack은 일반 log directory의 별칭이 아니라 최소 manifest다. raw stdout·stderr·dump·trace는 redacted ArtifactRef로만 연결한다. secret·token·개인정보·username·home/temp path를 fingerprint에 넣지 않으며, 확인된 secret·PII를 안전하게 가릴 수 없는 bytes는 `dropped_sensitive`다. `quarantined`는 bounded redaction staging 또는 정책상 non-secret 민감 artifact에만 사용한다.

`quarantined|unknown` artifact는 CLI 기본 출력, MCP 응답, ReviewPack과 update dashboard에 포함하지 않는다. unresolved failure·security finding이 retention hold여도 raw sensitive bytes의 보존을 자동 연장하지 않는다. 접근·보관·삭제는 각각 audit event와 필요한 permission을 가진다.

### 공급망 권한

Star-Control은 자체 scanner, vulnerability/license DB, package registry, SBOM signer, certificate authority와 private key store를 만들지 않는다. release file list·digest·manifest·SBOM·provenance·signature가 이미 있으면 관찰·검증 evidence로 연결할 뿐 새 서명이나 공개를 수행하지 않는다.

외부 action의 immutable pin과 workflow permission은 provider adapter가 정규화한다. GitHub Actions의 full commit digest 같은 provider별 immutable identity를 다른 provider에 임의 적용하지 않으며, mutable ref와 permission 확대는 ReviewPack에 명시한다.

## 8단계 migration·성능·언어 전환 경계

[8단계 migration·performance·language/platform 정본](../contracts/migration-performance-and-platform.md)은 계획과 실행을 분리한다. `inspect|plan|status|compare`는 source/target read-only이며, `dry-run`도 live target을 쓰지 않는다. Profile activation이나 MigrationPlan 자체는 permission이 아니다.

| 행동 | 최소 경계 | 승인 binding |
|---|---|---|
| version·chain·workload·behavior baseline 탐색 | `local_read` | Project, source kind, bounded path, revision |
| backup 사본 생성 | `local_write`; 외부 도구면 `process_run` | exact source fingerprint, destination, tool identity, retention |
| restore/migration rehearsal | Star-Control-owned copy root의 `local_write`; 필요 시 `process_run` | copy root, plan fingerprint, tool, expiry |
| live migration `execute\|resume` | `local_write`, target별 effect action, M3 pre Gate | attempt, exact step range, before fingerprint, activation strategy, expiry |
| destructive migration | `local_write`와 해당 `local_delete\|local_mass_move`; 항상 prompt | 삭제/손실 field·row·artifact 목록, backup/restore evidence, rollback 한계 |
| rollback·restore | forward와 동등한 write/delete action, M3 rollback Gate | failed attempt, recovery mode, restore point, current precondition |
| performance workload 실행 | `process_run`; 산출물 생성 시 bounded `local_write` | workload, command/tool, revision pair, environment, repetition/resource limit |
| profiler·build analyzer attach | registered external ToolDescriptor의 `process_run`; 민감 trace면 `secret_access` | process/build identity, duration, output/redaction/retention |
| language codegen·codemod preview/apply | M4 Patch·Recipe와 동일 | Recipe/PatchSet hash, source/consumer scope, pre/post Gate |
| consumer·writer cutover | exact `local_write`, M6 compatibility와 M3 cutover Gate | boundary, consumer set, compatibility window, rollback trigger |
| remote CI/지원 OS 실행 | `network_read\|network_upload` 등 provider별 action | provider, revision, OS/arch image, credential/cost, artifact policy |

다음 규칙은 `personal_auto`에서도 완화하지 않는다.

- unknown field 손실, irreversible step, downgrade 불가, live DB/table drop과 source-of-truth 전환은 destructive이며 exact prompt가 필요하다.
- backup file이 있다는 사실은 restore permission이나 복구 가능성의 증거가 아니다. restore rehearsal/validation 결과를 별도로 결합한다.
- timeout·crash 뒤 `outcome_unknown`이면 새 attempt를 자동 시작하지 않고 checkpoint·target을 read-only reconcile한다.
- approval은 plan/attempt/subject/version/step/tool/environment와 ValidationPlan·GatePolicy fingerprint에 결합하며 하나라도 바뀌면 stale이다. 실제 pre Gate 뒤 single-use in-memory permit이 이 approval과 GateDecision fingerprint를 함께 결합한다.
- performance 수집 승인으로 profiler의 임의 process attach, 전체 filesystem 수집 또는 raw sensitive trace 보존을 허용하지 않는다.
- compile 성공이나 target artifact 생성으로 consumer cutover를 승인하지 않는다. 기능 동등성·compatibility·실제 지원 플랫폼 Gate가 별도다.
- 둘 이상의 Project write는 M8에서 실행하지 않는다. `CrossProjectMigrationHandoff`까지만 만들고 9단계 ChangeBundle이 participant별 승인·적용·보상을 소유한다.

사본 환경은 Star-Control-owned bounded root만 사용하고 원본과 별도 identity/fingerprint를 가진다. 원자적 교체는 target adapter가 같은 filesystem·transaction·rename semantics를 증명한 경우에만 선언한다. capability가 없으면 side-by-side 또는 명시적 in-place 전략으로 낮추며, 용어만 `atomic`으로 바꾸지 않는다.

## 9단계 CrossRepo ChangeBundle 경계

[9단계 정본](../contracts/cross-repo-change-bundle.md)은 project별 M4/M8 effect·Gate를 조정하지만 여러 repository를 하나의 permission이나 transaction으로 묶지 않는다.

### local Git·worktree

| 행동 | 최소 경계 | 승인 binding |
|---|---|---|
| bundle plan·status·overlap·remote-free preflight | `local_read` | MultiProjectGoal·ProjectId set·revision·bounded scope |
| participant/integration worktree 생성 | `local_write`, `process_run`, `plan_execute` | ProjectId·repository fingerprint·base commit·role·owned root |
| project Patch/migration apply | M4/M8 action과 동일 | participant·PatchSet/plan·base·dirty·pre Gate·worktree |
| local commit 생성 | `git_commit` | ProjectId·worktree·actual ChangeSet·message metadata·parent |
| integration branch merge/update | `git_merge` | MergePlan·queue entry·target base·strategy·post Gate |
| conflict resolution | 새 M4 PatchSet permission | 양쪽 intent·contract·current conflict subject |
| worktree/branch 정리 | `local_delete` | exact ownership·registration·current manifest·evidence hold |

사용자 primary checkout·dirty byte·untracked file·branch는 자동 stash·reset·clean·checkout·강제 이동하지 않는다. target branch가 사용자 checkout에 열려 있으면 Star-owned integration branch에서 결과를 유지하고 user branch update를 별도 action으로 요청한다.

PatchSet·base·dirty manifest·MergePlan·target tip·permission fingerprint가 달라지면 승인은 stale이다. `git_commit` 승인은 merge나 branch update를, `git_merge` 승인은 remote push를 허용하지 않는다.

### remote Git·PR·merge·publish

remote read는 `network_read`와 필요한 `secret_access` 정책을 따르며 adapter가 redacted `RemoteStateSnapshot`을 만든다. capability observation은 permission이 아니다.

| 행동 | 최소 경계 | 승인 binding |
|---|---|---|
| remote push/upload | `git_push` + `external_write` + 명시적 ApprovalRequest | ProjectId, remote identity, local commit, target ref, before remote OID |
| PR 생성·수정·닫기 | `pull_request` + `external_write` + 명시적 ApprovalRequest | ProjectId, head/base commit, PR target, body artifact hash |
| remote PR merge·protected ref update | `external_write` + 명시적 ApprovalRequest | PR ID, head/base/expected merge commit, required check snapshot |
| release publish·deploy | `release_publish` + 명시적 ApprovalRequest | release manifest, project revisions, artifact hash, channel |

위 네 종류는 `safe_default`와 `personal_auto` 모두에서 current bundle action별 `ApprovalRequest decision=approved`를 요구한다. `RemoteWriteScope`는 허용 가능한 host·repository·action 범위를 좁힐 뿐 승인으로 쓰지 않는다. push 승인을 PR·merge·publish로 넓히거나 한 Project 승인을 다른 Project에 재사용하지 않는다.

remote effect 직전 before snapshot이 stale·partial·unverified이면 실행하지 않는다. adapter response 뒤 exact target을 after snapshot으로 다시 관찰하고, 확인하지 못하면 `outcome_unknown`으로 보존한다. timeout·connection loss에서 자동 retry하거나 local state로 remote 결과를 추측하지 않는다.

force push, history rewrite, protected-branch bypass와 account/permission 변경은 9단계 기본 `deny`다. rollback도 force update가 아니라 새 revert PR/merge 또는 provider가 지원하는 승인된 withdrawal operation이다.

### 부분 성공과 compensation

- 한 participant 성공 뒤 다른 participant가 실패해도 성공한 effect를 숨기거나 자동 rollback하지 않는다.
- `resume_remaining|roll_forward|compensate|hold|abandon_partial`은 각각 새 current precondition·PermissionPlan·evidence를 가진다.
- reverse PatchSet, revert commit, remote revert PR와 publish withdrawal은 original action의 승인을 재사용하지 않는다.
- `outcome_unknown` participant와 그 downstream은 read-only reconcile 전 새 effect를 시작하지 않는다.
- held worktree·backup·remote evidence는 ownership·retention·permission 확인 전 삭제하지 않는다.

CLI-only에서도 위 경계가 동일하다. Codex가 conflict 해결이나 PatchSet을 제안해도 Git·remote action을 직접 실행하거나 사용자의 승인을 대리하지 않는다.

## 10단계 Release·평가 권한 경계

[10단계 정본](../contracts/ci-release-evaluation-and-product-completion.md)은 release readiness, 외부 승인과 실제 원격 결과를 분리한다.

| 행동 | 최소 action·permission | 승인 binding |
|---|---|---|
| local quick·target Check | `process_run`, 필요 시 local artifact write | Task·source·ValidationPlan·Tool·scope |
| clean full/release CI 요청 | `network_read` 또는 provider별 external action, 유료면 `paid_action` | source revision·environment·workflow·budget·artifact policy |
| local package dry-run | bounded local artifact write | ReleaseManifest draft·package policy·staging root |
| disposable install/update/rollback/uninstall test | `local_write`, `process_run`, cleanup이면 `local_delete` | final artifact digest·owned sandbox root·state fixture·cleanup manifest |
| signing | `secret_access`, `external_write`, 필요 시 `paid_action` | final unsigned digest·signer identity·certificate policy·output destination |
| publish·release 생성 | `release_publish` + `external_write` | exact manifest revision·artifact set digest·channel·provider·destination |
| deploy | `external_write` + target별 명시적 action | published artifact digest·deploy target·before revision·smoke/rollback policy |
| withdrawal·remote rollback | 새 `external_write` 승인 | published/deployed subject·target·compensation·expected after-state |
| uninstall user-data purge | destructive `local_delete` | exact owned path classes·backup/export·expected byte·retention hold |

`safe_default`와 `personal_auto` 모두 publish·deploy·withdrawal·remote rollback·account change에는 현재 action의 명시적 ApprovalRequest를 요구한다. paid CI·signing도 비용과 provider 근거가 있을 때 별도 승인한다. `ready`, `RemoteWriteScope`, CLI `--yes`, 이전 release 승인과 provider capability는 승인으로 쓰지 않는다.

ApprovalRequest는 manifest revision, artifact set digest, version, channel, provider/destination, expiry, before snapshot, smoke·rollback policy와 permission fingerprint에 결합한다. byte·source·config·Tool·Profile·policy·target 하나라도 바뀌면 stale이다. `approved`는 effect 전 상태이며 adapter receipt만으로 `published`가 되지 않는다.

signing private key·token·certificate secret은 Star-Control config·management DB·log·artifact·ReleaseManifest에 저장하지 않는다. 외부 signer adapter가 반환한 signature·certificate chain observation과 redacted receipt를 Controller가 검증한다. signing이 byte를 바꾸면 signed output은 새 final candidate이고 이전 unsigned release Gate는 current가 아니다.

EvaluationRun `offline|replay|shadow`는 실제 route·Check·permission·source·release를 바꾸지 않는다. `trial`의 bounded opt-in은 exact Project/user scope·기간·fallback·stop trigger를 별도 승인하며, recommendation만으로 Catalog·Rule·Profile·Recipe를 자동 수정하지 않는다. validator guard·Corpus·required Check·severity·ratchet·suppression floor는 평가 candidate가 완화할 수 없는 보호 경계다.

install/update/rollback/uninstall 중에는 user config·management state·Project source·`.ai-runs`와 recovery artifact를 기본 보존한다. purge, backup 폐기와 ownership이 불명확한 cleanup은 release rollback이나 uninstall 승인에 포함되지 않는다.

## 11단계 Rust style 자동 교정 권한 경계

[Rust 코드 스타일 자동 교정 정본](../features/rust-code-style-auto-fix.md)은 M4 mutation protocol에 trusted Rust process와 policy evaluator 경계를 추가한다. `cargo fmt` rewrite와 `cargo clippy --fix`는 target checkout이 아니라 Star-Control-owned isolated preview에서만 실행한다. apply는 external tool을 다시 실행하지 않고 immutable PatchSet의 `.rs` modify operation만 SourceMutationPort로 수행한다.

| workflow/행동 | 최소 action | 권한·승인 binding |
|---|---|---|
| `inspect` | `local_read`; executable probe에 bounded `process_run` | Project/Checkout, manifest/config/toolchain candidate와 probe ToolDescriptor |
| `check`의 rustfmt | `local_read`, `process_run`; owned target artifact write | source/tool/config/scope, source 밖 `CARGO_TARGET_DIR` |
| `check`의 Clippy | trusted-project `process_run`, isolated subject/owned artifact write | package/target/feature/triple cell, build script/proc macro 실행 위험; live target cwd 금지 |
| `prepare` preview | isolated root `local_write`, `process_run`, worktree면 `plan_execute` | exact base/current byte, fixed pipeline/toolchain/policy/coverage, owned roots |
| `safe_default` apply | target `.rs` `local_write` + 사용자 exact ApprovalRequest | PatchSet fingerprint, Checkout, scope/action, before binding, expiry |
| `personal_auto` apply | 같은 `local_write` + policy-resolved exact ApprovalRequest | user standing grant ceiling, candidate `AUTO_PASS`, exact PatchSet/evidence와 pre Gate |
| reverse/recovery | 새 current precondition과 forward와 동등한 action | actual receipt, reverse PatchSet, user byte 보존과 recovery state |

### `safe_default`와 `personal_auto`

`safe_default`는 inspect/check/prepare까지 prompt 없이 진행할 수 있어도 source apply 전에 exact PatchSet 사용자 승인을 요구한다. generic `local_write=auto`, Profile 선택, Recipe prepare 성공과 `cargo` exit 0은 승인이 아니다.

`personal_auto`는 사용자가 config에서 다음 exact standing grant를 선택한 경우에만 terminal `star style rust auto-apply` workflow에서 prompt 없는 apply 후보가 된다.

- ProjectId, `rust_style_auto_fix` Profile exact version/definition과 `rust_style_v1@1` adapter fingerprint
- style policy fingerprint, package/workspace 및 handwritten `.rs` path ceiling
- `process_run`, preview/target `local_write`로 제한된 action set
- file/hunk/byte/line diff limit, public surface delta 0
- permit 전 candidate·`patch_pre_apply` required `AUTO_PASS`, 성공 terminal state 전 `patch_post_apply` required `AUTO_PASS`
- expiry와 user-owned grant fingerprint

standing grant는 exact PatchSet 승인도 bearer token도 아니다. prepare 뒤 policy evaluator가 current candidate를 다시 평가해 existing ApprovalRequest의 exact scope hash, PatchSet fingerprint, before binding, toolchain/policy/coverage/evidence와 expiry를 채우고 `decision=approved`, `resolved_by=policy_evaluator`인 ApprovalDecision을 남긴다. 그 뒤 M3 pre Gate가 `AUTO_PASS`일 때만 application이 one-shot `PatchApplyPermit(kind=automatic)`을 만든다. grant·decision·permit을 다른 Project/Profile/PatchSet에 재사용하지 않는다.

diff limit 초과, public API 영향, create/delete/rename, coverage partial, unpinned/nightly/unstable toolchain, unavailable component/target, dirty overlap/unknown, hunk mapping 불완전, side effect, non-idempotence, stale binding와 candidate/pre `HUMAN_REVIEW|BLOCK`은 automatic approval을 금지한다. apply 뒤 `patch_post_apply`가 `HUMAN_REVIEW|BLOCK`이면 automatic approval을 소급 변경하지 않고 성공 terminal state를 금지해 recovery로 전환한다. policy evaluator는 Gate를 통과시키기 위해 lint suppression/level을 바꾸거나 evidence requirement를 낮출 수 없다.

`auto-apply`는 사용자가 terminal에서 명시적으로 시작한 foreground workflow다. background watcher, filesystem save hook, daemon schedule과 cron으로 standing grant를 소비하지 않는다. `safe_default`/`personal_auto` 차이는 prompt resolution 방식뿐이며 prepare/apply state·event·ID·Evidence와 single Writer 경계는 같다.

### Clippy의 project code 실행

Clippy check/fix는 compile 과정에서 build script와 procedural macro를 실행할 수 있으므로 text-only static read로 분류하지 않는다. 다음을 모두 요구한다.

- 사용자가 trust한 exact Project/Checkout과 registered cargo/Clippy ToolDescriptor
- current source/worktree, package/feature/target/cfg coverage와 process-tree/resource limit
- network `deny`, package/component/target install·update·download `deny`
- source 밖 Star-Control-owned `CARGO_TARGET_DIR`; Cargo cache는 offline/read-only policy
- process 전·후 source root complete filesystem manifest와 undeclared child effect 검사

Job Object와 trusted desktop 실행은 filesystem sandbox가 아니다. build script/proc macro가 source root, Cargo/config/toolchain, generated/vendor 또는 scope 밖 file을 쓰면 `RUST_STYLE_SIDE_EFFECT_VIOLATION`이고 candidate 전체를 폐기한다. network가 필요하거나 child outcome이 unknown이면 자동 retry·설치·완화 없이 coverage incomplete와 target source 불변으로 끝낸다.

`cargo clippy --fix --allow-dirty`는 live checkout에서 금지한다. isolated preview에서도 staged byte가 0이고 dirty manifest 전체가 직전 rustfmt step과 byte-exact 일치할 때만 fixed adapter가 사용할 수 있다. `--allow-staged`, `--broken-code`, `--allow-no-vcs`는 권한 설정으로 허용할 수 없다.

## 비밀정보

- 비밀번호, 인증키, 개인정보 원문을 기록하거나 출력하지 않는다.
- 발견 위치와 위험만 남긴다.
- 외부 전송 전에 검사한다.
- 실행 로그와 증거에도 같은 가림 규칙을 적용한다.

## 금지 경로

프로젝트와 사용자 설정은 읽기 금지, 수정 금지, 외부 전송 금지 경로를 따로 지정할 수 있다.

## 공개 배포 원칙

- 새 사용자는 safe_default로 시작한다.
- personal_auto는 사용자가 의미를 확인한 뒤 직접 선택한다.
- 설치 과정에서 Plugin 검사와 권한 범위를 보여준다.
- 보호 기능이 꺼지면 상태 명령과 실행 보고서에 명확히 표시한다.
