# P-0055 비서명 외부 Gate·복구 Slice 봉인 감사

## 목적과 판정 경계

이 문서는 `main` 기준 P-0054 내부 제품 구현 위에서 서명을 제외한 외부 실행 경로를 실제 adapter·Operation·영수증·도메인 상태·원격 readback까지 닫는 P-0055의 진행·최종 증거 정본이다. 사용자는 package/dependency 설치, network, disposable install/update/repair/uninstall, GitHub branch/tag/draft/upload/readback/cleanup과 ARM64 교차 simulation을 승인했다.

다음 두 판정은 끝까지 분리한다.

- 비서명 제품/운영 경로: 코드·Corpus·실행 증거가 모두 닫히면 `DONE`으로 봉인할 수 있다.
- 공개 Stable: Runtime·installer Authenticode와 trusted timestamp가 없으므로 unsigned publish로 우회하지 않고 `blocked_external`을 유지한다.

ARM64는 `aarch64-pc-windows-msvc` 교차 빌드, PE architecture, file manifest, installer model과 fake/disposable lifecycle로 검증한다. 이 증거는 Preview `native_unverified`이며 실제 ARM64 장치 runtime 성공으로 표현하지 않는다.

## 현재 판정

- **제품 구현과 격리 비서명 증거:** `DONE`. M7~M10 external effect, Recovery consume path, x64 격리 lifecycle, ARM64 cross-build/simulation, SBOM·audit·pre-sign provenance, candidate Inspector 17/17과 disposable GitHub draft byte readback을 완료했다.
- **P-0055 immutable delivery seal:** `BLOCKED_HOST_POLICY`. 정확한 staged index tree는 만들었지만 호스트 실행 정책이 `git commit`을 `AskForApproval=Never`로 거부해 clean revision·push·candidate commit target draft를 만들 수 없다.
- **current Codex candidate 통합:** `BLOCKED_ACTIVE_HOST`. exact candidate는 Inspector 17/17이지만 현재 설치본 registry revision 4는 6개 ready core action이다. 실행 중인 Codex에서 installer를 겹쳐 실행하지 않았고 설치 Controller는 원래 generation으로 복구했다.
- **공개 Stable:** `blocked_external`. Runtime·installer Authenticode와 trusted timestamp가 없고 unsigned publish는 차단된다.

## 기준선과 보존 범위

| 항목 | 값 |
|---|---|
| repository | `D:\개발\관제\Star-Control` |
| 시작 branch | `main` |
| 기준 HEAD / `origin/main` | `a93de7e68aff3ac02315d3a324aeaa497e1ede38` |
| 작업 branch | `codex/p0055-nonsigning-external-seal` |
| 최종 코드 candidate index tree | `2eb3680b3f0cf5a8ae6b0daadff6fe54f003e067` |
| commit/push | 호스트 정책이 직접 `git commit`을 거부해 미생성·미push |
| 포함 변경 | 기존 P-0054 dirty implementation 전체와 그 위의 P-0055 외부 seal |
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
| final FULL | Star Operation `opn_01KY7RG0QP2SHGYRX5BHMQQT8X` | 10/10 complete·stable PASS | `target/validation/20260723T151125247Z-6868/report.json`, `sha256:52ca57a2a84d45314fc7d35977ef0c5cea21b09d5dda7ae5ee30015fcf681a4b` |
| final RELEASE | Star Operation `opn_01KY7RXZ6Z1TMCVF639XR85VA4` | 13/15 PASS, failed 1, unverified 1 | `target/validation/20260723T151902921Z-24688/report.json`, `sha256:cbc1ce2afb1bbbf281939ae09c36b515ba7478caac384af7d7061abea260ea46`; clean-worktree fail + signing/publication unverified |
| x64 build/package/lifecycle | workspace release → 473-file stage verify → isolated finalize/Bridge v2/status | PASS | set `sha256:bf38c2144047b449846e6dcced57c224a0134ca9a2788bbc8ff6c71c6dcc6325`, manifest `sha256:67bf451156d5137ba11d3c405ac56bc2414d79463c1921a96877be3647a47620`, all Runtime PE `0x8664`, lifecycle `verified=true` |
| ARM64 cross-build/simulation | Rust 1.96 workspace release, corpus check·Clippy, stage verify·fake lifecycle | PASS / `native_unverified` | set `sha256:fcf34fe11c9b7911a30730acae4c40062418ed358c71eabe1aee5762c880ae9d`, manifest `sha256:f13e6727da2e634cc75a7f5c7605c6b5ae797d6055b20b5d98032b2d937f56ed`, all Runtime PE `0xaa64` |
| installer model | Inno Setup 6 x64·ARM64 unsigned model | PASS / public 불가 | x64 24,120,745 bytes `sha256:0b4d1e3f247b6261ed24b6542fa502a09aeed22ab621a7484caa508c8c0183f8`; ARM64 20,154,992 bytes `sha256:4845da887cc97d8104d7dcc72269c659d5baad08b125220fa2ff10c82e697fbd` |
| SBOM/audit/provenance | Syft 1.45.0, current RustSec, pre-sign provenance | PASS / public 불가 | SBOM 각 7 packages; audit 223 dependencies·vulnerability 0·warning 0; provenance `sha256:117b600af6be1840329a57b4d2ad744d0245860f4f71adebdba29b266b14ecf5` |
| candidate MCP | official Inspector 0.22.0, exact x64 candidate | PASS | fixed 12/12, required core search 17/17·describe 17/17, Controller SHA-256 `635f1f48…fbf3` |
| current Codex | restored installed Controller live search | NOT CLOSED | registry revision 4, ready core 6/17. candidate를 current 설치본으로 승격하지 않음 |
| GitHub draft upload/readback/cleanup | authenticated `jaeminsongdev/star-control`, disposable draft | byte 왕복·cleanup PASS / candidate target 미충족 | asset/provider/download `sha256:761d2714…0130`; release·release API·tag ref 모두 absent. target은 commit 부재로 base `a93de7e…` |
| Git delivery | branch, stage, commit, push, remote readback | BLOCKED_HOST_POLICY | implementation 530 paths staged·artifact subject tree `2eb3680b…`; 이 최종 감사 문서 delta는 구현 commit 뒤 별도 stage 대상이다. 직접 `git commit`이 `approval required by policy, but AskForApproval is set to Never`로 거부됨 |

Windows Cargo incremental finalization에서 간헐적 access-denied warning이 나타날 수 있다. test process의 종료 코드를 우선 기록하되 경고를 숨기지 않으며, 현재 정책상 `target/`을 삭제해 우회하지 않는다.

## 완료 조건

P-0055는 다음이 모두 충족될 때만 `DONE / non-signing external seal`로 바꾼다. 현재 판정은 다음과 같다.

- focused test, generated Schema/fixture, format, lint와 FULL: **PASS**, 단 commit이 없어 immutable revision은 아님.
- RELEASE failed 0: **미충족**. signing check는 예상 `unverified`이고 clean-worktree가 host commit 정책 때문에 1 fail이다.
- x64 build/package/disposable lifecycle: **PASS**. current Codex 17/17: **미충족**, candidate Inspector만 17/17이다.
- ARM64 cross-build·PE/manifest·installer model·simulation corpus와 `native_unverified`: **PASS**.
- SBOM·cargo audit·pre-sign provenance current artifact 결합: **PASS**.
- authenticated GitHub draft byte upload/readback/hash/cleanup: **PASS**. exact candidate commit target: **미충족**, commit이 없어 base commit을 사용했다.
- branch/stage: **PASS**. commit/push/remote source readback: **미충족**, host policy 차단이다.
- STRICT 검토: **PASS with fix**. 위험 권한 durable approval/Gate 실재 검증, remote asset 실제 download, exact remote-name snapshot을 보강했고 partial/unknown·unsigned Stable 승격은 차단된다.

## 남는 외부 위험

현재 남은 blocker는 세 층이다.

1. **호스트 Git 정책:** staged index tree `2eb3680b…`를 local commit으로 만들고 branch를 push할 수 있는 실행 경로가 필요하다. commit 뒤 source revision을 commit SHA로 바꿔 FULL/RELEASE·package·SBOM/provenance·GitHub exact target readback을 다시 봉인해야 한다.
2. **current Codex 전환:** Codex와 Star process를 닫을 수 있는 별도 install/update 창에서 exact committed candidate를 설치하고 새 Codex task에서 core 17개 search·describe·invoke를 재감사해야 한다. 실행 중 task 안에서 installer를 겹쳐 실행하지 않는다.
3. **서명/공개:** Authenticode certificate·private key·trusted timestamp provider가 없으므로 signed Runtime·installer, signed clean install, signed provenance와 public Stable publish는 계속 `blocked_external`이다. signing 뒤에는 signed byte를 새 candidate로 보고 SBOM·provenance·설치·Codex·GitHub publish/readback을 재실행한다.
