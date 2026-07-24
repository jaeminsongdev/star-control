# P-0055 비서명 외부 Gate·복구 Slice 봉인 감사

## 목적과 판정 경계

이 문서는 `main` 기준 P-0054 내부 제품 구현 위에서 서명을 제외한 외부 실행 경로를 실제 adapter·Operation·영수증·도메인 상태·원격 readback까지 닫는 P-0055의 진행·최종 증거 정본이다. 사용자는 package/dependency 설치, network, disposable install/update/repair/uninstall, GitHub branch/tag/draft/upload/readback/cleanup과 ARM64 교차 simulation을 승인했다.

다음 두 판정은 끝까지 분리한다.

- 비서명 제품/운영 경로: 코드·Corpus·실행 증거가 모두 닫히면 `DONE`으로 봉인할 수 있다.
- 공개 Stable: Runtime·installer Authenticode와 trusted timestamp가 없으므로 unsigned publish로 우회하지 않고 `blocked_external`을 유지한다.

ARM64는 `aarch64-pc-windows-msvc` 교차 빌드, PE architecture, file manifest, installer model과 fake/disposable lifecycle로 검증한다. 이 증거는 Preview `native_unverified`이며 실제 ARM64 장치 runtime 성공으로 표현하지 않는다.

## 현재 판정

- **제품 구현과 recovery 보강:** `DONE / non-signing external seal`. replacement installer reconcile, `partially_applied` 복구, interrupted pre-apply receipt `aborted` 종결, 무재시작 reconcile과 canonical Runtime payload set digest 기반 generation identity를 구현했다. exact `0d0eca9a` source의 FULL/RELEASE, 두 architecture package, 격리 lifecycle과 현재 호스트 idempotence를 다시 봉인했다.
- **P-0055 remote delivery seal:** `DONE / unpublished`. origin/GitHub가 exact `0d0eca9a` commit/tree를 readback했고 disposable draft의 provider/download digest 왕복 뒤 release/tag 부재까지 확인했다. draft는 publish하지 않았다.
- **current Codex Runtime 통합:** `DONE`. verified installed payload의 manifest-owned `rt_c569d8e23ed61e8e`를 Desktop 재시작 없이 activation revision 5로 reconcile했다. current Codex MCP registry revision 7에서 core 17개 search·describe·invoke를 실행했고 TARGET Operation도 종단 성공했다. fixed EXE byte를 새 source candidate로 교체하는 maintenance install은 runtime readiness 복구와 분리한다.
- **공개 Stable:** `blocked_external`. Runtime·installer Authenticode와 trusted timestamp가 없고 unsigned publish는 차단된다.

## 기준선과 보존 범위

| 항목 | 값 |
|---|---|
| repository | `D:\개발\관제\Star-Control` |
| 시작 branch | `main` |
| 기준 HEAD / `origin/main` | `a93de7e68aff3ac02315d3a324aeaa497e1ede38` |
| 작업 branch | `codex/p0055-nonsigning-external-seal` |
| 최초 implementation commit / tree | `4554c4a56564ecea800a335dfbf4bb82d546e299` / `2eb3680b3f0cf5a8ae6b0daadff6fe54f003e067` |
| 최종 비서명 artifact source commit / tree | `0d0eca9a0fc441eb3cedb0d044608c3393222f07` / `3f33005b0ff4a159560d0f87500c3b41a2ff09a9` |
| recovery·identity chain | `7eedc7b` manifest-owned reconcile → `e248efe` 무재시작/receipt 종결 → `0d0eca9a` payload-content identity |
| artifact source push/readback | `origin/main=a93de7e` 포함 PASS / 봉인 시 origin branch exact `0d0eca9a` readback PASS. 후속 docs-only commit은 stage source revision을 바꾸지 않음 |
| 포함 변경 | P-0054/P-0055 구현, 복구 Slice와 content identity 보강. 후속 정본 문서 commit은 473-file 출시 byte set의 source revision을 바꾸지 않음 |
| 보존 | P-0054/P-0055 source 전체, `legacy/`, Codex runtime DB/state, installed Plugin cache. `target/`·`dist/` 증거는 정리하지 않음 |
| 금지 | Authenticode signing, unsigned Stable publish, source 대신 `dist/`·`target/` 직접 수정, 검사 약화 |

## 최신 main 기준 전수조사 결론

P-0054 감사에서 남은 비서명 gap은 “등록 adapter가 필요하다”는 계획 상태와 실제 process effect 사이의 공통 증거 경로, M10 provider 구현, ARM64 교차 검증, clean lifecycle, installed Codex readiness, authenticated GitHub readback이었다. P-0055는 이 gap을 다음 구현 단위로 닫는다.

| 범위 | 구현 | fail-closed 조건 | 현재 증거 상태 |
|---|---|---|---|
| 공통 외부 effect | `DevelopmentEffectReceiptV1`, `RegisteredDevelopmentEffectAdapter`, durable `OperationSnapshot.permission_actions`, `development.effect.record` | exact Project/subject/descriptor/arguments/executable/approval/permission/Gate 불일치, stale/partial/unknown 거부 | focused unit·Controller·CLI PASS |
| M7 security | SecurityRefresh/LicenseScan 영수증과 exact input source SHA-256 소비 | succeeded 영수증 없는 available input, 다른 source/effect/Project 거부 | Controller path 구현·테스트 PASS |
| M7 dependency | DependencyPrepare/DependencyApply 영수증 status projection | 영수증만으로 canonical manifest/lockfile applied 판정 금지; M4 PatchSet 필요 | Controller path 구현·테스트 PASS |
| M8 migration | MigrationExecute 영수증을 attempt tool observation·permission·Gate와 교차 검증 | failed/partial/outcome_unknown, plan fingerprint/Operation 불일치 거부 | Controller path 구현·테스트 PASS |
| M8 performance | PerformanceRun 영수증을 exact workload subject와 결합 | process/subject evidence 없는 측정 결과 거부 | Controller path 구현·테스트 PASS |
| M8 language cutover | LanguageCutover 영수증과 equivalent report/current Gate를 검증해 `applied` 기록 | `ready_for_*`를 성공으로 사용하지 않음; unknown/partial 거부 | Controller path 구현·테스트 PASS |
| M9 remote recovery | RemoteRecovery 영수증과 exact RecoveryPlan·permission·Gate를 검증해 `applied` 기록 | provider effect 없거나 stale/partial/unknown이면 거부 | Controller path 구현·테스트 PASS |
| M10 GitHub publisher | `ReleaseAssetBindingV1`, draft-first create, no-clobber upload, publish, readback/reconcile, bounded streaming download, exact remote-name snapshot | source/tag/asset digest·size mismatch, ambiguous timeout, unsafe remote path, unsigned Stable 거부 | provider 5 tests·FULL PASS; authenticated draft byte 왕복·cleanup PASS |
| Runtime Generation identity | canonical Runtime payload file-set digest 기반 `rt_<digest>`, stage/reseal rename, verifier 재계산 | source revision만 같은 서로 다른 byte의 generation collision과 stale selector 오인 거부 | same-revision payload-change regression 6/6 package tests·Clippy PASS; exact x64 `rt_23cd8e31911f8415`, ARM64 `rt_5913080cde8a516b` stage 검증 PASS |
| Schema/fixture | 두 신규 management Schema와 minimal/full/invalid/future fixture | future/invalid 수용 금지 | generated manifest 188개; final Schema gate PASS |
| CLI | `tools call`, approval resolve, operation get/cancel, effect receipt record와 receipt-required domain syntax | CLI가 DB/tool/provider를 직접 열지 않음 | 19 unit + 1 evidence test PASS |

## 공통 외부 effect 실행 계약

```text
Tool Registry ready descriptor
  + fixed risk lane
  + exact bounded arguments
  + durable approval / PermissionDecision / GateDecision when required
    -> Controller tool.invoke
    -> terminal durable Operation
    -> development.effect.record
    -> canonical DevelopmentEffectReceiptV1
    -> domain apply/status command
```

영수증은 다음을 fingerprint에 포함한다.

- Project와 exact subject ref/fingerprint
- OperationId, Tool ID, descriptor SHA-256, canonical arguments SHA-256, executable SHA-256
- approval, permission, Gate reference와 effect별 Permission Action
- started/completed 시각, `succeeded|failed|partial|outcome_unknown`, `source_effect_started`
- exact artifact refs, result fingerprint, limitation과 receipt fingerprint

Controller는 저장된 영수증을 읽을 때도 다시 seal하여 fingerprint를 비교한다. 따라서 저장 후 document 변조, stale arguments, 다른 Operation/effect/Project 재사용이 도메인 결과로 침투하지 않는다.

## M10 GitHub 게시 경계

`ReleaseAssetBindingV1`은 ReleaseManifest의 exact source revision/tag/destination과 모든 asset의 local path·remote name·size·SHA-256를 고정한다. GitHub adapter는 `gh` executable identity를 확인하고 다음 순서를 강제한다.

1. target commit에 draft release를 생성하거나 exact existing draft를 read-only reconcile한다.
2. `gh release upload`의 `file#text`가 rename이 아니라 label 문법인 점을 고려해, 512 MiB 이하 local asset을 exact `remote_name`의 임시 snapshot으로 복사·flush한 뒤 업로드한다. path component가 든 remote name은 거부한다.
3. remote asset name 충돌은 overwrite하지 않고 digest/size readback으로 동일 byte인지 확인한다.
4. upload stdout은 bounded capture, asset download는 bounded file streaming으로 처리한다.
5. publish 뒤 release와 모든 asset을 다시 조회하고 exact source/tag/digest/size가 일치할 때만 verified after-state를 반환한다.
6. timeout·partial response는 write 재시도가 아니라 read-only reconcile로 `publish_outcome_unknown`을 해소한다.
7. unsigned Stable은 signing policy에서 publish apply 전 차단한다.

## 검증 원장

| 시각/묶음 | 명령·대상 | 결과 | 증거 |
|---|---|---|---|
| 구현 focused·identity regression | contracts·execution·Controller·CLI·GitHub adapter와 package stage/reseal/verifier test·Clippy | PASS | 위험 permission durable approval, remote byte download, same-source payload-change와 fresh-stage/reseal ID 일치 포함; package regression 6/6 |
| exact FULL | native exact source `0d0eca9a` | 10/10 complete·stable PASS | `target/validation/20260724T112510915Z-3852/report.json`, `sha256:3f3ada0647e577455283769e15c3eed2583cfa9bf4a29c8ec8610aa7c759633b` |
| exact RELEASE | native exact source `0d0eca9a` | 14/15 PASS, failed 0, unverified 1 | `target/validation/20260724T112310049Z-10640/report.json`, `sha256:73abe9690a8c8e0b1cbdc643e90ea7ec7b2bb25038f5c449e970d8aec4277e9b`; signing/publication만 unverified |
| x64 build/package/lifecycle | explicit x64 release → 473-file stage verify → isolated finalize/Bridge v2/status | PASS | set `sha256:5344c92e…97ff`, manifest `sha256:0a446f2f…9834`, generation `rt_23cd8e31911f8415`, nested manifest `sha256:19121add…e4e65`, PE `0x8664`, lifecycle `verified=true`·activation revision 1 |
| ARM64 cross-build/simulation | Rust 1.96 explicit cross-build, corpus check·Clippy, 473-file stage verify | PASS / `native_unverified` | set `sha256:b165fb75…be12`, manifest `sha256:217e3db6…738a`, generation `rt_5913080cde8a516b`, nested manifest `sha256:7ff82e68…183e`, PE `0xaa64` |
| installer model | Inno Setup 6.7.3 x64·ARM64 unsigned model | PASS / public 불가 | x64 24,180,687 bytes `sha256:396cbb29…b268`; ARM64 20,181,633 bytes `sha256:0822f557…66c4`; 둘 다 `NotSigned` |
| SBOM/audit/provenance | Syft 1.45.0, RustSec `1abf7a8`, pre-sign provenance | PASS / public 불가 | SBOM 각 7 packages `2beac26b…e428`/`c252d867…5188`; audit 223 dependencies·vulnerability 0·warning 0; provenance `sha256:cbba5c53…67dc` |
| current Codex·exact candidate no-op | exact staged CLI `reconcile-installed-runtime` + live MCP | PASS / no restart | action search에 전용 ready action이 없어 native fallback. Operation `upd_Ns0vvX…`, `activation_changed=false`, revision 5·`rt_c569…` 유지, 종료 PID 0, integration verified; registry revision 7 release 17/17 ready |
| GitHub draft upload/readback/cleanup | authenticated exact `0d0eca9a` disposable draft | PASS / unpublished | release `359263161`, 1,261-byte asset local/provider/download `sha256:fd4a5bf3…bfaf3` 일치. release ID/tag/tag ref cleanup 후 absent; evidence `sha256:3b623692…8606` |
| Git delivery | latest main ancestry, artifact-source push, commit/tree remote readback | PASS | 봉인 시 `origin/main=a93de7e`, exact `0d0eca9a`/tree `3f33005b`가 origin branch와 GitHub API에서 일치 |

Windows Cargo incremental finalization에서 간헐적 access-denied warning이 나타날 수 있다. test process의 종료 코드를 우선 기록하되 경고를 숨기지 않으며, 현재 정책상 `target/`을 삭제해 우회하지 않는다.

## 완료 조건

P-0055는 다음이 모두 충족될 때만 `DONE / non-signing external seal`로 바꾼다. 현재 판정은 다음과 같다.

- focused test, generated Schema/fixture, format, lint와 exact local commit FULL: **PASS**.
- RELEASE failed 0: **PASS**. 유일한 unverified는 승인 범위 밖 signing/publication이다.
- exact `0d0eca9a` x64 build/package/disposable lifecycle와 current Codex 17/17 readiness: **PASS**. 고정 EXE maintenance install이나 추가 Desktop restart는 완료 조건이 아니다.
- exact ARM64 cross-build·PE/manifest·installer model·simulation corpus와 `native_unverified`: **PASS**.
- exact SBOM·cargo audit·pre-sign provenance artifact 결합: **PASS**.
- authenticated GitHub exact `0d0eca9a` draft byte upload/readback/hash/cleanup: **PASS**. draft는 publish하지 않았고 cleanup final state는 absent다.
- branch/local commit/clean worktree/push/remote source readback: **PASS**.
- STRICT 검토: **PASS with fix**. 위험 권한 durable approval/Gate 실재 검증, remote asset 실제 download, exact remote-name snapshot을 보강했고 partial/unknown·unsigned Stable 승격은 차단된다.

## 남는 외부 위험

비서명 exact reseal과 복구 Slice에는 열린 제품 blocker가 없다. `dist/release-evidence/p0055-e248efe4`와 `p0055-e248efe4-r2`는 stale explicit-target binary와 source-derived generation collision을 드러낸 비채택 증거로 보존하며 최종 candidate에 사용하지 않는다.

남은 blocker는 **서명/공개 한 층**이다. Authenticode certificate·private key·trusted timestamp provider가 없으므로 signed Runtime·installer, signed clean install, signed provenance와 public Stable publish는 계속 `blocked_external`이다. signing 뒤에는 signed byte를 새 candidate로 보고 content ID·SBOM·provenance·설치·Codex·GitHub publish/readback을 재실행한다.
