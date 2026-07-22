# Runtime update와 activation 계약

## 범위와 정본

이 문서는 고정 Bootstrap Bridge가 선택하는 Runtime Generation, activation record, 후보 기능 검토와 update operation의 불변식을 소유한다. 설치 transport와 Plugin 렌더링은 [Windows 설치와 Codex 연동 계약](windows-installation-and-codex-integration.md), 고정 MCP wire는 [MCP 구현 동결 계약](mcp-implementation-contract.md), 외부 Tool Registry hot reload는 [외부 Tool Registry](external-tool-registry.md)가 각각 소유한다.

현재 P-0038 구현은 persisted contract/schema, Bootstrap의 generation selector, Windows activation record 원자 writer, verified generation stage, candidate review와 rollback까지 포함한다. P-0039는 [ADR-0014](../decisions/ADR-0014-전용-Star-Updater와-Codex-생명주기.md)에 따라 그 mutation owner를 `star.exe`에서 단발 `star-updater.exe`로 옮긴다. Registry action은 MCP의 고정 진입점을 넓히지 않기 위해 별도 release package로만 후속 도입하며, installed-tree same-task E2E는 설치 상태를 바꾸므로 명시적 실행 승인이 필요하다.

Bridge v1에서 v2로의 최초 migration은 routine update가 아니다. offline installer가 Codex와 Star-Control process가 없는 상태에서 release를 복사하고 `installation finalize` 뒤 `installation bridge initialize --state-generation bootstrap_v2`를 실행한다. 이 명령은 activation record가 없으면 stage의 단일 generation을 선택하고, record가 있으면 검증된 기존 selector를 다시 bridge에 bind하는 idempotent recovery다. 이는 유일한 fixed Gateway/manifest 교체와 Codex/MCP 재시작 경계다. 이미 v2인 설치본의 이후 Runtime update는 이 경계를 다시 통과하지 않는다.

## Runtime Generation

generation root는 `<install-root>\\runtime\\generations\\rt_<digest>\\`이다. generation에는 Controller, generation-local CLI Runtime, release manifest, core catalog와 schema가 함께 있어야 한다. 부분 복사는 후보가 될 수 없고, controller/catalog/schema는 서로 다른 generation에서 섞일 수 없다.

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

## 상태와 실패 처리

durable update operation은 `planned`, `staged`, `candidate_verified`, `approval_required`, `accepted`, `draining`, `quiesced`, `activating`, `new_controller_started`, `postcheck_running`, `committed`만 성공 경로로 가진다.

`accepted`는 supervisor가 인계받을 수 있다는 뜻일 뿐이다. `committed`와 postcheck evidence가 있어야 성공이다. 실패는 `aborted`, `rollback_required`, `rolling_back`, `rolled_back`, `rollback_failed`, `outcome_unknown`로 구분한다.

Controller가 종료된 뒤에도 단발 Updater가 activation과 새 Controller start를 담당한다. Runtime Controller는 launch argument로 Bootstrap install root를 받으며, IPC peer 검증은 generation-local CLI와 fixed Bootstrap의 `star.exe`/`star-mcp.exe`만 허용한다. MCP는 cutover 중 mutation을 재시도하지 않고 `CONTROLLER_UPDATING` 또는 명시적 bounded wait를 반환한다.

## 구현 완료 조건

1. `star-contracts` type과 generated schema가 이 문서의 persisted type을 표현한다.
2. activation record는 경로·digest·unknown field를 fail-closed 처리한다.
3. apply/rollback은 exact approval scope에 바인딩되며 `star-updater.exe`가 Controller shutdown, selector 교체, 새 Controller postcheck와 rollback을 소유한다. `star.exe`는 verified delegation만 한다.
4. 같은 Codex 작업에서 ChatGPT PID와 MCP PID를 유지한 채 Controller PID와 generation만 교체하는 installed-tree 실기 증거는 별도 설치 상태 변경 승인 후 남긴다.
5. update 전후 fixed MCP 12개와 Plugin/MCP 설정 hash가 불변인 실기 증거는 4와 함께 남긴다. Registry action은 fixed MCP 변경 없이 별도 package로 search·describe·call 검증한다.
6. x64 Stable의 native lifecycle·crash-point rollback·current artifact digest evidence가 있다. ARM64 Preview는 cross-build·architecture·manifest·signature·installer model·fake lifecycle evidence를 가지며 native 결과는 `native_unverified`로 남긴다.
