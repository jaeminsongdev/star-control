# P-0055 비서명 외부 Gate·복구 Slice 봉인 감사

## 목적과 판정 경계

이 문서는 `main` 기준 P-0054 내부 제품 구현 위에서 서명을 제외한 외부 실행 경로를 실제 adapter·Operation·영수증·도메인 상태·원격 readback까지 닫는 P-0055의 진행·최종 증거 정본이다. 사용자는 package/dependency 설치, network, disposable install/update/repair/uninstall, GitHub branch/tag/draft/upload/readback/cleanup과 ARM64 교차 simulation을 승인했다.

다음 두 판정은 끝까지 분리한다.

- 비서명 제품/운영 경로: 코드·Corpus·실행 증거가 모두 닫히면 `DONE`으로 봉인할 수 있다.
- 공개 Stable: Runtime·installer Authenticode와 trusted timestamp가 없으므로 unsigned publish로 우회하지 않고 `blocked_external`을 유지한다.

ARM64는 `aarch64-pc-windows-msvc` 교차 빌드, PE architecture, file manifest, installer model과 fake/disposable lifecycle로 검증한다. 이 증거는 Preview `native_unverified`이며 실제 ARM64 장치 runtime 성공으로 표현하지 않는다.

## 현재 판정

- **제품 구현과 recovery 보강:** `RESEALING`. M7~M10 external effect와 Recovery consume path 위에 replacement installer의 manifest-owned Runtime reconcile, `partially_applied` 복구, 무재시작 installed-runtime reconcile과 interrupted pre-apply receipt `aborted` 종결을 구현했다. 직전 exact `7eedc7b` FULL/RELEASE와 artifact는 통과했으며 현재 보강 source를 새 exact candidate로 재봉인 중이다.
- **P-0055 remote delivery seal:** `DONE`. origin branch와 GitHub commit API가 exact `b20d234`/tree `ea4407e`를 확인했고, exact-target draft asset의 local/provider/download SHA-256 일치와 release/tag cleanup을 readback했다. draft는 publish하지 않았다.
- **current Codex Runtime 통합:** `DONE`. verified installed payload의 manifest-owned `rt_c569d8e23ed61e8e`를 Desktop 재시작 없이 activation revision 5로 reconcile했다. current Codex MCP registry revision 7에서 core 17개 search·describe·invoke를 실행했고 TARGET Operation도 종단 성공했다. fixed EXE byte를 새 source candidate로 교체하는 maintenance install은 runtime readiness 복구와 분리한다.
- **공개 Stable:** `blocked_external`. Runtime·installer Authenticode와 trusted timestamp가 없고 unsigned publish는 차단된다.

## 기준선과 보존 범위

| 항목 | 값 |
|---|---|
| repository | `D:\개발\관제\Star-Control` |
| 시작 branch | `main` |
| 기준 HEAD / `origin/main` | `a93de7e68aff3ac02315d3a324aeaa497e1ede38` |
| 작업 branch | `codex/p0055-nonsigning-external-seal` |
| implementation commit / tree | `4554c4a56564ecea800a335dfbf4bb82d546e299` / `2eb3680b3f0cf5a8ae6b0daadff6fe54f003e067` |
| 제품 증거 기준 commit / tree | `b20d234b38a7dcb347049b6b95aff3407c5dedc9` / `ea4407eab1a782fcd94ff671686cdedf952b44e6` |
| commit/push | local 두 commit과 clean worktree PASS / origin branch exact `b20d234` readback PASS |
| 포함 변경 | P-0054/P-0055 구현 전체와 첫 정본 감사 snapshot. 이후 증거 문서-only 갱신은 473-file 출시 byte set에 포함되지 않음 |
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
| 구현 focused | contracts·execution·Controller·CLI와 `star-adapter-github` test/Clippy | PASS | GitHub 5, Controller 86, CLI 19+1; 위험 permission durable approval 회귀 포함 |
| final FULL | official Inspector 0.22.0 + `SessionStart→UserPromptSubmit→Stop`, Operation `opn_01KY7WS7Y2ZEXB10WPKR2D583X` | 10/10 complete·stable PASS | `target/validation/20260723T162621247Z-36040/report.json`, `sha256:26c029c48f4ec2374906310edf4ffdc656b778aeda174797308ea578079e5b32`; requested=required=selected `full` |
| final RELEASE | native exact source `b20d234` | 14/15 PASS, failed 0, unverified 1 | `target/validation/20260723T154718929Z-29140/report.json`, `sha256:631c204633f4b5b1d44c8abde7006eeaec4a8a96666f08e31d4027cc4003b5df`; signing/publication만 unverified |
| x64 build/package/lifecycle | workspace release → 473-file stage verify → isolated finalize/Bridge v2/status | PASS | set `sha256:20ae1b660d135a363c4061a4808a9ce10dc6fe537485c47d9cf6824223d92eaa`, manifest `sha256:d96015a0bdb6f2fc437e0251a87266acecb20b12b79d629af6486df606edbe0c`, all Runtime PE `0x8664`, lifecycle `verified=true` |
| ARM64 cross-build/simulation | Rust 1.96 workspace release, corpus check·Clippy, stage verify·fake lifecycle | PASS / `native_unverified` | set `sha256:9872eb8e00d845a3a1dcad6f1b63972fadae56d791ab257a114efb170c15b808`, manifest `sha256:958b141dbc0ea2dd8565c3ab138e6577241bfdfc2ef5f761c46d6ad65b3dfd5a`, all Runtime PE `0xaa64` |
| installer model | Inno Setup 6.7.3 x64·ARM64 unsigned model | PASS / public 불가 | x64 24,125,769 bytes `sha256:a9d029b083c2b7d515421ae0d8a474668cd13a3f582ba8db2299d290d411289f`; ARM64 20,153,290 bytes `sha256:6b9f2bfc0316c5fc61e9549e7addad4e77e9cfc4cdfc7368d1af41116a3ccb72` |
| SBOM/audit/provenance | Syft 1.45.0, current RustSec, pre-sign provenance | PASS / public 불가 | SBOM 각 7 packages; audit 223 dependencies·vulnerability 0·warning 0; provenance `sha256:5f819316082de1ced749ee69524dadbb350d21b5883cdf009558eb044c9da78c` |
| candidate MCP | official Inspector 0.22.0, exact x64 candidate | PASS | fixed 12/12, required core search 17/17·describe 17/17, `validation.run` Operation 종단 성공, Controller SHA-256 `635f1f48…fbf3` |
| current Codex | installed-runtime reconcile + live MCP | PASS | interrupted restart `upd_0MMCLNf…`는 apply 전 `aborted`; activation revision 5 `rt_c569d8e23ed61e8e`, integration verified, registry revision 7 declared=ready 17/17. 17개 search·describe·invoke 중 15 success, ChangeBundle 없는 disposable goal의 merge/handoff는 expected `COORDINATION_NOT_FOUND`; validation Operation `opn_01KY9TWQERDG6FF2WHVR389VE5` TARGET 8/8 PASS, evidence `sha256:4d443a68…f186` |
| GitHub draft upload/readback/cleanup | authenticated `jaeminsongdev/star-control`, disposable exact-target draft | PASS / unpublished | release `359047620`, target `b20d234`; 515-byte asset local/provider/download `sha256:67b05a54…6637` 일치. release ID/tag와 tag ref 모두 cleanup 후 absent. roundtrip evidence `sha256:9d764cdb…6cdf` |
| Git delivery | branch, commit, push, remote readback | PASS | `4554c4a` implementation + `b20d234` docs/product-evidence 기준 commit. origin branch와 GitHub commit API가 exact commit/tree/parent를 확인함 |

Windows Cargo incremental finalization에서 간헐적 access-denied warning이 나타날 수 있다. test process의 종료 코드를 우선 기록하되 경고를 숨기지 않으며, 현재 정책상 `target/`을 삭제해 우회하지 않는다.

## 완료 조건

P-0055는 다음이 모두 충족될 때만 `DONE / non-signing external seal`로 바꾼다. 현재 판정은 다음과 같다.

- focused test, generated Schema/fixture, format, lint와 exact local commit FULL: **PASS**.
- RELEASE failed 0: **PASS**. 유일한 unverified는 승인 범위 밖 signing/publication이다.
- x64 build/package/disposable lifecycle: 직전 exact candidate **PASS**, 현재 보강 source 재봉인 필요. current Codex 17/17 search·describe·invoke: **PASS**.
- ARM64 cross-build·PE/manifest·installer model·simulation corpus와 `native_unverified`: **PASS**.
- SBOM·cargo audit·pre-sign provenance current artifact 결합: **PASS**.
- authenticated GitHub exact `b20d234` draft byte upload/readback/hash/cleanup: **PASS**. draft는 publish하지 않았고 cleanup final state는 absent다.
- branch/local commit/clean worktree/push/remote source readback: **PASS**.
- STRICT 검토: **PASS with fix**. 위험 권한 durable approval/Gate 실재 검증, remote asset 실제 download, exact remote-name snapshot을 보강했고 partial/unknown·unsigned Stable 승격은 차단된다.

## 남는 외부 위험

현재 남은 blocker는 두 층이다.

1. **비서명 exact reseal:** 현재 recovery 보강 source를 commit하고 FULL/RELEASE, x64/ARM64 stage·installer·SBOM·audit·provenance, x64 격리 lifecycle, exact GitHub draft byte 왕복과 remote readback을 다시 생성해야 한다. Runtime 17/17은 무재시작으로 닫혔으므로 fixed payload 교체만을 위한 불필요한 추가 restart를 완료 조건으로 만들지 않는다.
2. **서명/공개:** Authenticode certificate·private key·trusted timestamp provider가 없으므로 signed Runtime·installer, signed clean install, signed provenance와 public Stable publish는 계속 `blocked_external`이다. signing 뒤에는 signed byte를 새 candidate로 보고 SBOM·provenance·설치·Codex·GitHub publish/readback을 재실행한다.
