# 설치와 공개 배포

## 목표

개인 사용자는 유료 동작 외에는 자동으로 진행할 수 있고, 공개 사용자는 안전한 기본값으로 시작할 수 있어야 한다. 설정 계층과 Catalog의 상세 계약은 [설정과 Catalog 계약](../contracts/config-and-catalog.md), 검사 계층·artifact 승격·release 상태와 평가는 [10단계 CI·Release·평가 정본](../contracts/ci-release-evaluation-and-product-completion.md), 실제 Windows 설치 transport는 [Windows 설치와 Codex 연동 계약](../contracts/windows-installation-and-codex-integration.md)에서 확인한다.

MCP·외부 Tool Runtime의 OS baseline은 Windows 11 24H2 build 26100 이상이다. 공개 `v0.1.0`에서 x64는 signed Stable, ARM64는 cross-build·simulation 기반 `native_unverified` Preview이며 publication destination은 GitHub Releases다.

## 공개 배포 묶음

하나의 Star-Control release는 다음 두 산출물을 같은 version으로 제공한다.

release source revision·Task ID·config·Catalog·Tool·Profile과 final artifact SHA-256은 `ReleaseManifest` v2로 연결한다. architecture별 artifact는 한 번 build·package해 set digest로 봉인하고, 검증·승격·publish는 같은 byte를 사용한다. release용 재build, 재압축 또는 signing으로 byte가 바뀌면 새 candidate다.

### Codex Plugin

- Plugin 설명 파일
- 반복 작업 Skill
- MCP server 설정
- Hook 정의
- 기본 설정과 안내 자산
- 권한과 개인정보 설명

### Windows Runtime

- `star.exe`, `star-controller.exe`, `star-mcp.exe`, `star-updater.exe` 실행 파일
- required `star-control-core.toml`, ToolPackageManifest Schema와 fake example
- Windows installer와 uninstall 정보
- 상태·설정 migration
- license와 제3자 고지

`checksums.sha256`와 `release-manifest.json`은 항상 final artifact set에서 생성한다. SBOM·provenance·signing은 release policy가 각각 `required|not_required|unavailable|incomplete|complete`로 판정한다. required 자료가 unavailable/incomplete이면 release readiness를 차단하고, not-required이면 empty placeholder 대신 이유·policy·decision ref를 남긴다.

Installer는 runtime과 Plugin의 호환 version과 final artifact digest를 확인한다. Plugin은 설치 뒤 활성화 상태, Hook 신뢰 상태, MCP 준비 상태를 확인해야 한다. installer·Plugin·runtime metadata의 version은 root canonical version source와 일치해야 하며 각 파일이 독립 version 정본이 되지 않는다.

## 설치 경험

1. 사용자가 Windows runtime과 Codex Plugin을 같은 release에서 설치한다.
2. 포함된 기능과 권한을 확인한다.
3. Hook 정의를 검토하고 신뢰한다.
4. Star-Control MCP와 Controller를 활성화한다.
5. 목표 CLI `star doctor`로 binary·Plugin·MCP·Hook, project toolchain·manifest·lockfile와 Windows 환경의 version·상태를 읽기 전용으로 확인한다.
6. `safe_default`로 network·remote write·paid action·source mutation이 없는 deterministic first-run smoke를 실행한다.
7. 원하는 사용자는 personal_auto를 선택한다.

Stable 설치 evidence는 disposable clean Windows x64에서 수집한다. ARM64 Preview는 cross-build, PE architecture, file manifest, Authenticode signature, installer model과 fake lifecycle을 검증하며 native process·IPC·Controller·CLI·MCP·install을 통과했다고 표시하지 않는다. required core command가 `unavailable`이면 x64 first-run success나 release ready가 아니다.

주 배포 방식은 **installer-first**다. P-0026에서 architecture별 current-user Inno Setup 6 `.exe` 설치 파일을 선택하고 구현했다. 설치 마법사의 기본값은 `%LOCALAPPDATA%\Programs\Star-Control`이며 사용자가 바꿀 수 있고, update·repair는 같은 AppId의 이전 선택 경로를 재사용한다. portable archive는 개발·복구용 선택 산출물일 뿐 installer와 같은 수명주기 지원을 뜻하지 않는다.

P-0026/P-0039는 설치 transport, 네 Runtime binary, release-file manifest, installation record, 로컬 Codex Marketplace 렌더링과 updater one-shot 경계를 구현한다. M10 `ReleaseManifest` 상태기계, CI·공개 승격, 서명·SBOM·provenance까지 완료됐다는 뜻은 아니다. 로컬 빌드는 항상 `unsigned_local`로 기록하며 실제 서명 검증이 없는 입력으로 `signed`를 선택할 수 없다. Authenticode certificate나 timestamp provider가 없으면 unsigned Stable로 낮추지 않고 `blocked_external`을 유지한다.

Installer는 current-user Controller startup entry를 눈에 띄게 설명하고 기본 활성화한다. 설치 화면에서 해제할 수 있어야 하며 설치 후 `star controller autostart enable|disable|status`와 제거 방법을 제공한다. entry는 `star-controller.exe --background`만 시작하며 Goal이나 개발 작업을 예약·실행하지 않는다.

관리자 권한 executor나 service는 설치하지 않는다. 실제 기능에 elevation이 필수라는 use case, 최소 권한 protocol과 별도 위협 모델이 승인된 뒤에만 후속 설계로 추가할 수 있다.

`star doctor`는 현재 문서에 정의된 **목표 command이며 아직 구현되지 않았다**. 설치 성공을 주장하는 근거로 예시 출력을 사용하지 않는다.

2026-07-14 현재 PC에서 수행한 x64 실제 설치·Codex Plugin 설치와 x64·ARM64 패키지 검증 값은 [Windows 설치·Codex Plugin 로컬 실증](../testing/windows-installation-evidence-2026-07-14.md)에 분리해 기록한다. 이 근거는 native ARM64, 현재 release candidate의 clean x64 제거, 서명·공개 배포 Gate를 대신하지 않는다.

## doctor와 clean-room 운영 경계

doctor는 installer·repair tool·package manager가 아니다. [6단계 계약 호환성·환경 정본](../contracts/contract-compatibility-and-environment.md)의 `ProjectDoctorReport`를 만들기 위해 exact registered read-only probe만 사용한다.

doctor가 확인하는 범위는 다음과 같다.

- 설치된 Star-Control binary·Plugin·MCP·Hook의 identity와 호환 version
- 대상 project의 package manifest, lockfile, toolchain/runtime/package-manager 선언과 주요 registered task
- OS build·architecture, drive/UNC·junction, case behavior·collision, encoding·BOM, CRLF/LF, path length·long-path capability
- config key declaration·Schema·문서·reader·override provenance와 environment variable의 name/presence contract
- generated reference의 source/generator/input/output hash
- `CleanRoomSpecification`의 source·toolchain·lockfile·command·network/cache/path constraint 완전성

doctor가 하지 않는 동작은 다음과 같다.

- network download·update check·advisory DB refresh
- package restore/install/update, SDK/toolchain 설치 또는 lockfile rewrite
- Windows registry·PATH·execution policy·code page·long-path 설정 변경
- source, generated file, user/project config, Git state 변경
- service·scheduler·startup entry 추가·삭제
- secret·environment variable 실제 값과 raw 사용자 경로 출력·저장

누락 도구나 설정 차이가 있으면 stable Diagnostic, 현재/기대 상태, `safe_auto_fix=false`인 수동 remediation과 검증할 Check를 출력한다. `--fix`, `--install`, `--download`, `--configure-system` option은 제공하지 않는다. 설치·수정이 필요한 조치는 사용자가 별도 절차와 승인을 선택한 뒤 doctor 바깥에서 수행한다.

clean-room readiness도 환경을 만들지 않는다. exact source와 lockfile, preprovisioned toolchain, 등록 command, test network/cache policy와 writable disposable output root가 준비됐는지만 진단한다. 실제 clean-room 검사는 이미 준비된 disposable 환경에서 별도 M3 Check로 실행하며 dependency download·package install·system mutation은 고정 금지하고 missing prerequisite를 자동 설치하지 않는다.

환경 fingerprint에는 OS/architecture, filesystem capability, logical path shape, encoding/line ending, toolchain/package-manager identity·version·hash, manifest·lockfile·task descriptor hash와 environment variable presence contract만 포함한다. username, home/temp/absolute path, secret·environment value와 wall-clock timestamp는 제외한다.

## 업데이트

- 설치된 version·Plugin/runtime/catalog/config/store compatibility와 current artifact digest를 먼저 읽는다.
- 새 executable·Plugin·catalog를 side-by-side staging하고 ReleaseManifest의 final digest를 확인한다.
- 상태 파일 형식이 바뀌면 이전 버전을 읽을 방법 또는 compatible pre-update backup generation을 제공한다.
- 설정의 알 수 없는 항목을 조용히 삭제·기본값으로 덮어쓰지 않는다.
- 모델 이름은 실행 시 조회하므로 제품 업데이트 없이도 새 모델을 선택할 수 있어야 한다.
- 외부 개발 도구 EXE는 [ToolPackageManifest Reference](../contracts/tool-package-manifest-reference.md)에 맞는 TOML로 추가한다. 저장 뒤 `star tools status`로 새 revision을 확인하며 Star-Control binary update, MCP 재등록·재시작과 Codex 재시작을 요구하지 않는다.
- installer가 만드는 Codex MCP 설정은 [MCP 구현 동결 계약](../contracts/mcp-implementation-contract.md#codex-mcp-설정-정본)의 fixed server·approval 설정과 비교한다.
- Plugin Hook 내용이 바뀌면 사용자가 다시 검토해야 할 수 있음을 안내한다.
- activation 전 이전 executable set·store pointer·startup entry와 rollback validation plan을 고정한다.
- Bootstrap Bridge v1→v2 최초 설치는 offline installer만 수행한다. installer는 `installation finalize` 다음에 `installation bridge initialize --state-generation bootstrap_v2`를 실행하며, Codex·MCP가 실행 중이면 파일 변경 전 중단한다. 이 1회 migration 뒤의 Runtime Generation update는 Codex/MCP 재시작 없이 `star-updater.exe`가 수행한다.
- update 뒤 binary·Plugin·MCP·Controller identity, `safe_default` smoke와 state integrity를 검사한다.
- 실패한 업데이트에서 검증된 이전 artifact digest와 compatible state generation으로 돌아갈 수 있어야 한다.
- Bootstrap Bridge와 Runtime Generation의 구분, activation record 원자 교체, candidate review·approval scope와 `tool_hot_reload|runtime_update|codex_integration_update` 분류는 [Runtime update와 activation 계약](../contracts/runtime-update-and-activation.md) 및 [Codex 생명주기와 Updater 계약](../contracts/codex-lifecycle-and-updater.md)이 소유한다. Runtime-only update는 Codex Plugin cache와 MCP 설정을 바꾸지 않는다.

## Rollback

- binary rollback과 data/config/store downgrade를 같은 동작으로 보지 않는다.
- 이전 binary가 current store를 읽을 수 없으면 user data를 삭제하거나 손실 migration하지 않고 pre-update generation·export·read-only recovery를 사용한다.
- rollback은 exact artifact digest·state generation·startup entry·validation plan에 결합한 새 action이다.
- rollback 뒤 install smoke·state integrity·Plugin/MCP compatibility를 다시 검증한다.
- remote deploy rollback·withdrawal은 local rollback 승인과 별개이며 before/after remote snapshot이 필요하다.
- rollback 실패·outcome unknown은 release success로 숨기지 않고 `rollback_required`·recovery hold로 남긴다.

## Uninstall과 사용자 자료

기본 uninstall은 installer ownership manifest가 소유한 program file, runtime·Plugin registration과 Controller startup entry만 제거한다. 다음은 기본 보존한다.

- `%APPDATA%\Star-Control` user config·trusted tool manifest
- `%LOCALAPPDATA%\Star-Control` management state·backup·quarantine·release/evaluation evidence index
- 대상 Project의 `.star-control` source와 `.ai-runs` evidence
- 사용자 source·Git repository·worktree와 remote state

user data purge는 uninstall의 숨은 option이 아니라 별도 destructive action이다. exact owned path class·예상 byte·backup/export·retention hold·승인을 먼저 보여주고 ownership을 증명하지 못한 path는 삭제하지 않는다.

## Release 상태와 공개 승인

- `candidate`: final artifact set이 봉인됐지만 release Gate 미완료
- `ready`: clean build·package·install lifecycle Gate가 current·complete
- `approved`: exact manifest revision·digest·channel·provider의 주 publication 승인이 current
- `published`: provider after snapshot이 exact version·source·artifact digest를 확인
- `publish_outcome_unknown`: adapter call 뒤 실제 원격 결과를 확인할 수 없음

top-level `approved|published|publish_outcome_unknown`은 주 publication channel을 표현한다. deploy·withdrawal·remote rollback은 target별 `remote_actions[]`의 approval·operation·before/after observation과 `verified|outcome_unknown|rollback_required`를 유지하며 top-level publication 상태를 되감지 않는다. `ready`는 공개 상태가 아니고 `approved`는 성공 receipt가 아니다. 실제 remote 결과를 확인하지 않고 `published`로 표시하지 않는다.

## 개인정보와 기록

- 기본 실행 기록은 로컬에 저장한다.
- 외부 업로드는 사용자가 활성화한 기능에서만 일어난다.
- 공개 보고서에 로컬 절대 경로, 사용자 이름, 인증 정보를 넣지 않는다.
- 사용자가 기록을 확인하고 정리할 수 있는 명령을 제공한다.

## 공개 프로젝트

- Windows 지원 범위를 명확히 적는다.
- 안전 기본값과 자동화 프로필의 차이를 설명한다.
- Codex 기능 변화에 따른 호환 범위를 공개한다.
- 새 프로젝트도 MIT License로 배포한다.
- 최종 배포 전 clean Windows x64에서 설치, safe_default 첫 실행, 업데이트, failure rollback, repair, 제거·user data 보존 흐름을 검증하고 ARM64 Preview는 model-equivalent fake lifecycle을 별도로 검증한다.
- source revision·artifact digest·version·changelog·license·conditional supply-chain 자료와 remaining risk를 ReleaseManifest에 공개한다.
- GitHub Releases의 Runtime·installer Authenticode는 required다. SBOM·provenance의 current applicability와 signer·timestamp provider는 P9 실행 시점에 검증한다.
