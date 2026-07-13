# 상태 기록과 이어하기

## 목표

앱을 닫거나 작업이 실패해도 처음부터 다시 조사하지 않도록 목표, 단계, source 관찰, 결과와 다음 행동을 안전하게 저장한다. 동시에 source code와 공유 선언이 로컬 DB에 갇히지 않게 정본·projection·evidence를 분리한다.

Project·ScanRun·Finding·PatchSet과 관리 DB lifecycle은 [공통 개발 관리와 로컬 관리 DB 계약](../contracts/development-management.md), ProjectCheckout·ProjectCatalogSnapshot·CodeIndexSnapshot과 freshness 의미는 [Project Catalog·Code Index 계약](../contracts/project-catalog-and-code-index.md), EventEnvelope, RunSnapshot, Checkpoint, Handoff와 전이 불변식은 [이벤트와 상태 계약](../contracts/events-and-state.md)이 소유한다.

## 작업 상태

| 상태 | 의미 |
|---|---|
| draft | 목표가 처음 만들어짐 |
| clarifying | 필요한 질문을 확인 중 |
| planned | 단계와 배정 결과가 만들어짐 |
| approved | 계획이 실행 가능함 |
| running | 한 개 이상의 단계가 실행 중 |
| paused | 사용자가 일시 중단함 |
| validating | 검사 중 |
| reviewing | 독립 검토 중 |
| merging | 병렬 변경을 통합 중 |
| blocked | 사용자 결정이나 외부 상태가 필요함 |
| failed | 자동 복구 범위를 넘겨 실패함 |
| cancelled | 사용자가 취소함 |
| completed | 완료 조건과 증거가 충족됨 |

상태가 바뀔 때 시간, 이유, 관련 단계를 함께 기록한다.

## 저장 위치

### 저장 계층

| 위치 | 저장 내용 | 저장하지 않는 것 |
|---|---|---|
| 대상 Git repository | Project 선언, source, config, Rule·ChangeRecipe, shared suppression·baseline, Schema·Catalog | local scan projection, 개인 path, raw log |
| `%LOCALAPPDATA%\Star-Control\management\global\` | Project directory, ProjectCheckout relation, ProjectCatalogSnapshot, cross-project relation·coordination, global lifecycle summary | project scan detail, source file byte, raw project root |
| `%LOCALAPPDATA%\Star-Control\management\projects\<project-id>\` | project별 revision·workspace·CodeIndexSnapshot partition, graph·Finding query projection, local decision·operation·evidence index | 다른 project detail, 큰 diff·trace, raw project root |
| `%LOCALAPPDATA%\Star-Control\cache\project-index\<project-id>\` | adapter·input fingerprint별 다시 만들 수 있는 content-addressed index intermediate | current pointer, source 전체 복사본, local decision, backup 대상 자료 |
| `<project>\.ai-runs\star-control\` | hash가 있는 diff·patch·log·trace·report·canonical evidence export | DB backend file, secret, 다른 project 절대 path |

Git source가 공유 정본이다. 관리 DB는 source-derived projection과 local-only 운영 상태를 함께 가지지만 source code의 유일한 정본이 아니다. `.ai-runs`는 큰 evidence byte를 소유하고 DB는 ArtifactRef만 저장한다.

### Controller 상태

배경 Controller가 다시 시작해도 필요한 내부 상태는 Windows 사용자 로컬 데이터 폴더에 저장한다.

    %LOCALAPPDATA%\Star-Control\

개념 layout은 다음과 같다. 실제 DB filename과 backend 확장자는 public contract가 아니다.

```text
%LOCALAPPDATA%\Star-Control\
  controller\             # instance·health·single-writer lease
  management\
    active-set.json        # global+project generation header·relative locator를 고정하는 hash manifest
    global\
      active\             # 현재 global opaque store generation
      generations\        # migration·rebuild 후보
      backups\            # verified backup
      recovery\           # 손상 원본의 보존 copy
    projects\
      <project-id>\
        active\           # 이 ProjectId의 현재 generation
        generations\
        backups\
        recovery\
    backup-sets\           # 함께 복구할 generation vector manifest
  root-bindings\          # current-user protected opaque checkout root binding
  cache\
    project-index\
      <project-id>\
        <adapter-id>\
          <cache-key>\     # snapshot·config·adapter fingerprint 기반 재생성 cache
  logs\                   # redaction·retention 적용
```

global DB에는 Project directory·ProjectCheckout·ProjectCatalogSnapshot·cross-project coordination만, ProjectId별 DB에는 source-derived CodeIndexSnapshot partition, event·projection, local decision과 application 상태를 둔다. raw project root는 어느 DB에도 저장하지 않고 `root_binding_id`만 둔다. 실제 root locator는 별도 adapter가 Windows current-user protection으로 암호화한 opaque locator를 해석하며 plaintext는 process memory 밖으로 노출하지 않는다. root binding은 management backup·export에 포함하지 않는다.

0단계 현재 `Project` schema v1은 Project 하나에 `root_binding_id` 하나를 둔다. 1단계 구현은 [Project Catalog·Code Index 계약의 선행 gap](../contracts/project-catalog-and-code-index.md#0단계-선행조건과-호환성-gap)에 따라 binding을 `ProjectCheckout`으로 이동하는 schema migration을 먼저 거친다. migration 전 row를 복수 checkout으로 추정 복제하지 않으며, migration이 끝나기 전에는 단일 attached checkout만 current로 취급한다. 이 문서 반영은 schema·DB 구현 완료를 뜻하지 않는다.

cache는 store generation과 별도다. 삭제·miss·손상 시 같은 source와 fingerprint로 재생성해야 하며 `active-set.json`, backup-set, integrity 성공과 current 판정의 필수 자료가 아니다. cache key는 ProjectId·WorkspaceSnapshotId·partition·adapter fingerprint·index config fingerprint로 만들고 directory 이름에 project명·path·사용자명을 넣지 않는다. source 전체 byte, secret, 개인 절대 경로와 민감 literal은 cache에 저장하지 않는다.

v1 management DB byte 전체를 암호화하지 않는다. 관리 directory, DB auxiliary file, backup과 recovery copy에는 current user와 SYSTEM만 허용하는 Windows ACL을 적용하고 persistence 전 redaction을 강제한다. 이 경계는 다른 일반 사용자에 대한 보호이며 관리자 또는 이미 침해된 current-user process에 대한 비밀 저장소를 주장하지 않는다.

### 프로젝트 증거

프로젝트별 실행 증거는 대상 프로젝트에 둔다.

    <project>\.ai-runs\star-control\runs\<run-id>\

Star-Control 저장소 자체가 아니라 실제 작업 대상 프로젝트에 기록한다.

### 여러 프로젝트 작업

전체 목표의 연결 정보와 `CoordinatedOperation`은 global store에 두고, project 상세 상태와 participant receipt는 각 project store, 변경·검사 evidence byte는 각 프로젝트 `.ai-runs/`에 둔다. 모든 project-scoped DB record는 ProjectId partition을 가지며 서로의 root binding과 절대 위치를 복제하지 않는다. cross-project 관계는 ProjectId, stable entity ID와 project-relative path만 사용한다.

## 실행 증거 폴더 예시

    <run-id>\
      goal.json
      plan.json
      capability-snapshot.json
      events.jsonl
      stages\
        <stage-id>\
          stage.json
          route.json
          context-summary.json
          permission-plan.json
          validation-plan.json
          result.json
          checkpoint.json
      evidence\
        changes.json
        validations.json
        cost.json
        risks.json
        final-summary.md
      merge\
        merge-plan.json
        conflicts.json
        result.json

Goal Run 밖의 CLI-only scan·change evidence는 별도 scope를 사용한다.

```text
<project>\.ai-runs\star-control\management\
  scans\<scan-run-id>\              # catalog/index refs·source manifest·freshness·coverage·scan report
  patches\<patch-set-id>\           # diff·rollback·apply report
  validations\<validation-result-id>\ # log·trace·report
```

이 폴더는 DB layout이 아니라 evidence export layout이다. 파일 이름은 export 구현에서 달라질 수 있지만 각 파일이 담는 의미는 [데이터 계약 지도](../contracts/README.md)의 Schema ID를 따른다. Controller가 event·projection을 commit한 뒤 export하며 export가 늦거나 손상되면 committed 계약과 ArtifactRef에서 다시 만든다.

## 저장 원칙

- Controller 하나만 management repository와 evidence index를 쓴다.
- event, projection, idempotency와 store revision은 같은 logical store 안에서 한 repository transaction으로 commit한다.
- cross-store command는 global prepared operation, project participant receipt와 final completion으로 복구하며 하나의 DB transaction이라고 주장하지 않는다.
- scan 결과는 invisible generation에 batch write한 뒤 complete finalization에서만 visible pointer를 바꾼다.
- ProjectCatalogSnapshot과 CodeIndexSnapshot은 immutable content fingerprint를 가지며 current pointer와 freshness probe 결과를 snapshot 본문과 분리한다.
- incomplete·failed generation, stale cache와 parse no-result는 이전 complete current generation을 교체하지 않는다.
- 중요한 store generation과 evidence manifest는 중간 상태가 보이지 않게 안전하게 교체한다.
- event export는 순서대로 추가하며 DB event revision과 hash를 기록한다.
- 잘못된 상태는 조용히 무시하지 않는다.
- 모르는 새 필드는 가능한 한 보존한다.
- DB와 evidence에는 secret, 사용자 이름, 개인 절대 경로와 민감 source literal을 저장하지 않는다.
- source file, 전체 diff, stdout·stderr와 trace를 DB blob에 넣지 않고 ArtifactRef로 연결한다.
- CLI, MCP와 향후 Codex entry adapter는 DB나 evidence file을 직접 열지 않고 같은 application service를 사용한다.

## 이어하기 기록

이어하기 기록에는 다음만 남긴다.

- 현재 목표와 단계
- 이미 끝난 결과
- 실패 원인과 시도한 방법
- 아직 남은 일
- 건드리면 안 되는 범위
- 관련 파일
- 다음 검사
- 다음 단계에 필요한 모델과 실행 방식
- 현재 작업 복사본과 병합 상태

전체 대화와 전체 로그를 다음 Codex에 그대로 전달하지 않는다.

## 보관 기간

보관 정책은 설정할 수 있다.

- 실행 중 기록: 삭제하지 않음
- 완료 요약과 핵심 증거: 장기 보관
- 큰 원문 로그: 설정된 기간 후 정리 가능
- 임시 파일: 안전한 종료 뒤 정리
- 실패 재현에 필요한 기록: 문제가 닫힐 때까지 보관

설계 기본값은 완료 run의 큰 원문·중간 artifact 90일, 해결된 실패 재현 자료 180일이다. 최종 요약·manifest, 실행 중 자료, 보존 hold와 미해결 실패 자료는 자동 정리하지 않는다. 공개 배포 전 실제 사용량을 측정해 기본값 변경이 필요한지 검토한다.

관리 DB는 latest complete generation, incomplete staging, scan detail, resolved Finding, local decision과 migration backup을 서로 다른 retention class로 관리한다. 정확한 기본값과 merge 전략은 [설정과 Catalog 계약](../contracts/config-and-catalog.md)이 소유한다. source, shared declaration과 `.ai-runs` byte는 DB retention이 삭제하지 않는다.

정리는 startup 또는 수동 command에서만 실행하며 자체 예약 실행을 만들지 않는다. 먼저 candidate와 protected reason을 담은 retention plan을 만들고 같은 store revision·plan fingerprint와 필요한 permission에서만 적용한다.

## backup·손상·재구축

- migration·repair·active generation 교체 전 store별 consistent backup을 만든다. 여러 store가 관련되면 global과 affected project generation의 hash·revision을 한 backup-set manifest로 고정한다.
- backend structural check, relation·partition, event/projection revision, fingerprint와 ArtifactRef hash를 계층적으로 검사한다.
- 손상이 의심되면 read-write open을 중단한다. Controller recovery component가 제시한 read-only mode, verified restore 또는 rebuild 중 활성화할 generation은 사용자가 선택하며 자동 전환하지 않는다.
- 손상 store를 덮어쓰지 않고 verified backup restore 또는 side-by-side rebuild를 수행한다.
- Git 선언·source와 같은 scan 입력이 있으면 current ProjectCatalogSnapshot, ProjectRevision, WorkspaceSnapshot, CodeIndexSnapshot, Symbol, Reference와 Finding projection을 재구축할 수 있다.
- `.ai-runs` canonical manifest가 남아 있으면 ValidationResult, GateDecision과 ArtifactRef relation을 제한적으로 reindex할 수 있다.
- local-only Suppression·Disposition, 진행 상태, 과거 actor·timestamp와 idempotency는 backup·export가 없으면 복구할 수 없다고 보고한다.
- 새 generation set 전체를 검증한 뒤에만 `active-set.json` pointer를 atomic replace하고 이전·손상 generation은 승인 전 삭제하지 않는다.

## 비밀정보

- 상태와 증거에 인증키 원문을 넣지 않는다.
- 환경 변수 값은 이름과 사용 여부만 기록한다.
- OS 사용자 이름, email과 개인 절대 경로를 저장하지 않는다.
- source literal은 message code와 redaction된 typed parameter로 바꾼다. secret·사용자 이름·raw 절대 경로·민감 literal 원문과 그 hash는 quarantined 상태에서도 저장하지 않는다.
- 외부로 내보낼 보고서는 한 번 더 가림 검사를 한다.
