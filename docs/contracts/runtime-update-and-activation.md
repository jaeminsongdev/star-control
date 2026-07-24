# Runtime update와 activation 계약

## 범위와 정본

이 문서는 고정 Bootstrap Bridge가 선택하는 Runtime Generation, activation record, 후보 기능 검토와 update operation의 불변식을 소유한다. 설치 transport와 Plugin 렌더링은 [Windows 설치와 Codex 연동 계약](windows-installation-and-codex-integration.md), 고정 MCP wire는 [MCP 구현 동결 계약](mcp-implementation-contract.md), 외부 Tool Registry hot reload는 [외부 Tool Registry](external-tool-registry.md)가 각각 소유한다.

현재 P-0038 구현은 persisted contract/schema, Bootstrap의 generation selector, Windows activation record 원자 writer, verified generation stage, candidate review와 rollback까지 포함한다. P-0039는 [ADR-0014](../decisions/ADR-0014-전용-Star-Updater와-Codex-생명주기.md)에 따라 그 mutation owner를 `star.exe`에서 단발 `star-updater.exe`로 옮긴다. Registry action은 MCP의 고정 진입점을 넓히지 않기 위해 별도 release package로만 후속 도입하며, installed-tree same-task E2E는 설치 상태를 바꾸므로 명시적 실행 승인이 필요하다.

Bridge v1에서 v2로의 최초 migration은 routine update가 아니다. offline installer가 Codex와 Star-Control process가 없는 상태에서 release를 복사하고 `installation finalize` 뒤 `installation bridge initialize --state-generation bootstrap_v2`를 실행한다. 이 명령은 activation record가 없으면 stage의 단일 generation을 선택하고, record가 있으면 검증된 기존 selector를 다시 bridge에 bind하는 idempotent recovery다. 이미 v2인 설치본의 routine Runtime-only update는 이 경계를 다시 통과하지 않는다. 다만 fixed Gateway·Plugin·Updater와 Runtime을 함께 교체하는 full/mixed replacement installer는 Codex를 닫은 detached updater transaction 안에서만 아래의 manifest-owned generation reconcile을 수행한다.

## Runtime Generation

generation root는 `<install-root>\\runtime\\generations\\rt_<digest>\\`이다. `<digest>`는 source revision 문자열이 아니라 Controller·generation-local CLI Runtime·core catalog·schema의 canonical file-set SHA-256에서 파생한다. 같은 source revision을 주장해도 payload byte가 다르면 다른 generation ID여야 하며, package verifier는 directory/manifest 이름 일치뿐 아니라 payload set digest에서 ID를 다시 계산한다. unsigned 또는 signed reseal로 payload byte가 바뀌면 generation directory도 새 content ID로 바꾼 뒤 top-level manifest를 다시 봉인한다. generation에는 Controller, generation-local CLI Runtime, release manifest, core catalog와 schema가 함께 있어야 한다. 부분 복사는 후보가 될 수 없고, controller/catalog/schema는 서로 다른 generation에서 섞일 수 없다.

`RuntimeGenerationManifest`는 generation reference, target architecture, Controller hash와 Controller/CLI/catalog/schema 경로, bridge contract version을 기록한다. 모든 경로는 install root 아래여야 하며 reparse/path traversal와 release manifest digest 불일치는 거부한다.

## Activation record

`RuntimeActivationRecord`는 `%LOCALAPPDATA%\\Star-Control\\installation\\active-runtime.v1.json`에 저장한다.

- `active`: 현재 generation과 release manifest digest
- `previous`: rollback 가능한 직전 generation; 없으면 apply 불가
- `activation_revision`: 단조 증가 activation revision
- `state_generation_id`: Runtime state compatibility 경계
- `bridge_contract_version`: Bridge와 generation 호환성 gate

writer는 임시 파일을 flush한 뒤 원자적으로 교체한다. 손상·외부 경로·digest 불일치 record는 활성화하지 않으며 Last Known Good record가 있으면 그것으로 복구한다. 활성 또는 rollback generation은 별도 retention 작업 전에는 삭제하지 않는다.

## Candidate review와 승인

`RuntimeCandidateReview`는 mutation 없이 다음을 반환한다.

- update class와 candidate digest
- action added/removed/changed
- breaking schema, risk lane/permission widening
- handler readiness와 bridge compatibility
- rollback availability
- `requires_codex_restart`, `requires_new_task`, `hook_review_required`
- exact candidate에 바인딩된 `approval_scope_sha256`

`star update stage <runtime-generation-dir>`는 검증된 generation만 `<install-root>\runtime\generations\<id>`로 새 파일 생성 방식으로 복사한다. 기존 generation은 덮어쓰지 않는다. `star update inspect <generation-id>`는 release-owned tool package manifest를 비교해 candidate review와 exact `approval_scope_sha256`를 만든다.

`star update apply <generation-id> --state-generation <id> --approve <sha256>`는 bridge compatible, handler ready, rollback available, candidate verification pass, approval scope 일치를 모두 요구한다. action 제거, breaking schema, risk lane/permission widening 또는 hook/new-task/Codex restart가 필요한 review는 runtime apply로 통과시키지 않는다. Stable root `star.exe`는 설치 manifest 전체 검증 뒤 `star-updater.exe runtime-apply`에 요청만 위임한다. Updater가 인증된 Controller shutdown, bounded quiesce, selector 원자 교체, 새 Runtime Controller start와 IPC postcheck 및 rollback을 단독 소유한다. Codex와 고정 MCP를 포함하는 통합 변경은 이 경로가 아니라 ADR-0014의 10초 restart-armed updater transaction을 사용한다.

### Replacement installer의 generation reconcile

`star update offline-installer-restart`는 setup 성공만으로 완료되지 않는다. 기존 activation record가 있으면 Inno Setup의 `installation bridge initialize`는 그 selector를 보존하고, detached updater가 setup 종료 뒤 다음을 모두 수행한다.

1. current-user installation record와 새 root `release-manifest.json`의 전체 file identity를 다시 검증한다.
2. retained rollback directory의 정렬 순서가 아니라 새 root manifest가 `runtime/generations/<id>/...`로 소유한 정확히 한 generation만 선택한다. 0개·복수 generation, generation/runtime manifest 누락과 digest 불일치는 거부한다.
3. 선택한 generation의 release package manifest에서 expected ToolId 집합을 계산하고 prior activation을 rollback reference로 보존한 새 `RuntimeActivationRecord`를 원자 교체한다.
4. 새 Controller를 시작한 뒤 release source 전체 action 집합과 `ready` 집합을 각각 page 끝까지 읽는다. 둘 다 expected ToolId 집합과 정확히 같아야 `offline_verified`로 진행한다.
5. activation 또는 live Registry postcheck가 실패하면 Codex가 닫힌 상태에서 candidate Controller를 drain하고 prior activation을 복원·기동한다. replacement fixed files는 이미 설치됐으므로 전체 prior release rollback으로 과장하지 않고 receipt를 `partially_applied`로 남긴다. prior selector 복구까지 실패하면 `rollback_failed`이며 어느 경우도 성공으로 승격하지 않는다.

fresh install처럼 setup 단계에서 이미 bundled generation이 active인 경우에도 4의 live postcheck를 생략하지 않는다. 이 경계는 breaking action schema를 routine Runtime apply로 우회하는 통로가 아니라, fixed integration까지 함께 교체되어 새 Codex task가 필수인 full/mixed installer transaction 전용이다.

### 이미 설치된 payload의 무재시작 reconcile

fixed EXE나 Plugin byte를 교체할 필요 없이 설치 root의 `release-manifest.json`이 소유한 Runtime Generation만 stale selector 때문에 비활성인 경우에는 Codex Desktop을 재시작하지 않는다.

```text
star update reconcile-installed-runtime --install-root <absolute-path> [--json]
```

이 명령은 global update lease를 획득하고 설치 record·전체 release file identity를 재검증한 뒤 manifest가 소유한 정확히 한 generation과 그 package manifest의 expected ToolId 집합을 계산한다. apply 전 단계에서 updater가 사라져 `planned|staged|candidate_verified|restart_armed|countdown|draining|codex_stopped`에 남은 동일 install root receipt가 있으면, lease를 현재 명령이 소유한다는 증거 아래 그 receipt만 `aborted`로 종결한다. `applying` 이후 상태, `rollback_required`, `partially_applied`는 자동으로 다시 분류하지 않는다.

selector가 다르면 prior Controller image를 먼저 캡처하고 activation record를 원자 교체한 다음 그 정확한 구 image에 graceful shutdown을 요청한다. 12초 drain 뒤에도 남은 경우에만 해당 Controller image tree를 exact identity로 종료한다. Codex Desktop과 `star-mcp.exe`는 종료하지 않으며, fallback PID는 결과에 기록한다. MCP supervisor가 새 selector로 Controller를 다시 시작하므로 앱 재시작 없이 연결이 회복된다.

postcheck는 detached/staged updater를 Controller 신뢰 경계에 추가하지 않는다. 설치 manifest로 검증된 `<install-root>\star.exe`를 bounded subprocess probe로 사용해 release source의 declared 집합과 `ready` 집합을 각각 읽고, source·readiness·중복·pagination을 검증한 뒤 expected ToolId와 두 집합이 모두 정확히 같을 때만 성공한다. activation 뒤 shutdown·Registry·integration postcheck가 실패하면 selector를 먼저 prior로 원복하고 candidate Controller를 drain한 뒤 prior Controller를 기동한다. 원복 성공은 reconcile 실패이며 성공으로 표시하지 않고, 원복 실패는 별도 `rollback_failed` 계열 오류다.

이 경로는 설치 payload 자체를 갱신하지 않는다. root EXE, Plugin, Hook 또는 installer-owned file이 달라졌다면 여전히 offline installer maintenance restart가 필요하다. 따라서 “Runtime selector만 stale”과 “설치 byte 교체 필요”를 같은 재시작 요구로 과장하지 않는다.

## 상태와 실패 처리

durable update operation은 `planned`, `staged`, `candidate_verified`, `approval_required`, `accepted`, `draining`, `quiesced`, `activating`, `new_controller_started`, `postcheck_running`, `committed`만 성공 경로로 가진다.

`accepted`는 supervisor가 인계받을 수 있다는 뜻일 뿐이다. `committed`와 postcheck evidence가 있어야 성공이다. 실패는 `aborted`, `rollback_required`, `rolling_back`, `rolled_back`, `partially_applied`, `rollback_failed`, `outcome_unknown`로 구분한다. `partially_applied`는 replacement files가 남은 채 prior Runtime selector만 복원된 상태이며 `rolled_back`과 같지 않다.

Controller가 종료된 뒤에도 단발 Updater가 activation과 새 Controller start를 담당한다. Runtime Controller는 launch argument로 Bootstrap install root를 받으며, IPC peer 검증은 generation-local CLI와 fixed Bootstrap의 `star.exe`/`star-mcp.exe`만 허용한다. MCP는 cutover 중 mutation을 재시도하지 않고 `CONTROLLER_UPDATING` 또는 명시적 bounded wait를 반환한다.

## 구현 완료 조건

1. `star-contracts` type과 generated schema가 이 문서의 persisted type을 표현한다.
2. activation record는 경로·digest·unknown field를 fail-closed 처리한다.
3. apply/rollback은 exact approval scope에 바인딩되며 `star-updater.exe`가 Controller shutdown, selector 교체, 새 Controller postcheck와 rollback을 소유한다. `star.exe`는 verified delegation만 한다.
4. 같은 Codex 작업에서 ChatGPT PID와 MCP PID를 유지한 채 Controller PID와 generation만 교체하는 installed-tree 실기 증거는 별도 설치 상태 변경 승인 후 남긴다.
5. update 전후 fixed MCP 12개와 Plugin/MCP 설정 hash가 불변인 실기 증거는 4와 함께 남긴다. Registry action은 fixed MCP 변경 없이 별도 package로 search·describe·call 검증한다.
6. x64 Stable의 native lifecycle·crash-point rollback·current artifact digest evidence가 있다. ARM64 Preview는 cross-build·architecture·manifest·signature·installer model·fake lifecycle evidence를 가지며 native 결과는 `native_unverified`로 남긴다.
7. replacement installer는 root manifest가 소유한 bundled generation과 live release Registry의 declared/ready ToolId exact equality를 검증하며, stale selector·복수 generation·postcheck 실패를 성공으로 보고하지 않는다.
