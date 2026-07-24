# P-0057 현재 시스템 전수 점검 — 2026-07-25

## 판정 경계

이 감사는 문서의 `DONE`을 현재 실행 사실로 상속하지 않는다. live source, P-0056 artifact, 실행 중 설치 Runtime을 서로 다른 subject로 고정하고, 각 결과가 실제로 읽거나 실행한 revision·manifest·byte에만 귀속되게 했다. 실제 사용자 management/project data, 설치 root, Codex runtime DB/cache는 변경하지 않았다.

| 표면 | exact subject | 이 감사에서 인정한 범위 |
|---|---|---|
| 시작 live source | `main`/`origin/main` `816be57b4de21c0da937d2500686fc8395293ef6`, tree `c0339f71e6f4d985bcfacb515917c3e899044f53` | 시작 clean 상태와 pre-change FULL 10/10 |
| P-0056 artifact | `1bce4724c34414cef74862dbe9bf9de1f094ad2f`, tree `4e1c3b1d55bfbe35eb7eaf4455c02bde711bcac4` | `dist/release-evidence/p0056-1bce4724`의 historical package·lifecycle·provider evidence만 |
| 실행 중 설치 Runtime | source `b20d234b38a7dcb347049b6b95aff3407c5dedc9`, generation `rt_c569d8e23ed61e8e` | 설치 verify, Controller/Registry/action 실측과 설치본에서 재현된 결함 |
| P-0057 수정 source | 이 문서를 포함한 최종 local source revision | 아래 세 결함의 source fix와 final FULL/RELEASE Gate. 설치본에 적용됐다는 뜻은 아님 |

## 최종 판정

- **현재 시스템 점검과 source 수정:** `DONE`. 12개 우선순위를 live evidence에 대조했고, 로컬에서 재현 가능한 제품 결함 3건을 수정했다.
- **실행 중 설치본:** `verified / update held`. 설치·Codex 연동과 Controller single-writer는 정상이나, 설치 byte는 P-0057 수정 전이므로 permission·empty-v1 migration·Registry root dedupe 수정이 아직 반영되지 않았다. 실행 중인 Codex/Star process를 임의 종료하거나 설치본을 교체하지 않았다.
- **비서명 로컬 후보:** `verified / unpublished`. exact x64 stage, 격리 finalize/activation, installer compile과 ARM64 cross-build를 확인했지만 공개 후보가 아니다.
- **공개 x64 Stable:** `blocked_external`. Authenticode certificate/private key/trusted timestamp가 없고 signed-byte clean lifecycle·provenance·publish/readback도 없다.
- **ARM64 Preview:** `native_unverified`. Rust 1.96 cross-build·corpus·Clippy와 PE/model 검증일 뿐 native runner 실행이 아니다.

## 실행 중 설치 Runtime과 lifecycle

| 항목 | 관찰 | 판정 |
|---|---|---|
| installation | root manifest `sha256:d96015a0bdb6f2fc437e0251a87266acecb20b12b79d629af6486df606edbe0c`, Runtime manifest `sha256:634a36495a2fe51937e0ba6369f30287021bcfa036fd7c5e1de8cef566e36ae9`, install/update verify `true` | 설치 byte 자체는 `verified` |
| Codex integration | rendered Plugin과 cache의 `plugin.json`, `.mcp.json`, `hooks.json`, Star skill byte 일치; `requires_new_task=true`, `hook_trust_required=true` | 등록 정상, 새 작업·사람 Hook trust 경계 유지 |
| Controller | Controller 1개, writer lock이 해당 PID와 일치; 여러 `star-mcp`는 같은 Codex parent의 client process | split-brain 근거 없음 |
| restart receipt | 최신 integration restart operation `upd_0MMCLNfG-BIK2dSXJJnn9Xs9KGeLMC8E1pxeRaleWXc`는 terminal `aborted`; durable operation store에 active record가 없고 그 뒤 현재 generation이 activation됨 | dangling operation 아님. 과거 best-effort relaunch 기록으로 종결 |
| management | live v1 DB는 projects 0, mutable table 0, metadata 8; `recovery_only`, `RECOVERY_STORE_MIGRATION_REQUIRED`, `RECOVERY_ACTIVE_SET_MATERIALIZATION_MISMATCH` | 사용자 data 손상 근거는 없으나 pristine empty-v1 복구 dead-end를 재현 |
| signatures | 설치 root의 Controller/MCP/Updater/CLI 모두 `NotSigned` | local verified와 public trusted를 분리 |

## 핵심 17 action 실제 E2E

Registry revision 7의 `star.control.core` 17개 action을 모두 search→describe해 exact descriptor hash, risk lane, required call tool과 `ready`를 확인한 뒤 disposable goal `gol_01KYB6TGY82P1R6MPCF54A7VN1`에서 실행했다.

- `goal.start`의 `waiting_question` → `goal.answer` → active, `plan.update/get`, `goal.status`, pause/resume/cancel과 revision CAS를 확인했다.
- `doctor.run`은 4개 check와 17개 command를 PASS했고 `project.list/status`는 13/13 available·identity match를 반환했다.
- `validation.plan`은 dirty data-migration 변경을 FULL과 independent review로 승격했고, `evidence.get`은 exact report digest를 다시 읽었다.
- `validation.run` 1초 제한 operation은 `VALIDATION_TIMEOUT` terminal failure로 끝나 timeout을 success로 승격하지 않았다.
- stale descriptor는 `TOOL_DESCRIPTOR_STALE`, 없는 ChangeBundle의 `merge.status`/`handoff.get`은 `COORDINATION_NOT_FOUND`로 닫혔다.
- goal cancel은 current revision에서 terminal cancelled까지 관찰했다. 이 과정에서 설치본의 destructive action이 approval 없이 실행되는 결함을 발견했으며, disposable goal 외 사용자 goal/project는 건드리지 않았다.

`ready`는 실행 완료 주장이 아니다. 위 결과는 설치 source `b20d234`에만 귀속한다. P-0057 수정 source의 approval 경계는 isolated test로 별도 증명했다.

## 발견한 결함과 source 수정

| 결함 | 원인 | 수정과 검증 | 설치 상태 |
|---|---|---|---|
| destructive core action이 safe-default 승인 없이 dispatch | action 권한을 live EffectiveConfig가 아니라 일부 문자열 allowlist로 판정했고 direct core 경로는 durable approval gate를 통과하지 않음 | 모든 `permission_actions`를 `permissions.actions.<id>`의 `auto|prompt|deny`로 fail-closed 판정하고 generic/direct 경로 모두 dispatch 전에 `approval_wait` 또는 `POLICY_DENIED`; direct `goal.cancel`에서 operation 미시작 회귀 test | 실행 중 설치본에는 미반영 |
| 빈 pristine v1 store를 v2로 migration할 수 없음 | migration plan이 project entry 0개를 invalid로 거부 | 빈 plan을 유효한 global-only migration으로 허용하고 backup→apply→idempotent retry→rollback test 추가 | 실행 중 설치본에는 미반영 |
| DB migration 뒤 active-set store version이 stale | DB `user_version`만 바꾸고 sealed active-set header를 갱신하지 않음 | apply/rollback 뒤 store ID·scope·generation을 read-only 재검증하고 version·header·manifest를 atomic reseal | 실행 중 설치본에는 미반영 |
| Registry unavailable root 중 같은 project root가 2회 집계 | DOS path와 `\\?\` verbatim path를 다른 key로 watch | Windows watch path를 dedupe 전 normalize하고 DOS/verbatim missing-root test 추가 | 실행 중 revision 7에는 미반영 |

## persisted state·복구 안전

- live DB는 SQLite backup API로 격리 snapshot만 만들고 원본은 read-only로 관찰했다. snapshot integrity는 정상이고 v1 project 0개 상태였다.
- `star-state` 13개 test에서 writer lease, crash 시 current generation 보존, backup/restore/tamper, crash-point atomic restore, immutable replay, v1→v2 apply/retry/rollback을 통과했다.
- source/manifest가 canonical이고 DB/index는 derived라는 경계를 유지했다. migration active-set은 DB identity와 불일치하면 `IntegrityFailed`로 닫힌다.
- stale plan/backup fingerprint, orphan/missing binding, tampered backup, duplicate/replayed input은 success로 합치지 않는다.

## 권한·trust·Registry

- paid `yes|unknown`과 EffectiveConfig `prompt`는 exact descriptor/arguments/project scope/hash/expiry에 묶인 durable approval을 만든다. `deny`, missing, unknown permission 값은 `POLICY_DENIED`다.
- approval operation은 process start 전 `approval_wait`이고 raw arguments·actor·runtime scope를 응답에 노출하지 않는다. descriptor 변경은 기존 scope 재사용을 막는다.
- AppContainer는 manifest가 허용하는 좁은 compatibility 주장만 유지한다. `trusted_desktop`은 sandbox로 표시하지 않는다.
- live Registry `watched=2`, `unavailable=3`은 release root 누락이 아니었다. default user root 1개와 동일 project root의 DOS/verbatim 표현 2개였으며 source에서 중복 집계를 수정했다.
- watcher event/overflow는 truth가 아니라 invalidation이고, overflow 뒤 full demand scan으로 snapshot을 회복하는 test를 통과했다. LKG·manifest/Schema/EXE identity mismatch는 ready로 승격하지 않는다.

## M1~M6와 13개 교차 저장소

현재 source의 validation planning은 변경 경로를 재관찰해 Catalog/Index freshness → impact → ValidationPlan → immutable Patch/Gate 순서를 유지했다. data migration 변경은 FULL로 승격됐고 validator/corpus/Schema 약화 없이 최종 workspace Gate에 포함됐다. coordination test는 bundle/handoff atomic persistence, publish timeout 단일 시도, reconcile 뒤 `outcome_unknown` handoff 차단, remote request identity·approval evidence 재계산을 통과했다.

Project Catalog 13개는 모두 현재 origin identity와 일치했지만 merge-ready로 합치지 않았다. 관찰 시점의 별도 상태는 다음과 같다.

- dirty 11개: Star-Control 외 개발도구·콘텐츠·단풍·Emulink·포맷·모드 파운드리·저장소·생태계_정본·지식·코어. 기존 변경은 수정·숨김·commit하지 않았다.
- upstream과 다른 checkout: 콘텐츠·어댑터·지식은 ahead 1, 저장소는 behind 3. branch/HEAD/remote/pin은 repo별 사실로 분리했다.
- Star-Control worktree 3개 중 1개와 언어 worktree 2개 중 1개가 prunable로 표시됐지만 삭제하지 않았다. 단풍은 worktree 200개이며 숫자만으로 cache/폐기 대상으로 분류하지 않았다.
- active ChangeBundle이 없으므로 provider/consumer 순서나 전체 성공은 주장하지 않는다. `partial|held|rollback|outcome_unknown` participant를 pass로 합치는 경로는 test가 차단한다.

## 설치·공급망·P2

- P-0057 x64 source candidate는 503-file stage verify, isolated finalize→Bridge v2 activation, installation verify를 통과했다. 현재 사용자 설치 root와 `%APPDATA%`/`%LOCALAPPDATA%` state는 바꾸지 않았다.
- Inno Setup 6.7.3으로 x64 installer model을 compile했지만 `NotSigned`다. 실행 중 Codex/Star process가 있는 host에서 installer preflight를 우회하지 않았으므로 clean Windows install→repair→uninstall 실증은 현재 candidate의 환경 evidence gap이다.
- lifecycle unit 3개는 illegal order/same-byte/user-data-loss 차단, x64 failed-update rollback·data 보존, ARM64 fake-native 승격 차단을 통과했다.
- RustSec는 현재 `Cargo.lock` 223 dependencies, advisory DB 1,169개에서 vulnerability 0·warning 0이다. P-0056 SBOM/provenance는 current candidate evidence로 상속하지 않는다.
- ARM64는 Rust 1.96 `aarch64-pc-windows-msvc` workspace release cross-build, corpus check·Clippy를 통과했다. native 장비 실행은 없으므로 `native_unverified`다.
- 23개 feature/16 Profile, generated Schema, 170/170 matrix와 workspace test·all-feature Clippy는 final FULL/RELEASE Gate가 재검증한다. 실제 13개 사용자 workload의 비용·효율·오탐 평가는 사용자 data mutation 없이 수행할 독립 EvaluationRun이 없어 `not_run`으로 남긴다.

## 검증 Gate

최종 local source revision은 다음 Gate를 모두 통과해야 이 감사의 `DONE` 판정을 갖는다. 생성 report와 exact hash는 source 파일에 되먹임해 revision을 바꾸지 않고 최종 handoff에서 해당 revision과 함께 고정한다.

| 명령/증거 | 기대 판정 |
|---|---|
| focused Registry, permission, coordination, recovery tests | PASS; regression과 negative case 포함 |
| `cargo audit --json` | vulnerability 0, warning 0 |
| `pwsh ./scripts/validate.ps1 -Profile full` | 10/10 `complete/stable/pass` |
| `pwsh ./scripts/validate.ps1 -Profile release` | source-owned check PASS, external signing/publication 1개만 `unverified`; 실패 0 |
| x64 stage verify·isolated activation·installer compile | PASS / `unsigned_local`, 실제 사용자 install preserved |
| ARM64 cross-build | PASS / `native_unverified` |

## 남은 실행 경계

1. 실행 중 설치본을 P-0057 수정 byte로 update하고 Codex를 restart하는 작업은 설치·system state 변경이므로 별도 사용자 승인이 필요하다. 적용 전까지 destructive core action은 사용하지 않고 live management migration도 실행하지 않는다.
2. disposable clean Windows x64의 installer install→update→injected rollback→repair→uninstall과 user-data preservation은 현재 candidate에서 아직 실행하지 않았다.
3. 실제 13개 프로젝트 workload EvaluationRun과 ARM64 native runner는 `not_run`/`native_unverified`다.
4. Authenticode certificate/private key/trusted timestamp, signed Runtime/installer, signed-byte SBOM·provenance·lifecycle와 public GitHub publish/readback은 `blocked_external`이다. signing 뒤 byte가 달라지면 전 증거를 새 candidate로 다시 만든다.
