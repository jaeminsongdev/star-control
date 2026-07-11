# 레거시 기능 카탈로그

## 문서의 역할

이 문서는 `legacy/`에 보관된 기존 Star-Control 자료가 설명한 기능을 개념 단위로 정리한 역사적 조사 기록이다. 새 Star-Control이 이 기능을 채택하는지는 이 문서에서 판단하지 않는다. 새 제품의 범위 판단은 [기능 범위와 레거시 판정](../product/scope.md)이 담당한다.

이 문서만 읽어도 기존 시스템이 어떤 문제를 다루고 어떤 흐름으로 연결되도록 설계됐는지 이해할 수 있게 작성했다. 근거 경로는 조사 내용을 다시 확인할 때만 사용하는 역사적 출처이며 새 설계의 계약이 아니다.

## 조사 범위와 제외 범위

| 구분 | 조사 내용 |
|---|---|
| 문서 | `legacy/docs/` 아래 Markdown 134개 |
| 구현 브리프 | `E01`부터 `E67`까지 번호가 붙은 브리프 67개와 안내 문서 1개 |
| 데이터 계약 | `legacy/specs/schemas/`의 JSON Schema 46개와 `legacy/specs/contracts/` |
| 설정 자료 | `legacy/configs/`의 파일 102개: 기계 판독 자료 74개와 설명용 Markdown 28개 |
| 예시 자료 | `legacy/examples/`의 파일 60개: 내용이 있는 예시 56개와 빈 폴더 보존 파일 4개 |
| 보조 근거 | 앱·패키지 README, provider·tool manifest, capability·registry 자료 |
| 조사하지 않은 것 | 소스 코드 내부 구조, 함수, 클래스, 실제 실행 결과와 작동 여부 |
| 판단하지 않은 것 | 기능의 효용성, 새 설계 채택 여부, 개발 진척 상태 |

같은 기능이 결정 기록, 로드맵, 계약, 브리프에 반복되면 하나의 기능으로 합쳤다. 반대로 같은 이름을 쓰더라도 사용자가 접하는 동작이나 입출력이 다르면 별도 세부 기능으로 나눴다.

## 항목을 나눈 기준

각 항목은 다음 질문에 서로 다른 답을 가지면 별도 기능으로 보았다.

1. 어떤 문제를 해결하려 했는가?
2. 사용자나 다른 기능에서 보이는 동작은 무엇인가?
3. 무엇을 받아 무엇을 만들거나 어떤 상태로 바꾸는가?
4. 어떤 기능의 결과를 받아 다음 기능으로 넘기는가?

고유 번호 `F01-01`의 앞부분은 상위 기능군, 뒷부분은 그 안의 세부 기능을 뜻한다.

## 전체 연결 흐름

```text
사용자 요청
  -> JobSpec
  -> Router가 RouteSpec과 WorkSpec 작성
  -> Provider 실행과 ProviderRunResult 기록
  -> ValidationEngine과 Star Sentinel 검사
  -> 자동 진행, 사람 승인 대기, 차단 중 하나로 판정
  -> Review Pack과 ReportSpec 작성
  -> Release Readiness, 복구, 감사 자료와 연결
```

핵심 실행 자료는 대상 프로젝트의 `.ai-runs/{job_id}/` 아래에 모으고, CLI·Daemon·API·UI가 같은 상태와 산출물을 서로 다른 방식으로 다루는 흐름이었다. 일부 운영 문서가 제시한 다른 저장 위치는 부록 D에 설명 차이로 따로 기록했다.

## F01. 설계 기준·결정 기록·로드맵 관리

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F01-01 | 기준 저장소와 정본 영역 | 문서, Schema, 설정, manifest의 기준이 여러 장소로 흩어지는 문제 | 저장소 안에서 자료 종류별 기준 위치와 소유 관계를 정한다. | 저장소 자료 → 기준 경로와 참조 관계 | F01-02, F03, F20 | `legacy/docs/decisions/0001-canonical-repository.md`, `legacy/docs/implementation/current-repository-map.md` |
| F01-02 | 결정 기록 | 중요한 구조 선택의 이유와 영향을 나중에 다시 확인하기 어려운 문제 | 결정별 배경, 선택, 결과를 번호가 붙은 문서로 남긴다. | 설계 쟁점 → 결정 문서 | 모든 기능군 | `legacy/docs/decisions/`, `legacy/docs/decisions/0001-canonical-repository.md` |
| F01-03 | 목표 구조와 단계별 로드맵 | 전체 구조와 개발 순서가 분리되어 작업 관계를 놓치는 문제 | 목표 구조, M0~M9 단계, 선행 관계와 산출물을 연결한다. | 목표·제약 → 단계·산출물·검증 순서 | F02~F20 | `legacy/docs/02_구현로드맵.md`, `legacy/docs/implementation/complete-implementation-roadmap.md`, `legacy/docs/implementation/target-architecture.md` |
| F01-04 | EPIC·브리프·작업 대기열 | 긴 작업에서 범위와 근거가 대화마다 달라지는 문제 | 큰 목표를 EPIC과 번호가 붙은 브리프로 나누고 작업·검증·인계 자료를 연결한다. | 목표·범위 → 브리프·대기열·감사 자료 | F20 | `legacy/docs/implementation/briefs/README.md`, `legacy/docs/implementation/codex-work-queue.md`, `legacy/docs/implementation/codex-long-run-workflow.md` |

## F02. 사용자 요청 정리와 Job 수명주기

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F02-01 | 사용자 요청의 Job 변환 | 자유 형식 요청을 추적 가능한 실행 단위로 다루기 어려운 문제 | 요청, 대상 프로젝트, 제약, 요청자를 `JobSpec`으로 고정한다. | 사용자 요청·프로젝트 → `JobSpec` | F03, F04, F05 | `legacy/docs/00_개요.md`, `legacy/docs/implementation/data-contracts.md` |
| F02-02 | 단계별 실행 흐름 | 계획, 실행, 검증, 보고 사이의 전달 관계가 모호해지는 문제 | `JobSpec -> RouteSpec -> WorkSpec -> ProviderRunResult -> ValidationDecision -> ReportSpec` 순서로 자료를 넘긴다. | Job 자료 → 단계별 계약 자료 | F05, F07, F09, F11 | `legacy/docs/implementation/target-architecture.md`, `legacy/docs/implementation/run-lifecycle.md` |
| F02-03 | 실행 상태와 단계 | 장시간 작업의 현재 위치와 다음 행동을 알기 어려운 문제 | 요청됨, 계획됨, 실행 중, 검증 중, 승인 대기와 종료 상태를 `RunState`에 기록한다. | 상태 전이 원인 → `RunState`·이벤트 | F04, F09, F11, F12~F15 | `legacy/docs/implementation/run-lifecycle.md`, `legacy/docs/implementation/data-contracts.md` |
| F02-04 | 종료·중단·재개 규칙 | 실패, 차단, 취소, 승인 대기를 같은 상태로 취급하는 문제 | 종료 상태와 다시 시작할 수 있는 상태를 구분하고, 재개 전에 필요한 조건을 확인한다. | 실패·차단·취소·승인 응답 → 종료 또는 재개 | F11, F12, F13, F18 | `legacy/docs/implementation/run-lifecycle.md`, `legacy/docs/implementation/state-store.md` |
| F02-05 | 작업 단위와 단계 선행 조건 | 실행자가 필요한 자료 없이 다음 단계로 넘어가는 문제 | 각 `WorkSpec`에 목표, 범위, 입력 artifact, 실행자, 검증 요구와 선행 단계를 적는다. | RouteSpec → 하나 이상의 WorkSpec | F05, F07, F09 | `legacy/docs/implementation/data-contracts.md`, `legacy/docs/implementation/run-lifecycle.md` |

## F03. Schema·데이터 계약·설정 병합

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F03-01 | Schema 버전과 변화 규칙 | 계약 형식이 바뀔 때 과거 자료의 의미가 사라지는 문제 | 주요 JSON 자료에 `schema_version`을 두고 변화 원칙을 정의한다. | JSON 자료 → 버전이 표시된 계약 | 모든 계약 기능 | `legacy/docs/implementation/data-contracts.md` |
| F03-02 | 공통 Schema 검사기 | 기능마다 JSON 검사 방식과 오류 표현이 달라지는 문제 | 지원하는 JSON Schema 규칙으로 자료를 검사하고 경로가 포함된 구조화 오류를 반환한다. | JSON·Schema → 성공 또는 오류 목록 | F04, F06~F19 | `legacy/docs/implementation/schema-validator.md`, `legacy/docs/implementation/briefs/E01-schema-validator.md` |
| F03-03 | 핵심 실행 계약 | Job, Route, Work, 상태, 보고, 이벤트의 필드 의미가 어긋나는 문제 | 핵심 자료별 Schema와 예시를 하나의 계약 묶음으로 정의한다. | 실행 자료 → `JobSpec`, `RouteSpec`, `WorkSpec`, `RunState`, `ReportSpec`, `CoreEvent` | F02, F04, F05, F11 | `legacy/docs/implementation/data-contracts.md`, `legacy/specs/schemas/` |
| F03-04 | 공통 참조와 오류 계약 | 산출물 위치와 실패 원인을 제각각 표현하는 문제 | `ArtifactRef`와 `ErrorEnvelope`로 경로·종류·오류 코드를 통일한다. | artifact·오류 → 공통 참조·오류 자료 | F04, F07, F12~F15 | `legacy/docs/implementation/data-contracts.md`, `legacy/specs/schemas/artifact-ref.schema.json`, `legacy/specs/schemas/error.schema.json` |
| F03-05 | Provider·검증 계약 | 실행자와 검사 도구의 결과를 core가 해석하지 못하는 문제 | Provider 결과, 진단, 검증 실행과 판정 자료의 공통 형식을 정의한다. | 실행·검사 결과 → 정규화된 계약 자료 | F06~F11 | `legacy/docs/implementation/data-contracts.md`, `legacy/specs/contracts/` |
| F03-06 | 계층별 설정 병합 | 저장소, 프로젝트, 사용자, 실행별 설정이 충돌하는 문제 | 정해진 우선순위로 설정을 합치고 최종 적용 값을 만든다. | repository·project·user·run 설정 → effective config | F05, F06, F09, F16~F19 | `legacy/docs/implementation/config-system.md`, `legacy/configs/` |
| F03-07 | 선언형 확장 자료 | 역할, 도구, 정책, Hook, Skill, Renderer를 코드 이름만으로 연결하는 문제 | 각 확장 요소를 Schema가 있는 선언 자료로 표현하고 참조 관계를 검사한다. | 선언 파일 → role·tool·policy·hook·skill·renderer 정보 | F05, F06, F10, F11 | `legacy/docs/implementation/config-system.md`, `legacy/specs/schemas/role.schema.json`, `legacy/specs/schemas/tool-manifest.schema.json` |
| F03-08 | Lifecycle Hook | route, provider 실행, 검증과 보고 전후의 반복 동작을 각 기능에 직접 넣는 문제 | lifecycle event에 schema 검사, rendering, event 추가, report 작성, tool 호출 같은 내부 step을 연결한다. | event·HookSpec → 순서가 정해진 내부 step | F02, F04, F07, F09, F11 | `legacy/docs/implementation/config-system.md`, `legacy/configs/hooks/` |
| F03-09 | Role 선언 | Router가 stage별 worker와 reviewer의 책임·필요 능력을 이름만으로 추측하는 문제 | 허용 stage, 필요한 capability와 기본 policy profile을 role에 선언한다. | RoleSpec → stage assignment 조건 | F05, F06 | `legacy/docs/implementation/config-system.md`, `legacy/configs/roles/` |
| F03-10 | Artifact Renderer | 구조화 JSON artifact를 사람이 직접 해석해야 하는 문제 | ReportSpec, ReviewPack, ApprovalRequest를 template에 따라 Markdown이나 text로 변환한다. | JSON artifact·RendererSpec → 사람용 문서 | F08, F11, F12, F15 | `legacy/docs/implementation/config-system.md`, `legacy/configs/renderers/` |
| F03-11 | Skill 선언 | repository 요약, diff 요약, report rendering, Schema 검사 같은 반복 절차의 입출력과 안전 조건이 흩어지는 문제 | 재사용 기능의 입력, 출력과 승인 필요 조건을 `SkillSpec`으로 선언한다. | SkillSpec·입력 artifact → 선언된 출력 artifact | F05, F07, F09, F16, F20 | `legacy/docs/implementation/config-system.md`, `legacy/configs/skills/` |
| F03-12 | Tool Manifest·Adapter 계약 | core가 외부 도구의 명령과 전용 입출력 형식을 직접 알아야 하는 문제 | tool ID, 명령, 입력·출력 Schema를 manifest로 선언하고 adapter 경계로 호출한다. | tool manifest·input artifact → tool result·output artifact | F09, F10 | `legacy/specs/contracts/tool-adapter.md`, `legacy/specs/schemas/tool-manifest.schema.json` |
| F03-13 | Artifact·Prompt Template | route, WorkSpec, 승인 요청과 보고서의 기본 뼈대가 작성자마다 달라지는 문제 | 구조화 자료와 사람용 문서의 기본 필드·문구를 template으로 제공한다. | template·run context → 초안 artifact | F01, F05, F11 | `legacy/configs/templates/` |

## F04. StateStore·실행 산출물·이벤트 기록

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F04-01 | 프로젝트별 실행 저장소 | 별도 DB나 특정 daemon 없이는 작업 기록을 읽지 못하는 문제 | 대상 프로젝트의 `.ai-runs/{job_id}/`를 실행 기록의 기준 위치로 사용한다. | project root·job id → 실행 폴더 | 모든 runtime 기능 | `legacy/docs/implementation/state-store.md`, `legacy/docs/operations/run-artifacts.md` |
| F04-02 | 사람이 추적할 수 있는 Job ID | 실행 폴더를 서로 구분하고 순서를 알아보기 어려운 문제 | 프로젝트 안에서 `J-NNNN` 형태의 다음 Job ID를 배정한다. | project root → 새 job id | F02, F12~F15 | `legacy/docs/implementation/state-store.md` |
| F04-03 | 표준 산출물 트리 | 기능별 파일이 임의 위치와 이름으로 저장되는 문제 | job, state, route, work, provider, validation, approval, review pack, report, audit, release 자료의 위치를 정한다. | 단계별 결과 → 표준 파일 경로 | F07~F19 | `legacy/docs/implementation/artifact-layout.md`, `legacy/docs/implementation/artifact-naming.md` |
| F04-04 | 원자적 JSON 저장 | 저장 중 중단되어 기존 JSON까지 손상되는 문제 | 임시 파일을 작성한 뒤 같은 파일시스템 안에서 교체한다. | JSON 자료 → 전체가 쓰였거나 기존 값이 남은 파일 | F02, F06~F19 | `legacy/docs/implementation/state-store.md`, `legacy/docs/implementation/artifact-layout.md` |
| F04-05 | 추가 전용 실행 이벤트 | 상태 변경 이유가 최종 JSON에 덮여 사라지는 문제 | 실행 흐름 사건을 `events.jsonl` 뒤에 순서대로 추가한다. | 상태·단계 사건 → `CoreEvent` 행 | F02, F07, F09, F13 | `legacy/docs/implementation/state-store.md`, `legacy/docs/implementation/artifact-layout.md` |
| F04-06 | Artifact Registry | 산출물 파일과 그 의미·생성 주체의 관계를 찾기 어려운 문제 | artifact의 종류, 경로, 작성 주체와 관련 단계를 목록으로 관리한다. | 생성된 파일 → ArtifactRef·registry 항목 | F07~F19 | `legacy/docs/implementation/artifact-layout.md` |
| F04-07 | 경로 이탈 차단 | 잘못된 상대 경로가 프로젝트 밖 파일을 읽거나 쓰는 문제 | 모든 artifact 경로가 허용된 root 안에 있는지 확인한다. | 요청 경로 → 허용된 경로 또는 구조화 오류 | F07, F12~F19 | `legacy/docs/implementation/state-store.md` |
| F04-08 | 손상 자료의 가시화 | 파싱 실패나 일부 손상을 빈 상태로 오인하는 문제 | 읽기 실패를 숨기지 않고 손상 위치와 종류를 복구 검사로 넘긴다. | 손상 JSON·event log → 명시적 오류·복구 issue | F18 | `legacy/docs/implementation/state-store.md`, `legacy/docs/implementation/state-store-recovery.md` |
| F04-09 | Job 실행 Lock | 같은 Job과 stage를 여러 process가 동시에 바꿔 상태와 artifact가 충돌하는 문제 | 단일 process 전제와 중복 재실행 차단을 기본 경계로 설명하고, 확장 형태로 PID·시간·명령·hostname을 담은 `run.lock`을 제시한다. | job·stage·process 정보 → 실행 허용 또는 lock 충돌 | F07, F13 | `legacy/docs/implementation/state-store.md` |

## F05. Router·위험 분류·단계와 실행자 배정

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F05-01 | 요청 특성 분류 | 모든 요청을 같은 작업 흐름과 검사 강도로 처리하는 문제 | 변경 종류, 크기, 위험, 요청 제약을 규칙으로 분류한다. | JobSpec·정책 → classification | F05-02, F05-03, F09 | `legacy/docs/implementation/router-engine.md`, `legacy/docs/implementation/router-decision-matrix.md` |
| F05-02 | 정책 Profile 선택 | 요청 성격과 관계없이 같은 정책을 적용하는 문제 | 분류 결과에 맞는 실행·검증 profile을 고른다. | classification·policy → profile | F03, F09, F10 | `legacy/docs/implementation/router-engine.md`, `legacy/docs/implementation/policy-profiles.md` |
| F05-03 | 자동 진행·사람 검토·차단 결정 | 위험한 변경이 승인 지점 없이 실행되거나 안전한 흐름도 멈추는 문제 | 규칙에 따라 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK`과 승인 이유를 작성한다. | risk·change type·policy → router decision | F09, F11 | `legacy/docs/implementation/router-engine.md`, `legacy/docs/implementation/router-decision-matrix.md` |
| F05-04 | 단계 계획 | 여러 성격의 작업을 한 번에 실행하거나 순서를 잃는 문제 | route, plan, design, implement, validate, review, polish, report 단계와 선행 관계를 만든다. | JobSpec·classification → RouteSpec 단계 | F02, F07, F09 | `legacy/docs/implementation/router-engine.md` |
| F05-05 | Capability·Role 기반 배정 | 특정 제품명만으로 실행자를 선택해 필요한 능력을 확인하지 못하는 문제 | 단계와 role이 요구하는 capability를 provider 후보와 대조한다. | stage·role·capability registry → provider assignment | F06, F07 | `legacy/docs/implementation/router-engine.md`, `legacy/docs/providers/provider-capability.md` |
| F05-06 | Budget 후보 계산 | 외부 실행 전에 비용 한도 영향을 알기 어려운 문제 | route 수준에서 예상 예산과 경고·승인 조건을 붙인다. | JobSpec·provider 정보·budget policy → budget metadata | F08, F17 | `legacy/docs/implementation/router-engine.md`, `legacy/docs/implementation/security-cost-observability.md` |
| F05-07 | 결정적 Routing | 같은 입력이 매번 다른 계획을 만들어 재현하기 어려운 문제 | 동일한 JobSpec, registry, policy에는 같은 RouteSpec을 만들고 애매한 경우 오류를 낸다. | 동일 입력 → 동일 RouteSpec 또는 명시 오류 | F20 | `legacy/docs/implementation/router-engine.md` |
| F05-08 | Model Registry·Tier·Fallback Routing | 역할과 stage마다 쓸 실행 등급과 대체 경로를 매번 직접 고르는 문제 | model ID를 provider kind와 routing tag에 연결하고 role별 preferred tier, fallback, 편집 권한, 최대 위험과 작업 크기별 stage 기본값을 정의한다. | model registry·role·size·risk → model tier와 fallback 후보 | F03, F06, F08 | `legacy/configs/registries/model-registry.yaml`, `legacy/configs/policies/model-routing.yaml` |

## F06. Provider 추상화·Registry·Capability·Readiness

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F06-01 | Provider 중립 모델 | 실행자마다 다른 이름과 형식을 core가 직접 알아야 하는 문제 | `kind`, `transport`, `adapter`, `capability`, `instance`를 분리해 공통 모델로 표현한다. | provider 자료 → 공통 Provider 계약 | F05, F07, F08 | `legacy/docs/providers/provider-model.md`, `legacy/docs/implementation/provider-system.md` |
| F06-02 | Builtin Provider Registry | 사용할 수 있는 provider와 관련 계약 파일을 한곳에서 찾기 어려운 문제 | provider manifest, capability profile, adapter와 transport 참조를 등록소에 모은다. | registry 파일 → provider 목록·상세 정보 | F05, F12, F15 | `legacy/docs/providers/provider-registry.md`, `legacy/configs/registries/builtin-provider-registry.yaml` |
| F06-03 | Capability Profile | provider가 할 수 있는 편집, shell, repository 읽기, JSON, offline 작업을 이름만으로 추측하는 문제 | 능력과 제약을 명시한 profile을 provider에 연결한다. | capability 선언 → 검색 가능한 능력 목록 | F05, F07 | `legacy/docs/providers/provider-capability.md`, `legacy/configs/registries/capability-registry.yaml`, `legacy/builtin-providers/` |
| F06-04 | Provider Instance | 같은 provider라도 endpoint, command, 한도와 credential이 실행 환경마다 다른 문제 | manifest와 별도로 실행별 연결 값을 저장하고 명시적으로 선택한다. | endpoint·command·limit·credential ref → instance 파일 | F07, F08, F12, F15 | `legacy/docs/implementation/provider-system.md`, `legacy/specs/schemas/provider-instance.schema.json` |
| F06-05 | Registry 참조 검사 | 존재하지 않는 adapter, transport, capability를 manifest가 가리키는 문제 | 등록소 안의 ID와 참조 관계를 교차 검사한다. | registry·manifest → 검색 결과 또는 계약 오류 | F03, F20 | `legacy/docs/implementation/provider-system.md`, `legacy/docs/implementation/ci-contract-validation.md` |
| F06-06 | Provider 계약 적합성 검사 | ProviderRunResult와 실제 artifact가 다른 실행을 가리키는 문제 | 공통 result, ArtifactRef와 필수 sidecar의 일치 여부를 검사한다. | provider result·artifacts → 적합성 결과 | F07, F09, F20 | `legacy/docs/implementation/provider-system.md`, `legacy/specs/contracts/provider-adapter.md` |
| F06-07 | Offline Readiness·Healthcheck | 실제 실행을 시작하기 전에 설정 누락과 정책 위반을 찾기 어려운 문제 | manifest와 instance를 읽어 파일·필드·정책 준비 상태를 검사하며 외부 호출은 하지 않는다. | provider instance → readiness 결과 | F12, F15, F20 | `legacy/docs/implementation/briefs/E48-provider-offline-readiness-healthcheck.md`, `legacy/docs/implementation/cli-command-reference.md` |
| F06-08 | 실행 환경 Capability Registry | provider·agent 환경이 제공하는 문맥, 계획, 안전, 위임, 도구, 검색, Git, 검토, 비용 기능을 같은 기준으로 비교하기 어려운 문제 | capability를 범주와 필수 여부로 등록해 profile과 routing 조건에서 참조한다. | capability registry·provider profile → 환경 능력 정보 | F03, F05, F20 | `legacy/configs/registries/capability-registry.yaml`, `legacy/specs/contracts/capability-registry.md` |
| F06-09 | Provider 공식 근거 Snapshot·Refresh | 외부 provider의 endpoint, 인증, streaming, JSON, tool call과 usage 계약이 바뀌는 문제 | 확인 날짜와 공식 문서 근거를 snapshot으로 남기고 adapter 작업 전에 항목별 refresh checklist를 수행한다. | 공식 문서·확인 날짜 → provider snapshot·manifest 갱신 후보 | F01, F03, F20 | `legacy/docs/providers/provider-reference-snapshots.md` |

## F07. Provider 실행 조정·결과 정규화

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F07-01 | WorkSpec 로딩과 실행 요청 고정 | 실행 중 입력이 달라져 결과를 재현하지 못하는 문제 | WorkSpec과 provider instance를 읽고 실행 시점의 `ExecutionRequest`를 artifact로 남긴다. | WorkSpec·instance → ExecutionRequest | F04, F06, F08 | `legacy/docs/implementation/execution-engine.md`, `legacy/specs/schemas/execution-request.schema.json` |
| F07-02 | Adapter 실행 경계 | core가 provider별 명령, HTTP, 파일 전달 방식을 직접 처리하는 문제 | adapter의 prepare, execute, cancel, collect, healthcheck 단계로 실행을 감싼다. | ExecutionRequest → adapter 실행 | F06, F08 | `legacy/docs/implementation/provider-system.md`, `legacy/specs/contracts/provider-adapter.md` |
| F07-03 | 결과 정규화 | stdout, HTTP 응답, 파일 결과를 다음 단계가 각각 다르게 해석하는 문제 | provider별 결과를 `ProviderRunResult`와 공통 artifact 참조로 변환한다. | raw output → ProviderRunResult·artifacts | F09, F11, F17 | `legacy/docs/implementation/execution-engine.md`, `legacy/specs/schemas/provider-run-result.schema.json` |
| F07-04 | 실행 시도 기록 | 재시도 결과가 이전 실행과 섞여 실패 원인을 잃는 문제 | 시도 번호, 시작·종료, 오류와 산출물을 `ExecutionAttempt`로 구분한다. | 실행 시도 → attempt 자료·별도 artifact 이름 | F04, F17 | `legacy/docs/implementation/execution-engine.md`, `legacy/docs/implementation/artifact-naming.md` |
| F07-05 | Timeout·Retry·Cancel | 멈춘 실행이 무기한 남거나 재시도가 통제되지 않는 문제 | 정책에 따라 timeout, 제한된 재시도와 취소 전달을 수행한다. | 실행 상태·retry policy·cancel → 재시도·취소·실패 결과 | F02, F13, F17 | `legacy/docs/implementation/execution-engine.md`, `legacy/configs/policies/retry-policy.yaml` |
| F07-06 | 중복 실행과 덮어쓰기 방지 | 같은 요청이 두 번 실행되거나 기존 증거가 조용히 바뀌는 문제 | idempotency key와 기존 artifact 확인으로 중복 실행과 무표시 교체를 막는다. | ExecutionRequest·기존 artifact → 재사용·명시 오류·새 attempt | F04, F18 | `legacy/docs/implementation/execution-engine.md` |
| F07-07 | 출력 계약 검사 | provider가 성공을 반환해도 필수 파일이나 필드가 빠지는 문제 | result Schema와 필수 artifact를 검사한 뒤 다음 단계로 넘긴다. | ProviderRunResult·artifacts → 검사 결과 | F03, F06, F09 | `legacy/docs/implementation/execution-engine.md` |
| F07-08 | 금지 행동 Guard | WorkSpec 범위 밖의 위험 행동이 provider 실행으로 넘어가는 문제 | 실행 직전에 금지 행동과 승인 요구를 다시 확인한다. | WorkSpec·policy → 실행 허용 또는 차단 | F05, F11, F16 | `legacy/docs/implementation/execution-engine.md`, `legacy/docs/implementation/security-cost-observability.md` |

## F08. Fake·Local·Cloud·Human Provider 경로

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F08-01 | Fake Provider | 외부 프로그램과 네트워크 없이 전체 실행 흐름을 반복 검사하기 어려운 문제 | 같은 WorkSpec에 결정적인 request, response, 비용 자료를 생성한다. | WorkSpec → deterministic ProviderRunResult·artifacts | F07, F13, F20 | `legacy/docs/decisions/0003-fake-provider-instance.md`, `legacy/docs/implementation/provider-system.md` |
| F08-02 | Local Process Provider | 로컬 명령 실행의 대상과 인자를 통제하고 결과를 보존해야 하는 문제 | allowlist에 있는 실행 파일을 shell 없이 호출하고 stdout, stderr, timeout, cancel을 기록한다. | command vector·WorkSpec → process result·sidecars | F07, F13, F17 | `legacy/docs/implementation/local-process-provider-policy.md` |
| F08-03 | Local OpenAI-compatible Loopback | 로컬 모델 서버를 공통 request·response 계약으로 명시 실행하려는 문제 | loopback endpoint의 instance를 선택해 HTTP 요청과 원문 응답, 정규화 결과를 기록한다. | loopback endpoint·request → HTTP·response·cost artifacts | F06, F07, F15 | `legacy/docs/implementation/provider-system.md`, `legacy/apps/star-control-ui/README.md` |
| F08-04 | Cloud Provider 사전 점검 | 외부 전달 전에 credential, privacy 승인과 budget 조건을 확인해야 하는 문제 | raw credential을 받지 않고 참조만 사용하며 handoff 범위와 예상 비용을 검사한다. | credential ref·privacy·budget → 실행 준비 또는 차단 자료 | F16, F17 | `legacy/docs/implementation/cloud-provider-policy.md`, `legacy/docs/implementation/briefs/E12-cloud-provider-preflight.md` |
| F08-05 | Cloud CLI Transport | cloud 도구를 shell 문자열로 호출해 인자와 결과 경계가 흐려지는 문제 | 승인된 command vector와 request placeholder를 사용하고 stdout, stderr, timeout을 수집한다. | command·request file → CLI result artifacts | F06, F07 | `legacy/docs/implementation/cloud-provider-policy.md`, `legacy/docs/implementation/briefs/E13-cloud-cli-transport.md` |
| F08-06 | OpenAI-compatible 요청·응답 변환 | HTTP 요청 형식과 provider 원문 응답을 core 계약으로 바꿔야 하는 문제 | request builder와 response parser로 공통 입력·출력 사이를 변환한다. | ExecutionRequest·raw response → HTTP body·ProviderRunResult | F07 | `legacy/docs/implementation/briefs/E15-openai-compatible-parser.md`, `legacy/docs/implementation/briefs/E16-openai-compatible-request-builder.md` |
| F08-07 | Cloud API Offline Fixture | 유료·외부 호출 없이 request와 parser 경계를 검사해야 하는 문제 | 고정 fixture로 요청, 원문 응답, 파싱 결과와 비용 자료를 만든다. | fixture·request → offline response artifacts | F17, F20 | `legacy/docs/implementation/briefs/E17-cloud-api-offline-fixture.md`, `legacy/docs/implementation/cloud-provider-policy.md` |
| F08-08 | Cloud API Transport Plan과 Live Gate | 실제 HTTP 호출의 목적지와 승인 근거가 실행 전에 보이지 않는 문제 | 전송 계획 artifact를 만들고 live intent에는 명시적 승인 자료를 요구한다. | endpoint·handoff·approval → transport plan·승인 대기 또는 차단 | F11, F16, F17 | `legacy/docs/implementation/briefs/E18-cloud-api-transport-boundary.md`, `legacy/docs/implementation/briefs/E19-cloud-api-live-approval-gate.md` |
| F08-09 | Human Handoff | 자동 실행으로 처리하지 않을 판단을 같은 수명주기에서 이어가야 하는 문제 | 사람을 provider 종류로 표현하고 ApprovalRequest를 만든 뒤 응답을 기다린다. | WorkSpec·승인 사유 → 승인 대기·ApprovalResponse | F02, F11 | `legacy/docs/implementation/provider-system.md`, `legacy/specs/schemas/approval-request.schema.json` |
| F08-10 | Anthropic-compatible·Remote Self-hosted 경로 | OpenAI-compatible 이외의 로컬 서버와 원격 자체 운영 모델·agent를 provider kind로 표현하려는 문제 | `local_anthropic_compatible_server`와 `remote_self_hosted_model`을 kind 후보로 두고 capability와 routing tag로 세부 배정을 구분한다. | server·remote instance·capability → provider 후보 | F05, F06, F07 | `legacy/docs/implementation/provider-system.md`, `legacy/specs/schemas/provider-kind.schema.json` |

## F09. ValidationEngine·검증 결과 전달

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F09-01 | 검증 요구 수집 | 실행 결과에 어떤 검사가 필요한지 단계마다 달라지는 문제 | WorkSpec, route profile과 변경 내용을 바탕으로 검증 요구를 모은다. | ProviderRunResult·WorkSpec·policy → validation requirements | F05, F10 | `legacy/docs/implementation/validation-engine.md` |
| F09-02 | 검사 실행 조정 | 여러 검사 명령과 Sentinel 결과를 하나의 판정으로 연결하기 어려운 문제 | 검증 실행을 호출하고 결과, 종료 코드, artifact를 `ValidationRun`으로 모은다. | requirements·provider output → ValidationRun 목록 | F10, F20 | `legacy/docs/implementation/validation-engine.md` |
| F09-03 | Sentinel 호출 경계 | core와 정책 검사 도구가 서로의 내부 구조에 결합되는 문제 | 정해진 tool request와 result 계약으로 Star Sentinel을 호출한다. | 변경·정책·검증 자료 → Sentinel 결과 | F10 | `legacy/docs/implementation/validation-engine.md`, `legacy/docs/tools/star-sentinel.md` |
| F09-04 | Diagnostic 통합 | 검사 도구마다 위치, 심각도, 이유 표현이 다른 문제 | 진단을 공통 `Diagnostic` 형식으로 정규화하고 중복을 정리한다. | tool diagnostics → Diagnostic 목록 | F10, F11 | `legacy/docs/implementation/validation-engine.md`, `legacy/specs/schemas/diagnostic.schema.json` |
| F09-05 | ValidationDecision과 상태 전달 | 검사 결과와 Job 상태가 따로 움직이는 문제 | 결과를 판정으로 바꾸고 `VALIDATED`, `WAITING_APPROVAL`, `BLOCKED`, `FAILED` 상태와 후속 artifact를 함께 기록한다. | ValidationRun·Diagnostic → ValidationDecision·RunState | F02, F11 | `legacy/docs/implementation/validation-handoff.md`, `legacy/docs/implementation/validation-engine.md` |
| F09-06 | 검증 증거 원장 | 어떤 검사와 근거가 판정에 쓰였는지 사라지는 문제 | 검사 명령, 결과, artifact와 판정 관계를 원장에 기록한다. | validation evidence → ledger·ArtifactRef | F10, F11, F19, F20 | `legacy/docs/implementation/validation-engine.md` |

## F10. Star Sentinel 정책 검사

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F10-01 | `check` | AI 변경이 요청 범위와 안전 규칙을 지켰는지 근거로 검사해야 하는 문제 | repository map, 변경 줄, provider output, validation과 policy를 읽어 진단을 만든다. | task·diff·evidence·policy → diagnostics·validation runs | F09, F10-02 | `legacy/docs/tools/star-sentinel.md`, `legacy/docs/implementation/star-sentinel-full-spec.md` |
| F10-02 | `gate` | 진단 목록만으로 자동 진행 여부를 일관되게 정하기 어려운 문제 | profile과 증거를 평가해 `AUTO_PASS`, `HUMAN_REVIEW`, `BLOCK` 판정을 만든다. | diagnostics·evidence·profile → gate decision·approval | F09, F11 | `legacy/docs/implementation/star-sentinel-full-spec.md` |
| F10-03 | `review-pack` | 사람이 변경 범위, 위험과 검사 근거를 여러 파일에서 모아야 하는 문제 | 판정과 증거를 JSON과 Markdown 검토 묶음으로 만든다. | decision·changes·risks·validations → review pack | F11 | `legacy/docs/implementation/star-sentinel-full-spec.md` |
| F10-04 | `selfcheck` | 검사 도구 자신의 manifest, policy, Schema가 어긋나 검사 신뢰가 깨지는 문제 | builtin tool 자료의 파싱, Schema, 중복 ID와 명칭을 검사한다. | Sentinel 자산 → selfcheck 결과 | F03, F20 | `legacy/docs/implementation/star-sentinel-full-spec.md` |
| F10-05 | P0 안전 규칙 | 범위 위반, 테스트 삭제, 의존성 무단 변경, 평문 비밀정보, 검사 우회를 놓치는 문제 | changed lines와 task scope를 규칙별로 평가해 진단과 gate 신호를 만든다. | diff·scope·claims → rule diagnostics | F16, F20 | `legacy/docs/decisions/0004-star-sentinel-p0-scope.md`, `legacy/docs/implementation/star-sentinel-p0-contracts.md` |
| F10-06 | 검사 Profile | 모든 변경에 같은 검사 집합을 적용하는 문제 | quick, near, full, security, release, validator profile에 rule set을 연결한다. | profile → 적용 규칙과 검사 요구 | F05, F09, F19 | `legacy/docs/implementation/policy-profiles.md`, `legacy/docs/implementation/star-sentinel-full-spec.md` |
| F10-07 | Sentinel Ledger | 정책 검사부터 gate와 review pack까지의 사건 관계를 추적하기 어려운 문제 | 검사, 판정, review pack, artifact 사건을 추가 전용 `ledger.jsonl`에 기록한다. | Sentinel 사건 → ledger 행 | F09, F11, F17 | `legacy/docs/implementation/star-sentinel-full-spec.md` |
| F10-08 | 증거·보고 일치와 검사기 변경 규칙 후보 | 테스트 무력화, 근거 없는 검증 주장, 실제 diff와 다른 보고, 검사 정책 변경을 P0 규칙만으로 다루지 못하는 문제 | skip·only·ignore 추가, validation evidence 누락, changed files 불일치, validator policy 변경을 각각 사람 검토·차단·승인 후보로 판정하는 확장 규칙을 설명한다. | diff·report·validation evidence·validator changes → P1 diagnostic 후보 | F09, F11, F20 | `legacy/docs/implementation/star-sentinel-full-spec.md` |

## F11. 승인·Review Pack·최종 보고

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F11-01 | ApprovalRequest | 자동 진행을 멈춘 이유와 사람이 결정할 내용을 전달하기 어려운 문제 | 요청 행동, 이유, 위험, 증거와 허용 선택지를 구조화한다. | route·validation decision·diagnostics → approval request | F05, F09, F10 | `legacy/docs/implementation/approval-review-flow.md`, `legacy/specs/schemas/approval-request.schema.json` |
| F11-02 | ApprovalResponse | 승인자의 결정과 조건을 실행 흐름에 안전하게 반영해야 하는 문제 | 승인, 거절, 조건과 응답자를 기록하고 원래 요청과 연결한다. | approval request·사람 응답 → approval response | F02, F07, F12~F15 | `legacy/docs/implementation/approval-review-flow.md`, `legacy/specs/schemas/approval-response.schema.json` |
| F11-03 | 조건부 승인 전파 | 승인 조건이 다음 WorkSpec과 실행 과정에서 사라지는 문제 | 허용 범위, 시간, 비용, 행동 조건을 후속 실행 자료에 전달한다. | approval constraints → 제한이 붙은 실행·검증 | F07, F09, F16, F17 | `legacy/docs/implementation/approval-review-flow.md` |
| F11-04 | 판정 우선순위 | Router와 Validation, Sentinel 판정이 다를 때 어느 결정을 따를지 모호한 문제 | 차단, 사람 검토, 자동 진행의 우선순위로 최종 행동을 정한다. | 여러 decision → effective decision | F05, F09, F10 | `legacy/docs/implementation/approval-review-flow.md`, `legacy/docs/implementation/validation-handoff.md` |
| F11-05 | Review Pack | 사람이 변경과 위험, 검사 자료를 여러 artifact에서 직접 모아야 하는 문제 | 변경 요약, 파일, 위험, 진단, 검증, 승인과 다음 행동을 JSON·Markdown으로 묶는다. | run evidence → review pack | F10, F12~F15, F19 | `legacy/docs/implementation/approval-review-flow.md`, `legacy/docs/implementation/briefs/E38-release-review-pack-foundation.md` |
| F11-06 | Review Pack Handoff | 검증 도구가 만든 검토 자료의 위치를 core가 알기 어려운 문제 | review pack 파일과 판정, 관련 artifact를 `ReviewPackHandoff`로 전달한다. | review pack artifacts → handoff 자료 | F09, F10 | `legacy/docs/implementation/validation-engine.md`, `legacy/specs/schemas/review-pack-handoff.schema.json` |
| F11-07 | ReportSpec과 단계별 보고 | 최종 결과만 남아 어떤 단계와 증거를 거쳤는지 알기 어려운 문제 | 단계 결과, 상태, 변경, 검증, 비용, 위험과 artifact 참조를 보고서로 만든다. | Job·RunState·evidence → stage 또는 final ReportSpec | F04, F12~F15, F17, F19 | `legacy/docs/implementation/data-contracts.md`, `legacy/specs/schemas/report.schema.json` |
| F11-08 | 승인 감사 연결 | 승인 요청과 응답, 후속 행동의 관계가 끊기는 문제 | 각 승인 사건을 execution event와 audit event, 보고서에 연결한다. | approval 사건 → event·audit·report 참조 | F04, F17 | `legacy/docs/implementation/approval-review-flow.md`, `legacy/docs/implementation/security-privacy-observability-contracts.md` |

## F12. CLI 실행·조회·제어

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F12-01 | `run` | 파일 기반 관제 흐름을 한 명령에서 시작하기 어려운 문제 | 요청과 프로젝트를 받아 Job, Route, Work, provider 실행 또는 dry-run, 검증·보고 후보를 만든다. | project·request·provider instance → job artifacts·CLI 응답 | F02, F05, F07~F11 | `legacy/docs/implementation/cli-command-reference.md`, `legacy/docs/implementation/briefs/E08-cli-fake-flow.md` |
| F12-02 | `status` | Job의 상태와 단계, 대기 이유를 파일을 직접 열지 않고 확인하려는 문제 | RunState, stage, provider와 다음 행동을 읽어 표시한다. | project·job → 상태 출력 | F02, F04 | `legacy/docs/implementation/cli-command-reference.md` |
| F12-03 | `report` | 단계별·최종 보고서와 release 자료를 명령으로 조회하려는 문제 | 지정한 report artifact를 읽어 사람용 또는 JSON 형식으로 출력한다. | project·job·stage → report 출력 | F11, F16, F19 | `legacy/docs/implementation/cli-command-reference.md` |
| F12-04 | `approve`·`cancel`·`resume` | 승인 대기와 장시간 Job을 터미널에서 제어해야 하는 문제 | 응답 artifact와 event를 쓰고 수명주기 조건에 맞게 상태를 바꾼다. | job·action·response → approval·event·RunState | F02, F11, F13, F17 | `legacy/docs/implementation/cli-command-reference.md`, `legacy/docs/implementation/briefs/E20-cli-control-commands.md` |
| F12-05 | `release` | 배포 관련 증거와 실행 계획을 터미널에서 확인하려는 문제 | readiness를 읽고 action별 dry-run 계획, 승인 요구나 local result를 표시한다. | job·release action·token → plan·result·차단 응답 | F19 | `legacy/docs/implementation/cli-command-reference.md`, `legacy/docs/implementation/briefs/E37-release-readiness-cli-read.md` |
| F12-06 | `recover` | 손상 검사와 복구 계획·행동을 같은 명령 계약으로 다루려는 문제 | inspect, dry-run, action, source, target과 token을 받아 복구 artifact를 만든다. | job·recovery option → inspection·plan·result | F18 | `legacy/docs/implementation/cli-command-reference.md`, `legacy/docs/implementation/briefs/E39-recovery-command-surface.md` |
| F12-07 | `providers` | provider 등록소와 instance 준비 상태를 터미널에서 확인하려는 문제 | list, show, healthcheck로 계약과 offline readiness를 읽는다. | registry·provider id·instance → provider 정보·readiness | F06 | `legacy/docs/implementation/cli-command-reference.md`, `legacy/docs/implementation/briefs/E44-cli-providers-read-only.md` |
| F12-08 | `sentinel` | 정책 검사 도구를 core 실행과 별도로 직접 호출하려는 문제 | selfcheck, check, gate, review-pack 하위 명령을 같은 CLI 규칙으로 제공한다. | Sentinel options → diagnostics·decision·review pack | F10 | `legacy/docs/implementation/cli-command-reference.md`, `legacy/docs/implementation/briefs/E45-cli-sentinel-command-group.md` |
| F12-09 | 사람용·JSON 출력과 Exit Code | 스크립트와 사람이 같은 명령을 사용할 때 결과 해석이 달라지는 문제 | 성공·오류 envelope, JSON 모드와 명령별 종료 코드를 정의한다. | 명령 결과 → human text 또는 schema-valid CLI envelope | F03, F14 | `legacy/docs/implementation/cli-command-reference.md`, `legacy/specs/schemas/cli-output.schema.json` |
| F12-10 | `init` | 사용자 전역 설정과 프로젝트별 실행 폴더의 초기 위치를 매번 직접 만드는 문제 | global config root와 project root를 초기화하는 명령 형태를 제시한다. | global 또는 project path → 초기 설정·폴더 구조 | F03, F04 | `legacy/docs/operations/Star-Control_MVP_Runbook.md` |
| F12-11 | `validate schemas`·`validate policies` | 실행 전에 계약과 정책 자료의 형식 오류를 따로 찾기 어려운 문제 | Schema와 policy 자료를 선택해 검사하는 명령 흐름을 제시한다. | config root·검사 종류 → validation result | F03, F09, F20 | `legacy/docs/operations/Star-Control_MVP_Runbook.md` |
| F12-12 | `render --dry-run`·`render --apply` | provider·agent용 설정 결과를 적용 전에 보고 필요할 때만 쓰려는 문제 | 대상 renderer의 예상 결과를 dry-run으로 보여주고 별도 apply 명령 형태를 제시한다. | renderer target·effective config → preview 또는 rendered files | F03, F06 | `legacy/docs/operations/Star-Control_MVP_Runbook.md` |
| F12-13 | `config inspect` 후보 | 계층별 설정 병합 결과와 출처를 확인하기 어려운 문제 | 병합된 설정과 원본 계층을 조회하는 CLI 후보를 문서에 남긴다. | repository·project·user·run config → effective config 설명 | F03 | `legacy/docs/implementation/config-system.md` |

## F13. Daemon·Queue·Scheduler

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F13-01 | Daemon Process | CLI 호출 사이에도 queue와 실행 제어 상태를 공유하려는 문제 | daemon 상태를 읽고 `status`와 제한된 tick 수의 `serve` 동작, loopback API를 제공한다. | daemon config·명령 → daemon status·serve result | F12, F14, F15 | `legacy/docs/implementation/daemon-contract.md`, `legacy/apps/star-daemon/README.md` |
| F13-02 | Daemon State와 Queue | 여러 Job의 대기·활성 상태와 선택 provider를 한곳에서 관리하려는 문제 | `{config_root}/daemon/state.json`에 queue entry, active job과 scheduler 정보를 기록한다. | job reference·provider path → queue·daemon state | F02, F04 | `legacy/docs/implementation/daemon-contract.md`, `legacy/specs/schemas/daemon-state.schema.json` |
| F13-03 | Queue 전제 조건 검사 | 종료된 Job, 중복 요청, 승인 대기 Job이 다시 실행되는 문제 | enqueue와 scheduling 전에 terminal, duplicate, approval 조건을 확인한다. | queue request·RunState → enqueue 또는 구조화 오류 | F02, F11 | `legacy/docs/implementation/daemon-contract.md` |
| F13-04 | Scheduler Tick | queue에서 다음 Job을 골라 실행 단계로 넘기는 규칙이 필요한 문제 | 한 tick마다 실행 가능한 entry를 선택하고 결과와 queue 상태를 갱신한다. | daemon state·queue → 선택·실행·다음 state | F07, F08 | `legacy/docs/implementation/briefs/E55-daemon-queue-scheduler-tick.md` |
| F13-05 | Fake·Local Process Scheduler 실행 | scheduler가 실제 adapter 경계를 통해 provider 결과를 만들려는 문제 | fake 또는 allowlist local process instance를 ExecutionEngine에 넘긴다. | queue entry·instance → provider artifacts·Job state | F07, F08 | `legacy/docs/implementation/briefs/E56-daemon-local-process-scheduler-executor.md` |
| F13-06 | Cancel·Resume·Status Watch | background 흐름의 제어 요청과 화면 상태가 어긋나는 문제 | 취소 전파, 재개 전제 조건과 반복 상태 조회를 같은 daemon state에 연결한다. | control request·Job state → cancel/resume·status | F02, F12, F14, F15 | `legacy/docs/implementation/cli-daemon-api-ui.md` |
| F13-07 | Provider Session 관리 | 장시간 Job과 그 Job을 수행하는 provider 실행 문맥의 관계를 daemon에서 이어가려는 문제 | queue, scheduling, cancel, resume, status watch와 함께 provider session을 daemon 책임으로 둔다. | Job·provider assignment·실행 상태 → provider session 관리 정보 | F07, F15 | `legacy/docs/implementation/cli-daemon-api-ui.md`, `legacy/docs/implementation/complete-implementation-roadmap.md` |

## F14. HTTP API 조회·제어

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F14-01 | Local HTTP Server | UI와 로컬 도구가 daemon 기능을 공통 JSON 계약으로 호출하려는 문제 | loopback 주소에서 정해진 API path와 JSON 응답을 제공한다. | HTTP request → API response envelope | F13, F15 | `legacy/docs/implementation/api-contract.md`, `legacy/docs/implementation/briefs/E50-local-http-api-server-surface.md` |
| F14-02 | 상태 조회 API | daemon, 프로젝트, Job, event, report를 파일 경로를 모르는 도구도 읽게 하려는 문제 | daemon·project·job·events·report·release-readiness 조회 endpoint를 제공한다. | GET path·query → read model 또는 구조화 오류 | F02, F04, F11, F19 | `legacy/docs/implementation/api-contract.md`, `legacy/docs/implementation/briefs/E22-api-read-only.md` |
| F14-03 | Job 제어 API | UI의 approve, cancel, resume가 CLI와 다른 규칙으로 상태를 바꾸는 문제 | 같은 StateStore와 수명주기 계약으로 제한된 POST mutation을 수행한다. | POST action·body → approval·state·event·API 응답 | F11, F13, F17 | `legacy/docs/implementation/api-contract.md`, `legacy/docs/implementation/briefs/E24-api-control-mutations.md` |
| F14-04 | Provider Connection API | UI에서 provider instance를 저장하고 실행 경로에 넘기려는 문제 | provider 탐색, instance 저장·검사·선택·healthcheck와 run request를 제공한다. | instance JSON·action → 저장 경로·정책 결과·queue request | F06, F08, F13, F15 | `legacy/docs/implementation/cli-daemon-api-ui.md`, `legacy/apps/star-daemon/README.md` |
| F14-05 | API 오류 계약 | 잘못된 path, body, Job ID와 상태 전이 실패를 호출자가 구분하기 어려운 문제 | HTTP status와 공통 API envelope 안에 오류 코드·메시지·세부 정보를 넣는다. | 요청 오류 → `api-response` 오류 | F03, F12, F15 | `legacy/docs/implementation/api-contract.md`, `legacy/specs/schemas/api-response.schema.json` |
| F14-06 | Browser Origin 연결 | 정적 UI가 loopback daemon을 호출할 때 browser preflight와 origin 정책이 필요한 문제 | 허용된 로컬 origin에 CORS와 OPTIONS 응답을 제공한다. | browser preflight·API call → CORS headers·API response | F15, F16 | `legacy/apps/star-daemon/README.md`, `legacy/apps/star-control-ui/README.md` |

## F15. Browser UI·Job 관제·Provider 설정

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F15-01 | Job 목록과 상세 화면 | 여러 Job의 상태와 대기 이유를 파일을 열지 않고 비교하려는 문제 | Job 목록에서 상태를 보여주고 상세 화면에서 단계·provider·다음 행동을 표시한다. | API job view → list·detail 화면 | F02, F14 | `legacy/docs/implementation/ui-shell-contract.md`, `legacy/docs/implementation/briefs/E23-ui-read-only-view.md` |
| F15-02 | Timeline | 장시간 실행에서 사건의 순서와 상태 변경 원인을 이해하기 어려운 문제 | CoreEvent 기반 실행 사건을 시간 순서로 보여준다. | event API → timeline | F04 | `legacy/docs/implementation/ui-shell-contract.md` |
| F15-03 | Evidence Viewer | provider 출력, 검증, 승인, review pack과 보고서를 여러 경로에서 찾아야 하는 문제 | artifact 종류별 패널에서 내용과 경로를 표시한다. | artifact refs·API response → evidence panels | F07, F09~F11, F16, F17 | `legacy/docs/implementation/ui-shell-contract.md` |
| F15-04 | Approval Control Panel | 승인·취소·재개 가능 여부와 결과를 화면에서 명확히 다뤄야 하는 문제 | 상태에 맞는 버튼만 활성화하고 mutation 결과와 새 상태를 표시한다. | user action·Job state → control API call·result | F11, F14, F17 | `legacy/docs/implementation/briefs/E25-ui-browser-control-shell.md` |
| F15-05 | Release Readiness Viewer | 배포 준비 checks, blockers와 approvals를 한 화면에서 검토하려는 문제 | readiness 경로, 상태, 검사, 차단 사유와 승인을 표시한다. | release API model → readiness panel | F19 | `legacy/docs/implementation/briefs/E36-release-readiness-ui-read.md` |
| F15-06 | Provider Connection Manager | instance JSON을 직접 편집하지 않고 연결 정보를 관리하려는 문제 | provider 선택, instance 입력·검사·저장·선택·healthcheck와 run 요청 화면을 제공한다. | form data → provider connection API·저장 경로 | F06, F08, F14 | `legacy/apps/star-control-ui/README.md` |
| F15-07 | 장시간 작업 상태 표현 | 실행 중인 단계, 경과 시간, 차단 원인과 다음 행동을 한눈에 알기 어려운 문제 | stage, provider, 최근 event, elapsed, approval, blocker와 next action을 함께 표시한다. | API state·events → 관제 상태 화면 | F02, F13, F14 | `legacy/docs/implementation/cli-daemon-api-ui.md` |
| F15-08 | Provider Session Dashboard | 장시간 Job에서 어떤 provider session이 연결되어 있는지 관제 화면에서 확인하려는 문제 | provider session dashboard를 UI 목표에 포함하고 Job의 active provider·상태 정보와 연결한다. | Job·provider session 정보 → dashboard view | F13, F14 | `legacy/docs/implementation/cli-daemon-api-ui.md` |

## F16. 보안·Privacy Handoff·Redaction

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F16-01 | Credential Reference | API key와 token 원문이 설정·artifact에 복사되는 문제 | credential 값 대신 외부 저장 위치를 가리키는 `credential_ref`만 계약에 넣는다. | credential 위치 → reference | F06, F08 | `legacy/docs/implementation/security-cost-observability.md`, `legacy/docs/implementation/cloud-provider-policy.md` |
| F16-02 | Secret Redaction | provider 출력, 보고서와 로그에 비밀정보 후보가 노출되는 문제 | key, token, password, private key와 `.env` 후보를 `[REDACTED]`로 바꾼다. | text·JSON artifact → 가려진 결과 | F07, F11~F15 | `legacy/docs/implementation/security-cost-observability.md`, `legacy/docs/implementation/briefs/E26-security-redaction-utility.md` |
| F16-03 | RedactionReport | 무엇을 가렸는지 기록하면서도 원문을 다시 노출하지 않아야 하는 문제 | finding 종류, 위치와 처리 결과만 별도 감사 artifact로 저장한다. | redaction findings → `redaction-report-*.json`·ArtifactRef | F04, F17 | `legacy/docs/implementation/security-privacy-observability-contracts.md`, `legacy/docs/implementation/briefs/E61-redaction-report-artifact-storage.md` |
| F16-04 | Privacy Handoff | provider나 tool에 어떤 파일과 문맥을 보냈는지 알 수 없는 문제 | destination, context path, 가림 여부와 승인 근거를 handoff artifact로 남긴다. | 전달 대상·context·approval → privacy handoff 또는 차단 | F08, F11, F17 | `legacy/docs/implementation/security-privacy-observability-contracts.md`, `legacy/specs/schemas/privacy-handoff.schema.json` |
| F16-05 | 위험 행동 분류 | 의존성, workflow, 배포, 삭제, 계정·권한, 검사기 변경이 일반 편집으로 실행되는 문제 | 위험 행동 종류를 분류해 승인 요청이나 차단 조건으로 넘긴다. | WorkSpec·change type·policy → danger classification | F05, F07, F11 | `legacy/docs/implementation/security-cost-observability.md`, `legacy/configs/policies/permission-policy.yaml` |
| F16-06 | CLI 보고서 가림 연결 | CLI가 원본 report를 그대로 출력해 비밀정보가 터미널로 나오는 문제 | report를 출력하기 전에 가리고 RedactionReport 참조를 남긴다. | ReportSpec → redacted CLI output·audit artifact | F12, F17 | `legacy/docs/implementation/briefs/E64-cli-report-redaction-artifact-wiring.md` |
| F16-07 | Provider 출력 가림 연결 | provider 원문과 정규화 결과가 후속 화면·보고서에 그대로 전달되는 문제 | provider output 저장과 전달 경계에서 가림 결과와 보고서를 연결한다. | provider artifacts → redacted artifacts·RedactionReport | F07, F09, F15 | `legacy/docs/implementation/briefs/E65-provider-output-redaction-artifact-wiring.md` |

## F17. Audit·Event·Log·Cost·Budget

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F17-01 | CoreEvent와 AuditEvent 분리 | 실행 흐름 사건과 사람이 검토해야 할 통제 사건이 섞이는 문제 | 상태·단계 사건은 CoreEvent, 승인·정책·제어 사건은 AuditEvent로 기록한다. | runtime·control 사건 → `events.jsonl` 또는 `audit-events.jsonl` | F04, F11, F13~F16 | `legacy/docs/implementation/artifact-layout.md`, `legacy/docs/implementation/security-privacy-observability-contracts.md` |
| F17-02 | 추가 전용 감사 기록 | 자동 판단과 사람 행동의 과거 기록이 덮이는 문제 | actor, action, target, reason, correlation ID와 artifact 참조를 순서대로 추가한다. | control·policy 사건 → audit event 행 | F11, F14, F19 | `legacy/docs/implementation/briefs/E27-observability-audit-event-writer.md` |
| F17-03 | Event·Log·Metric 역할 분리 | 흐름 추적, 상세 디버깅과 집계값을 같은 파일로 해결하려는 문제 | event는 상태 흐름, log는 상세 실행 내용, metric은 수치 요약을 담당한다. | runtime activity → event·log·metric artifact | F07, F13 | `legacy/docs/implementation/security-cost-observability.md` |
| F17-04 | Cost Metric Sidecar | provider별 token, 시간과 비용 표현이 달라 비교·보고가 어려운 문제 | 입력·출력 token, 경과 시간, 추정 비용, 통화와 quota를 공통 sidecar로 기록한다. | provider run → `cost-metric.json` | F07, F08, F11 | `legacy/docs/implementation/security-privacy-observability-contracts.md`, `legacy/docs/implementation/briefs/E62-provider-cost-metric-sidecar-integration.md` |
| F17-05 | Budget 경고 | 누적 사용량이 기준에 가까워지는 사실을 실행 뒤에야 아는 문제 | warning threshold를 넘으면 실행·보고에 경고 신호를 붙인다. | budget·cost metrics → warning | F05, F11 | `legacy/docs/implementation/security-cost-observability.md`, `legacy/docs/implementation/briefs/E28-cost-metric-budget-guard.md` |
| F17-06 | Cloud Hard Budget | 예상 비용이 한도를 넘는 외부 전송을 실행 뒤에 발견하는 문제 | 전송 전에 estimated cost와 hard limit을 비교해 초과 시 `BLOCKED` 결과를 만든다. | estimate·hard limit → transport 허용 또는 blocked result | F08, F11 | `legacy/docs/implementation/briefs/E63-cloud-hard-budget-enforcement.md` |
| F17-07 | HTTP 제어 감사 연결 | API로 수행한 승인·취소·재개가 실행 이력에 남지 않는 문제 | control API 성공·실패를 Job, actor, action과 함께 감사 기록에 추가한다. | API mutation → AuditEvent·API response | F14 | `legacy/docs/implementation/briefs/E52-daemon-http-control-audit-integration.md` |

## F18. Recovery·Retention·Artifact 교체

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F18-01 | Recovery Inspection | 손상 JSON, 잘린 event log, 남은 tmp와 경로 위반을 숨기지 않고 찾아야 하는 문제 | Job 폴더를 읽기 전용으로 검사해 issue 종류, 위치와 제안 행동을 만든다. | job artifacts → RecoveryInspection·issues | F04, F12 | `legacy/docs/implementation/state-store-recovery.md`, `legacy/docs/implementation/briefs/E30-state-recovery-inspection.md` |
| F18-02 | Recovery Dry-run Plan | 복구 명령이 어떤 파일을 바꿀지 실행 전에 확인해야 하는 문제 | action, target, 예상 변경과 승인 필요 여부를 계획 artifact로 만든다. | inspection·requested action → recovery plan | F11, F12, F17 | `legacy/docs/implementation/briefs/E53-recovery-action-dry-run-approval-surface.md` |
| F18-03 | 승인 Token 기반 행동 | 손상 복구라는 이름으로 파괴적 정리가 바로 실행되는 문제 | 계획과 일치하는 token을 확인한 뒤 cleanup, trim, retention 행동을 수행한다. | recovery plan·token → action result 또는 차단 | F11, F17 | `legacy/docs/implementation/state-store-recovery.md`, `legacy/docs/implementation/briefs/E57-recovery-action-executor.md` |
| F18-04 | 원본 보존 복구본 | 손상 원본을 고쳐 쓰다가 감사 증거를 잃는 문제 | 원본을 남기고 별도 recovered copy와 그 출처를 기록한다. | 손상 artifact → recovered copy·source reference | F04, F17 | `legacy/docs/implementation/state-store-recovery.md` |
| F18-05 | Event Log Trim | 잘린 마지막 JSONL 행 때문에 전체 이력을 읽지 못하는 문제 | 손상 지점 전까지의 별도 복구본이나 승인된 교체 결과를 만든다. | corrupt event log·plan → trimmed copy·result | F04, F17 | `legacy/docs/implementation/state-store-recovery.md` |
| F18-06 | Artifact 교체 Source 선택 | 어떤 복구본을 원본 경로에 사용할지 모호한 문제 | source와 target을 명시하고 적합성·경로·승인 조건을 검사한 뒤 원자적으로 교체한다. | source artifact·target·token → replacement result | F04, F07 | `legacy/docs/implementation/briefs/E59-artifact-replacement-source-selection.md` |
| F18-07 | Retention Cleanup | 오래된 실행 자료를 정리하면서 보존 규칙과 감사 정보를 잃는 문제 | data policy에 따른 대상 목록을 계획하고 승인된 범위만 정리한다. | retention policy·job inventory → cleanup plan·result | F04, F16, F17 | `legacy/docs/implementation/artifact-layout.md`, `legacy/configs/policies/data-policy.yaml` |

## F19. Release Readiness·승인형 자동화

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F19-01 | ReleaseReadiness Artifact | 배포 전 checks, blockers, approvals와 evidence가 여러 문서에 흩어지는 문제 | 준비 상태와 항목별 근거를 `release-readiness.json`에 고정한다. | checks·evidence·approvals → ReleaseReadiness | F04, F11, F12, F14, F15 | `legacy/docs/implementation/release-readiness.md`, `legacy/docs/implementation/briefs/E31-release-readiness-writer.md` |
| F19-02 | Version·Changelog 일치 검사 | 기대 버전, 선언 파일과 변경 기록의 버전이 서로 다른 문제 | 지정한 경로의 version과 changelog entry를 비교해 check와 blocker를 만든다. | expected version·version files·changelog → consistency checks | F20 | `legacy/docs/implementation/briefs/E33-release-version-consistency-checker.md` |
| F19-03 | Evidence File 발견 | readiness가 가리킨 검증·보안·rollback 자료의 존재를 확인하기 어려운 문제 | 지정 evidence 경로를 찾아 존재와 참조 가능 여부를 기록한다. | evidence requirements·repository → evidence refs·blockers | F04, F20 | `legacy/docs/implementation/briefs/E34-release-evidence-file-discovery.md` |
| F19-04 | Release Profile 통합 | 일반 검증과 배포 전 검사를 서로 다른 판정 체계로 운영하는 문제 | release profile의 Sentinel 결과를 readiness checks와 blockers에 합친다. | release validation·Sentinel decision → readiness update | F09, F10 | `legacy/docs/implementation/briefs/E35-release-profile-readiness-integration.md` |
| F19-05 | Release Review Pack | 준비 상태를 사람이 읽을 수 있는 형태로 검토하려는 문제 | checks, blockers, approvals, evidence와 다음 행동을 Markdown 검토 자료로 만든다. | validated readiness → release review pack | F11 | `legacy/docs/implementation/release-readiness.md`, `legacy/docs/implementation/briefs/E38-release-review-pack-foundation.md` |
| F19-06 | Release Automation Dry-run | signing, publish, deploy, rollback 행동의 순서와 영향을 실행 전에 확인해야 하는 문제 | action별 단계, 정책, 필요 승인과 예상 artifact를 JSON 계획으로 만든다. | readiness·release action → automation plan | F11, F12, F17 | `legacy/docs/implementation/briefs/E54-release-automation-dry-run-approval-surface.md` |
| F19-07 | 승인형 Local Executor | 승인 후의 자동화 결과를 외부 효과와 분리해 기록하려는 문제 | token을 확인하고 로컬 automation result artifact를 작성한다. | plan·approval token → local result 또는 차단 | F11, F17 | `legacy/docs/implementation/briefs/E58-release-automation-executor.md` |
| F19-08 | External Release Policy | publish, deploy, 원격 설정과 계정 변경의 외부 경계를 명시해야 하는 문제 | 외부 행동별 승인·증거 요구와 실행 결과 기록 형식을 정의한다. | external action intent·approval → policy decision·result contract | F11, F16, F17 | `legacy/docs/implementation/briefs/E66-external-release-execution-policy.md` |
| F19-09 | 최종 범위 감사 자료 조립 | 여러 단계와 CI·PR 증거를 하나의 readiness 판단 자료로 모으려는 문제 | M0~M9, hardening, recovery, release와 PR evidence를 checks·blockers로 조립한다. | milestone·CI·PR evidence → readiness·audit evidence | F20 | `legacy/docs/implementation/briefs/E40-final-m9-readiness-audit.md`, `legacy/docs/implementation/briefs/E67-final-readiness-pre-live-ai.md` |

## F20. CI·E2E·GitHub·PR·Worktree 운영

| ID | 기능 | 해결하려던 문제 | 문서상 동작 | 입력 → 결과·상태 | 연결 기능 | 역사적 근거 |
|---|---|---|---|---|---|---|
| F20-01 | 계약 검사 Runner | 문서, Schema, 예시, manifest와 설정이 서로 어긋나는 문제 | repository 정책, 자료 형식, manifest 참조, 명명, Schema 예시, 문서, work queue 검사를 한 흐름으로 실행한다. | repository files → check별 결과·통합 결과 | F01, F03, F06, F10 | `legacy/docs/implementation/ci-contract-validation.md`, `legacy/docs/operations/ci-roadmap.md` |
| F20-02 | 단계별 CI | 기능이 늘어날 때 모든 검사를 처음부터 같은 비용으로 실행하는 문제 | milestone에 따라 provider, router, execution, Sentinel, daemon/API/UI, 보안, 복구, release 검사를 확장한다. | 변경 범위·milestone → targeted 또는 전체 CI | F03~F19 | `legacy/docs/operations/ci-roadmap.md` |
| F20-03 | Integration Smoke | Job부터 보고까지 여러 기능의 전달 계약이 함께 맞는지 확인하려는 문제 | fake provider를 사용해 core 실행 흐름과 artifact 생성을 한 번에 검사한다. | fixture project·JobSpec → smoke evidence | F02~F11 | `legacy/docs/implementation/briefs/E11-integration-smoke.md` |
| F20-04 | Productization E2E Smoke | daemon, API, UI, 보안, 비용, 복구와 release 표면을 함께 검사하려는 문제 | 로컬 fixture와 runtime을 사용해 주요 사용자·운영 흐름의 증거를 만든다. | local fixtures·runtime → E2E smoke evidence | F06, F08, F12~F19 | `legacy/docs/implementation/briefs/E60-productization-e2e-smoke.md`, `legacy/README.md` |
| F20-05 | 장기 작업 Queue와 인계 | 긴 작업에서 범위, 검증 결과와 다음 행동이 대화 사이에 사라지는 문제 | EPIC, TASK, PR 단위와 checkpoint, validation report, handoff 내용을 고정한다. | work queue item → branch·PR·checkpoint·handoff | F01 | `legacy/docs/implementation/codex-long-run-workflow.md` |
| F20-06 | Branch·PR 작업 규칙 | 여러 작업이 같은 변경에 섞이고 검토 근거가 부족해지는 문제 | 작은 변경 범위, branch 명명, PR 본문과 검증 보고 형식을 정의한다. | task·diff·validation → branch·PR evidence | F01, F11, F19 | `legacy/docs/operations/chatgpt-github-workflow.md`, `legacy/docs/implementation/codex-long-run-workflow.md` |
| F20-07 | Stacked PR Readiness | 연속된 PR의 기준 branch, 순서와 병합 가능 상태를 잃는 문제 | 연속 stack, draft 상태, CI, base 관계와 main 반영 여부를 evidence로 정리한다. | PR chain·CI state → stacked readiness evidence | F19 | `legacy/docs/implementation/briefs/E43-stacked-pr-readiness-coordination.md` |
| F20-08 | Stacked Merge 절차 | 의존 PR을 잘못된 순서로 병합하거나 이상 상태에서도 계속 진행하는 문제 | clean 상태를 확인하고 검토·병합 순서와 중단 조건을 적용한다. | 승인된 PR range → merge evidence·main CI evidence | F19 | `legacy/docs/implementation/audit/stacked-pr-merge-procedure.md`, `legacy/docs/implementation/briefs/E47-stacked-merge-procedure.md` |
| F20-09 | Worktree 상태 확인 | 여러 작업 폴더의 변경이 섞이거나 잘못된 branch를 대상으로 삼는 문제 | 작업 전 clean worktree, branch, base와 변경 범위를 확인하고 기록한다. | worktree·git state → 작업 가능 여부·checkpoint | F01, F20-07, F20-08 | `legacy/docs/implementation/codex-long-run-workflow.md`, `legacy/docs/implementation/audit/stacked-pr-merge-procedure.md` |
| F20-10 | 최종 감사 Evidence 갱신 | 구현과 문서가 바뀐 뒤 과거 검증 자료를 그대로 사용하는 문제 | CI·PR·readiness 근거를 다시 수집해 감사 자료와 참조를 갱신한다. | repository·CI·PR evidence → audit evidence set | F19 | `legacy/docs/implementation/briefs/E41-final-completion-audit.md`, `legacy/docs/implementation/briefs/E42-final-audit-evidence.md`, `legacy/docs/implementation/briefs/E46-final-evidence-refresh.md` |
| F20-11 | 장시간 Context 요약·새 세션 인계 | 세션이 길어져 과거 대화가 줄거나 새 세션으로 옮길 때 핵심 계약과 진행 상태를 잃는 문제 | EPIC·TASK, PR, branch, 수정 파일, 검증, 남은 일, 주의 계약과 다음 handoff를 짧게 요약하고 기준 문서를 다시 읽는다. | 장시간 작업 상태 → context 요약·새 세션 시작 자료 | F01, F02, F20-05 | `legacy/docs/implementation/codex-long-run-workflow.md` |

## 부록 A. E01~E67 브리프 대응표

각 브리프는 주된 사용자·시스템 동작을 기준으로 한 기능군에 한 번만 배치했다. 다른 기능군과의 관계는 본문 각 항목의 `연결 기능`에서 확인한다.

| 기능군 | 번호가 붙은 브리프 |
|---|---|
| F01 | 전용 번호 브리프 없음. 결정 기록·로드맵·작업 흐름 문서가 직접 근거다. |
| F02 | 전용 번호 브리프 없음. 개요·데이터 계약·수명주기 문서가 직접 근거다. |
| F03 | E01 |
| F04 | E02, E03 |
| F05 | E06 |
| F06 | E04, E14, E29, E48 |
| F07 | E07 |
| F08 | E05, E12, E13, E15, E16, E17, E18, E19 |
| F09 | E10 |
| F10 | E09 |
| F11 | E38 |
| F12 | E08, E20, E37, E39, E44, E45 |
| F13 | E21, E49, E55, E56 |
| F14 | E22, E24, E32, E50 |
| F15 | E23, E25, E36, E51 |
| F16 | E26, E61, E64, E65 |
| F17 | E27, E28, E52, E62, E63 |
| F18 | E30, E53, E57, E59 |
| F19 | E31, E33, E34, E35, E40, E54, E58, E66, E67 |
| F20 | E11, E41, E42, E43, E46, E47, E60 |

대응 결과는 번호 67개, 서로 다른 번호 67개이며 빠진 번호와 중복 배치는 없다. `legacy/docs/implementation/briefs/README.md`는 브리프 사용법을 설명하므로 F01의 보조 근거로 분류했다.

경계가 겹치는 브리프는 다음 기준으로 배치했다.

- E10은 승인 자료도 만들지만 중심 동작이 검증 결과 전달이므로 F09에 두었다.
- E12와 E19는 보안·승인 조건도 다루지만 Cloud Provider 경로를 정의하므로 F08에 두었다.
- E32, E36, E37은 같은 Release Readiness를 각각 API, UI, CLI에서 다루므로 F14, F15, F12에 나눴다.
- E38은 release 자료를 사용하지만 주된 산출물이 Review Pack이므로 F11에 두었다.
- E39와 E45는 각각 Recovery와 Sentinel을 호출하지만 사용자 표면이 CLI이므로 F12에 두었다.
- E52는 HTTP 제어 뒤에 AuditEvent를 연결하는 것이 중심이므로 F17에 두었다.
- E56은 Local Process를 실행하지만 Queue Scheduler의 실행 범위를 설명하므로 F13에 두었다.
- E61은 artifact 저장도 포함하지만 산출물의 의미가 RedactionReport이므로 F16에 두었다.
- E41, E42, E46은 제품 기능보다 전체 작업 증거와 readiness 자료를 조립하므로 F20에 두었다.

## 부록 B. Schema 46개 대응표

| 기능군 | Schema 파일 이름에서 `.schema.json`을 뺀 이름 |
|---|---|
| F02 | `job`, `run-state` |
| F03 | `config`, `error`, `hook`, `policy`, `renderer`, `role`, `skill` |
| F04 | `artifact-ref`, `event` |
| F05 | `route`, `router-decision`, `workspec` |
| F06 | `capability`, `capability-profile`, `model-profile`, `provider-capability`, `provider-instance`, `provider-kind`, `provider-manifest`, `provider-registry` |
| F07 | `execution-attempt`, `execution-request`, `provider-result`, `provider-run-result` |
| F09 | `diagnostic`, `validation-decision`, `validation-run` |
| F10 | `tool-manifest`, `tool-result` |
| F11 | `approval`, `approval-request`, `approval-response`, `report`, `review-pack-handoff` |
| F12 | `cli-error`, `cli-output` |
| F13 | `daemon-state` |
| F14 | `api-response` |
| F15 | `ui-job-view` |
| F16 | `privacy-handoff`, `redaction-report` |
| F17 | `audit-event`, `cost-metric` |
| F19 | `release-readiness` |

F01, F08, F18, F20은 전용 Schema가 없고 문서·설정·예시가 근거다. `run-state`는 저장 기능인 F04에도 연결된다.

`legacy/specs/contracts/`의 계약 문서 7개는 다음과 같이 연결된다.

| 기능군 | 계약 문서 |
|---|---|
| F06 | `capability-registry.md`, `provider-capability.md`, `provider-transport.md` |
| F07 | `provider-adapter.md` |
| F09 | `diagnostic-model.md`, `quality-gate.md` |
| F10 | `tool-adapter.md` |

## 부록 C. 설정·예시·Manifest 자료 대응표

### 설정 자료

`legacy/configs/`에는 파일이 102개 있다. 그중 기계 판독 자료는 YAML 72개와 JSON 2개이고, 나머지 28개는 역할·Skill·Renderer 등을 설명하는 Markdown이다.

| 자료군 | 파일 수 | 연결 기능군 |
|---|---:|---|
| `defaults` | 2 | F04, F05, F06, F10 |
| `hooks` | 15 | F02, F04, F05, F07, F09, F11, F17, F18 |
| `policies` | 15 | F05~F11, F16~F18, F20 |
| `provider-instances` | 10 | F06, F08, F16, F17 |
| `registries` | 4 | F05, F06, F08, F10 |
| `renderers` | 27 | F03, F08 |
| `roles` | 11 | F01, F05, F07~F09, F11, F16, F19 |
| `skills` | 12 | F01, F05, F07, F09, F11, F16, F18~F20 |
| `templates` | 6 | F01, F05, F11 |

정책 파일은 다음 기능 경계를 설명한다.

| 정책 | 연결 기능군 |
|---|---|
| `approval-policy` | F11, F16, F20 |
| `budget-policy` | F17 |
| `command-policy` | F16, F20 |
| `data-policy` | F16, F18 |
| `error-taxonomy` | F07, F18 |
| `model-routing` | F05, F08 |
| `permission-policy` | F07, F11, F16 |
| `provider-policy` | F06, F07, F16 |
| `provider-selection` | F05, F06, F08 |
| `retry-policy` | F07, F18 |
| `risk-policy` | F05, F11, F16 |
| `sandbox-policy` | F07, F16, F20 |
| `scope-policy` | F07, F16 |
| `secret-policy` | F16 |
| `tool-policy` | F09, F10 |

### 예시 자료

`legacy/examples/`에는 파일이 60개 있다. 내용이 있는 예시는 JSON 45개, Markdown 4개, TOML 3개, YAML 4개로 모두 56개다. 나머지 4개는 빈 예시 폴더를 보존하는 `.gitkeep`이므로 기능 근거로 세지 않았다.

| 자료군 | 파일 수 | 연결 기능군 |
|---|---:|---|
| `cli-contracts` | 4 | F12 |
| `config-contracts` | 6 | F03 |
| `core` | 3 | F03, F04 |
| `execution-contracts` | 3 | F07, F08 |
| `fake` | 3 | F05, F08, F11 |
| `mvp` | 2 | F02, F05 |
| `projects/*.gitkeep` | 3 | 기능군 없음. 빈 fixture 폴더 보존 파일 |
| `provider-contracts` | 5 | F06, F07 |
| `provider-instances` | 4 | F06, F08 |
| `release-contracts` | 3 | F19, F20 |
| `rendered-provider-artifacts` | 3 | F03, F08 |
| `router-contracts` | 2 | F05, F11 |
| `runs`의 내용 있는 파일 | 7 | F02, F04, F05, F11 |
| `runs/.gitkeep` | 1 | 기능군 없음. 빈 실행 폴더 보존 파일 |
| `security-contracts` | 4 | F16, F17 |
| `surface-contracts` | 3 | F13, F14, F15 |
| `validation-contracts` | 4 | F09, F11 |

별도 예시는 `legacy/builtin-tools/star-sentinel/examples/`과 `legacy/configs/provider-instances/*.example.yaml`에도 있으며 각각 F10과 F06·F08의 근거로 확인했다.

### Builtin Provider 자료

Builtin Registry 항목 20개는 manifest 20개, capability 20개와 ID가 일대일로 연결된다. 각 provider 폴더에는 설명 README도 하나씩 있다.

| Provider kind | 항목 수 | 연결 기능군 |
|---|---:|---|
| `cloud_api_model` | 3 | F06, F08 |
| `cloud_cli_agent` | 7 | F06, F08 |
| `local_openai_compatible_server` | 5 | F06, F08 |
| `local_process_model` | 3 | F06, F08 |
| `fake_provider` | 1 | F06, F08 |
| `human_handoff` | 1 | F06, F08, F11 |

Capability Registry는 29개 이름을 다음 범주로 묶는다. 이 목록은 실행 환경을 비교하기 위한 선언 자료이며 각 항목이 별도 제품 화면이나 명령이라는 뜻은 아니다.

| 범주 | Capability 이름 | 연결 기능군 |
|---|---|---|
| 문맥·기억 | `context_files`, `scoped_context_rules`, `memory`, `memory_compaction` | F02, F06 |
| 계획·목표 | `plan_mode`, `goal_mode` | F02, F05, F06 |
| 절차·Lifecycle | `skills`, `commands`, `hooks` | F03, F06 |
| 안전 | `command_rules`, `permissions`, `sandbox`, `checkpoints` | F11, F16, F20 |
| 위임·이어하기 | `subagents_workers`, `agent_teams`, `thread_resume`, `background_agents` | F02, F06, F13 |
| 도구·검색 | `mcp`, `tool_calling`, `lexical_search`, `semantic_search` | F03, F06 |
| 격리·Git | `worktrees`, `branch_pr` | F20 |
| 품질 | `code_review`, `validation_loop`, `critic` | F09, F10, F20 |
| 비용·확장·관제 | `budget_control`, `extension_packages`, `control_plane` | F06, F15, F17 |

Star Sentinel 폴더에는 비코드 파일이 44개 있다. 내용이 있는 자료 41개는 tool 선언 1개, Schema 12개, 예시 9개, 정책 5개, template 4개, fixture 2개, 설명 문서 8개이며 F09, F10, F11, F16, F17에 연결된다. 나머지 3개는 `corpus/positive`, `corpus/negative`, `corpus/regression`의 빈 폴더 보존용 `.gitkeep`으로 기능군에 연결하지 않았다.

### 파일별 설정 대응

아래 표는 설정 폴더의 102개 파일을 빠짐없이 나열한 검증 목록이다. 여러 기능군이 함께 적힌 경우 하나의 자료가 그 기능들의 공통 설정 또는 설명 근거라는 뜻이다.

| 파일 | 연결 기능군 |
|---|---|
| `legacy/configs/defaults/router.yaml` | F05 |
| `legacy/configs/defaults/star-control.yaml` | F03, F04, F05, F06, F10 |
| `legacy/configs/hooks/after-compact.yaml` | F03, F20 |
| `legacy/configs/hooks/after-plan.yaml` | F03, F05 |
| `legacy/configs/hooks/after-review.yaml` | F03, F10, F11, F20 |
| `legacy/configs/hooks/after-route.yaml` | F03, F05 |
| `legacy/configs/hooks/after-tool.yaml` | F03, F09, F10 |
| `legacy/configs/hooks/after-validation.yaml` | F03, F09 |
| `legacy/configs/hooks/after-worker.yaml` | F03, F07, F13 |
| `legacy/configs/hooks/before-compact.yaml` | F03, F20 |
| `legacy/configs/hooks/before-plan.yaml` | F03, F05 |
| `legacy/configs/hooks/before-route.yaml` | F03, F05 |
| `legacy/configs/hooks/before-tool.yaml` | F03, F09, F10 |
| `legacy/configs/hooks/before-worker.yaml` | F03, F07, F13 |
| `legacy/configs/hooks/on-blocked.yaml` | F02, F03, F11, F17 |
| `legacy/configs/hooks/on-done.yaml` | F02, F03, F04, F17 |
| `legacy/configs/hooks/on-failed.yaml` | F02, F03, F04, F17 |
| `legacy/configs/policies/approval-policy.yaml` | F11, F16, F20 |
| `legacy/configs/policies/budget-policy.yaml` | F17 |
| `legacy/configs/policies/command-policy.yaml` | F16, F20 |
| `legacy/configs/policies/data-policy.yaml` | F16, F18 |
| `legacy/configs/policies/error-taxonomy.yaml` | F07, F18 |
| `legacy/configs/policies/model-routing.yaml` | F05, F08 |
| `legacy/configs/policies/permission-policy.yaml` | F07, F11, F16 |
| `legacy/configs/policies/provider-policy.yaml` | F06, F07, F16 |
| `legacy/configs/policies/provider-selection.yaml` | F05, F06, F08 |
| `legacy/configs/policies/retry-policy.yaml` | F07, F18 |
| `legacy/configs/policies/risk-policy.yaml` | F05, F11, F16 |
| `legacy/configs/policies/sandbox-policy.yaml` | F07, F16, F20 |
| `legacy/configs/policies/scope-policy.yaml` | F07, F16 |
| `legacy/configs/policies/secret-policy.yaml` | F16 |
| `legacy/configs/policies/tool-policy.yaml` | F09, F10 |
| `legacy/configs/provider-instances/anthropic-api.example.yaml` | F06, F08, F16, F17 |
| `legacy/configs/provider-instances/claude-code.example.yaml` | F06, F08 |
| `legacy/configs/provider-instances/codex-cli.example.yaml` | F06, F08 |
| `legacy/configs/provider-instances/fake-provider.example.yaml` | F06, F08 |
| `legacy/configs/provider-instances/gemini-cli.example.yaml` | F06, F08 |
| `legacy/configs/provider-instances/google-gemini-api.example.yaml` | F06, F08, F16, F17 |
| `legacy/configs/provider-instances/local-openai-compatible.example.yaml` | F06, F08 |
| `legacy/configs/provider-instances/local-process.example.yaml` | F06, F08 |
| `legacy/configs/provider-instances/openai-api.example.yaml` | F06, F08, F16, F17 |
| `legacy/configs/provider-instances/README.md` | F06 |
| `legacy/configs/registries/builtin-provider-registry.yaml` | F06, F08 |
| `legacy/configs/registries/builtin-tool-registry.yaml` | F03, F10 |
| `legacy/configs/registries/capability-registry.yaml` | F06 |
| `legacy/configs/registries/model-registry.yaml` | F05, F06 |
| `legacy/configs/renderers/claude/claude-md-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/claude/hook-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/claude/permission-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/claude/plugin-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/claude/skill-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/claude/subagent-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/codex/agent-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/codex/hooks-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/codex/profile-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/codex/rules-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/codex/skill-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/cursor/background-agent-adapter.yaml` | F03, F08 |
| `legacy/configs/renderers/cursor/cli-adapter.yaml` | F03, F08 |
| `legacy/configs/renderers/cursor/plan-adapter.yaml` | F03, F08 |
| `legacy/configs/renderers/cursor/rules-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/gemini/command-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/gemini/extension-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/gemini/hook-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/gemini/mcp-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/gemini/subagent-renderer.yaml` | F03, F08 |
| `legacy/configs/renderers/github/code-review-renderer.yaml` | F03, F20 |
| `legacy/configs/renderers/github/custom-agent-renderer.yaml` | F03, F20 |
| `legacy/configs/renderers/github/instructions-renderer.yaml` | F03, F20 |
| `legacy/configs/renderers/github/skill-renderer.yaml` | F03, F20 |
| `legacy/configs/renderers/local/lmstudio-adapter.yaml` | F03, F08 |
| `legacy/configs/renderers/local/ollama-adapter.yaml` | F03, F08 |
| `legacy/configs/renderers/local/prompt-renderer.yaml` | F03, F08 |
| `legacy/configs/roles/router-low.md` | F03, F05 |
| `legacy/configs/roles/worker-debug.md` | F03, F05, F07 |
| `legacy/configs/roles/worker-design.md` | F03, F05, F07 |
| `legacy/configs/roles/worker-docs.md` | F03, F05, F20 |
| `legacy/configs/roles/worker-explore.md` | F03, F05, F07 |
| `legacy/configs/roles/worker-impl.md` | F03, F05, F07 |
| `legacy/configs/roles/worker-local-draft.md` | F03, F05, F08 |
| `legacy/configs/roles/worker-polish.md` | F03, F05, F07 |
| `legacy/configs/roles/worker-release.md` | F03, F05, F19 |
| `legacy/configs/roles/worker-review.md` | F03, F05, F10, F11 |
| `legacy/configs/roles/worker-security.md` | F03, F05, F10, F16 |
| `legacy/configs/skills/batch.md` | F03, F13 |
| `legacy/configs/skills/code-review.md` | F03, F10, F20 |
| `legacy/configs/skills/debug.md` | F03, F07, F18 |
| `legacy/configs/skills/dependency-review.md` | F03, F10, F16 |
| `legacy/configs/skills/docs-update.md` | F03, F20 |
| `legacy/configs/skills/migration.md` | F03, F07, F18 |
| `legacy/configs/skills/model-routing.md` | F03, F05, F06 |
| `legacy/configs/skills/plan-ledger.md` | F01, F03, F20 |
| `legacy/configs/skills/release.md` | F03, F19 |
| `legacy/configs/skills/security-review.md` | F03, F10, F16 |
| `legacy/configs/skills/task-orchestrator.md` | F02, F03, F05, F13 |
| `legacy/configs/skills/validation.md` | F03, F09, F20 |
| `legacy/configs/templates/approval-request-template.md` | F11 |
| `legacy/configs/templates/final-report-template.md` | F11 |
| `legacy/configs/templates/plans-template.md` | F01, F20 |
| `legacy/configs/templates/report-template.json` | F11 |
| `legacy/configs/templates/route-template.json` | F05 |
| `legacy/configs/templates/workspec-template.md` | F02, F05 |

### 파일별 예시 대응

아래 표는 예시 폴더의 60개 파일을 빠짐없이 나열한 검증 목록이다. `.gitkeep` 4개는 기능을 설명하지 않는 빈 폴더 보존 파일로 따로 표시했다.

| 파일 | 연결 기능군 |
|---|---|
| `legacy/examples/cli-contracts/approve-output.example.json` | F11, F12 |
| `legacy/examples/cli-contracts/error-output.example.json` | F03, F12 |
| `legacy/examples/cli-contracts/run-output.example.json` | F12 |
| `legacy/examples/cli-contracts/status-output.example.json` | F02, F12 |
| `legacy/examples/config-contracts/config.example.json` | F03 |
| `legacy/examples/config-contracts/hook.example.json` | F03 |
| `legacy/examples/config-contracts/policy.example.json` | F03 |
| `legacy/examples/config-contracts/renderer.example.json` | F03 |
| `legacy/examples/config-contracts/role.example.json` | F03 |
| `legacy/examples/config-contracts/skill.example.json` | F03 |
| `legacy/examples/core/artifact-ref.example.json` | F04 |
| `legacy/examples/core/error.example.json` | F03 |
| `legacy/examples/core/event.example.json` | F04, F17 |
| `legacy/examples/execution-contracts/execution-attempt.success.example.json` | F07 |
| `legacy/examples/execution-contracts/execution-request.fake.example.json` | F07 |
| `legacy/examples/execution-contracts/fake-provider-response.success.example.json` | F07, F08 |
| `legacy/examples/fake/impl-report-done.json` | F08, F11 |
| `legacy/examples/fake/review-report-approve.json` | F08, F11 |
| `legacy/examples/fake/route-done.json` | F05, F08 |
| `legacy/examples/mvp/expected_route_minimal.json` | F05 |
| `legacy/examples/mvp/stopwatch_request.md` | F02 |
| `legacy/examples/projects/node-sample/.gitkeep` | 기능군 없음 |
| `legacy/examples/projects/python-sample/.gitkeep` | 기능군 없음 |
| `legacy/examples/projects/rust-sample/.gitkeep` | 기능군 없음 |
| `legacy/examples/provider-contracts/capability-profile.fake.example.json` | F06 |
| `legacy/examples/provider-contracts/provider-instance.fake.example.json` | F06 |
| `legacy/examples/provider-contracts/provider-manifest.fake.example.json` | F06 |
| `legacy/examples/provider-contracts/provider-registry.example.json` | F06 |
| `legacy/examples/provider-contracts/provider-run-result.success.example.json` | F07 |
| `legacy/examples/provider-instances/codex-cli.personal.example.yaml` | F06, F08 |
| `legacy/examples/provider-instances/llama-cpp.gpu.example.yaml` | F06, F08 |
| `legacy/examples/provider-instances/lm-studio.desktop.example.yaml` | F06, F08 |
| `legacy/examples/provider-instances/local-vllm.dgxspark.example.yaml` | F06, F08 |
| `legacy/examples/release-contracts/complete-implementation-readiness.example.json` | F19, F20 |
| `legacy/examples/release-contracts/release-readiness.example.json` | F19 |
| `legacy/examples/release-contracts/stacked-pr-readiness.example.json` | F19, F20 |
| `legacy/examples/rendered-provider-artifacts/user-codex/low-router.config.toml` | F03, F08 |
| `legacy/examples/rendered-provider-artifacts/user-codex/worker-impl.config.toml` | F03, F08 |
| `legacy/examples/rendered-provider-artifacts/user-codex/worker-review.config.toml` | F03, F08 |
| `legacy/examples/router-contracts/route-approval-required.example.json` | F05, F11 |
| `legacy/examples/router-contracts/router-decision.schema-change.example.json` | F05 |
| `legacy/examples/runs/.gitkeep` | 기능군 없음 |
| `legacy/examples/runs/J-0001/final-report.md` | F11 |
| `legacy/examples/runs/J-0001/job.json` | F02 |
| `legacy/examples/runs/J-0001/request.md` | F02 |
| `legacy/examples/runs/J-0001/route.json` | F05 |
| `legacy/examples/runs/J-0001/run-state.json` | F02, F04 |
| `legacy/examples/runs/J-0001/workspec-impl.md` | F02, F05 |
| `legacy/examples/runs/J-0001/workspecs/implement.json` | F02, F05 |
| `legacy/examples/security-contracts/audit-event.example.json` | F17 |
| `legacy/examples/security-contracts/cost-metric.fake.example.json` | F17 |
| `legacy/examples/security-contracts/privacy-handoff.example.json` | F16 |
| `legacy/examples/security-contracts/redaction-report.example.json` | F16 |
| `legacy/examples/surface-contracts/api-job-response.example.json` | F14 |
| `legacy/examples/surface-contracts/daemon-state.example.json` | F13 |
| `legacy/examples/surface-contracts/ui-job-view.example.json` | F15 |
| `legacy/examples/validation-contracts/approval-request.example.json` | F11 |
| `legacy/examples/validation-contracts/approval-response.example.json` | F11 |
| `legacy/examples/validation-contracts/review-pack-handoff.example.json` | F11 |
| `legacy/examples/validation-contracts/validation-decision.human-review.example.json` | F09, F11 |


## 부록 D. 자료 사이의 불명확한 점

아래 항목은 어느 한 자료를 정답으로 선택하지 않았다. 기능을 이해할 때 주의해야 할 설명 차이만 기록한다.

| 번호 | 불명확한 점 | 함께 확인할 역사적 근거 |
|---|---|---|
| U01 | 개요 문서는 Schema·Registry·도구 메타데이터를 세부 기준으로 설명하지만 구현 문서는 구현 문서·작업 queue·브리프에 별도 우선순위를 둔다. | `legacy/docs/00_개요.md`, `legacy/docs/implementation/README.md` |
| U02 | `provider_id`와 `provider_instance_id`를 같게 써도 된다는 설명과 나중에 엄격히 분리한다는 설명이 함께 있다. | `legacy/docs/implementation/data-contracts.md` |
| U03 | ProviderAdapter 수명주기는 prepare·execute·cancel·collect·healthcheck 형태와 단일 execute 형태로 각각 설명된다. | `legacy/docs/implementation/provider-system.md`, `legacy/specs/contracts/provider-adapter.md` |
| U04 | HumanProvider의 승인 요청·응답 저장 책임이 HumanProvider, ValidationEngine, control plane에 각각 배치되어 있다. | `legacy/docs/implementation/provider-system.md`, `legacy/docs/implementation/validation-engine.md`, `legacy/docs/implementation/artifact-layout.md` |
| U05 | `remote_self_hosted_model`의 분류 위치가 local model/server와 remote agent 양쪽 설명에 등장한다. | `legacy/docs/implementation/provider-system.md` |
| U06 | 기본 transport는 CLI·HTTP·process·manual로 설명되지만 다른 문서에는 stdio·websocket·file handoff도 등장한다. | `legacy/docs/providers/provider-model.md`, `legacy/docs/implementation/provider-system.md` |
| U07 | Provider kind Schema에는 8종이 있지만 `local_anthropic_compatible_server`, `remote_self_hosted_model`에 대응하는 builtin manifest 예시는 없다. | `legacy/specs/schemas/provider-kind.schema.json`, `legacy/configs/registries/builtin-provider-registry.yaml` |
| U08 | `provider-selection.yaml`의 `codex`, `local-ollama`와 Registry의 `provider.codex-cli`, `provider.ollama` 사이 별칭 변환 규칙이 설명되지 않는다. | `legacy/configs/policies/provider-selection.yaml`, `legacy/configs/registries/builtin-provider-registry.yaml` |
| U09 | `capability`, `provider-capability`, `capability-profile`이 서로 다른 층의 능력을 표현하지만 층 사이 연결 규칙은 짧게만 설명된다. | `legacy/specs/schemas/capability.schema.json`, `legacy/specs/schemas/provider-capability.schema.json`, `legacy/specs/schemas/capability-profile.schema.json` |
| U10 | `model-routing.yaml`이 참조하는 `validator`, `reviewer-lite`와 같은 이름의 역할 자료가 `configs/roles`에서 확인되지 않는다. | `legacy/configs/policies/model-routing.yaml`, `legacy/configs/roles/` |
| U11 | `provider-result`와 `provider-run-result`는 상태 이름, 대소문자와 필드 구성이 다르다. 두 계약의 변환 규칙은 자료에서 하나로 고정되지 않는다. | `legacy/specs/schemas/provider-result.schema.json`, `legacy/specs/schemas/provider-run-result.schema.json` |
| U12 | 공통 Diagnostic·ValidationRun과 Star Sentinel 전용 계약은 severity, status와 필드 구성이 다르다. | `legacy/specs/schemas/diagnostic.schema.json`, `legacy/specs/schemas/validation-run.schema.json`, `legacy/builtin-tools/star-sentinel/schemas/` |
| U13 | 주요 Schema의 `schema_version: 1.0.0`, 설정의 `0.1.0`, 숫자 `version: 1`, 버전 필드가 없는 자료가 함께 있다. | `legacy/specs/schemas/`, `legacy/configs/` |
| U14 | Hook Schema가 요구하는 구조와 Hook 설정 15개의 `event`, `description`, 빈 `steps` 구조가 직접 맞지 않으며 변환 설명이 없다. | `legacy/specs/schemas/hook.schema.json`, `legacy/configs/hooks/` |
| U15 | Role·Skill Schema는 구조화 JSON이지만 실제 역할·Skill 자료는 Markdown 지시문이며 변환 관계가 명시되지 않는다. | `legacy/specs/schemas/role.schema.json`, `legacy/specs/schemas/skill.schema.json`, `legacy/configs/roles/`, `legacy/configs/skills/` |
| U16 | Renderer 자료 27개는 파일명과 간단한 상태 설명이 중심이어서 구체적인 입력·출력과 호출 관계가 모두 설명되지는 않는다. | `legacy/configs/renderers/` |
| U17 | Route는 stage별 provider 배정을 기록하지만 provider 산출물 경로는 주로 instance ID로 구분한다. 같은 instance가 여러 stage를 실행할 때의 구분 방식은 명시되지 않는다. | `legacy/docs/implementation/data-contracts.md`, `legacy/docs/implementation/artifact-layout.md` |
| U18 | 재시도 산출물은 provider 폴더의 평면 구조와 `attempt-0001` 하위 구조로 각각 설명된다. | `legacy/docs/implementation/artifact-layout.md`, `legacy/docs/implementation/artifact-naming.md` |
| U19 | 최종 보고서는 작업 폴더 바로 아래 `final-report.md`와 `reports/final-report.json` 두 위치·형식으로 설명된다. | `legacy/docs/operations/run-artifacts.md`, `legacy/docs/implementation/artifact-naming.md` |
| U20 | 승인 파일 위치는 `.star-control/approvals/`와 `.ai-runs/{job_id}/approvals/`로 각각 설명되며 두 폴더의 관계가 명시되지 않는다. | `legacy/docs/operations/run-artifacts.md`, `legacy/docs/implementation/artifact-layout.md` |
| U21 | 간단한 `approval` 계약과 단계별 `approval-request`·`approval-response` 계약이 별도로 존재하지만 관계가 설명되지 않는다. | `legacy/specs/schemas/approval.schema.json`, `legacy/specs/schemas/approval-request.schema.json`, `legacy/specs/schemas/approval-response.schema.json` |
| U22 | Daemon은 장시간 Job Queue로 설명되면서도 scheduler 동작은 제한된 tick 중심이고 Windows 기본 상태·로그 위치도 하나로 정해지지 않는다. | `legacy/docs/implementation/daemon-contract.md`, `legacy/apps/star-daemon/README.md` |
| U23 | API와 UI 자료에는 조회 전용 단계와 approve·cancel·resume 제어 단계가 함께 있다. 자료가 서로 다른 확장 단계를 설명한다. | `legacy/docs/implementation/ui-shell-contract.md`, `legacy/docs/implementation/cli-daemon-api-ui.md` |
| U24 | 복구 문서는 원본 보존과 자동 교체 금지를 말하면서 별도 항목에서는 승인된 삭제, event log와 artifact 교체를 설명한다. 검사·계획·실행 경계를 함께 봐야 한다. | `legacy/docs/implementation/state-store-recovery.md` |
| U25 | `recovered-copy`는 token 없이 가능한 행동과 token 일치가 필요한 행동으로 각각 설명된다. | `legacy/docs/implementation/state-store-recovery.md`, `legacy/docs/implementation/cli-command-reference.md` |
| U26 | Release 문서의 `release_actions_performed`에는 로컬 계획·결과 작성도 포함되지만 실제 외부 행동은 `external_actions_performed`로 따로 구분한다. | `legacy/docs/implementation/release-readiness.md` |
| U27 | signing 방식과 rollback 정책은 기능 이름과 검사 항목은 있으나 구체적인 실행 계약이 모두 정해져 있지는 않다. | `legacy/docs/implementation/release-readiness.md` |
| U28 | Star Sentinel P0의 다섯 규칙과 확장 profile의 더 넓은 규칙 목록은 서로 다른 범위 층위다. | `legacy/docs/implementation/star-sentinel-p0-contracts.md`, `legacy/docs/implementation/star-sentinel-full-spec.md` |
| U29 | error taxonomy에는 `BLOCK`, `BLOCKED`, `WAIT_APPROVAL`, `STOPPED`가 섞여 있으며 RunState 상태 이름과 완전히 일치하지 않는다. | `legacy/configs/policies/error-taxonomy.yaml`, `legacy/specs/schemas/run-state.schema.json` |
| U30 | Provider README 20개는 설명 깊이가 서로 다르다. 한 문서는 실행·승인·Privacy·Cost artifact를 상세히 다루지만 나머지는 manifest 등록 설명이 중심이다. | `legacy/builtin-providers/` |
| U31 | MVP Runbook에는 `init`, `validate`, `provider check`, `render` 명령이 나오지만 상세 CLI 명령 문서는 `run`, 조회·제어, `providers`, `sentinel`, `release`, `recover`를 중심으로 설명한다. 같은 이름의 확정된 명령 집합으로 보지 않았다. | `legacy/docs/operations/Star-Control_MVP_Runbook.md`, `legacy/docs/implementation/cli-command-reference.md` |
| U32 | Provider session은 Daemon 책임과 UI dashboard 목표로 언급되지만 별도 Session Schema, ID, 수명주기, 저장 위치와 API 계약은 설명되지 않는다. | `legacy/docs/implementation/cli-daemon-api-ui.md`, `legacy/docs/implementation/complete-implementation-roadmap.md` |

### 불명확 항목과 기능 연결

| 기능군 | 관련 불명확 항목 |
|---|---|
| F01 | U01 |
| F02 | U29 |
| F03 | U10, U11, U13, U14, U15, U16, U21, U29, U31 |
| F04 | U17, U18, U19, U20 |
| F05 | U08, U10, U17 |
| F06 | U02, U03, U05, U06, U07, U08, U09, U30, U31 |
| F07 | U02, U03, U06, U11, U17, U18, U29 |
| F08 | U04, U05, U06, U07, U30 |
| F09 | U04, U12, U29 |
| F10 | U12, U28 |
| F11 | U04, U20, U21, U29 |
| F12 | U25, U31 |
| F13 | U22, U32 |
| F14 | U23 |
| F15 | U23, U32 |
| F16 | 직접 연결된 불명확 항목 없음 |
| F17 | 직접 연결된 불명확 항목 없음 |
| F18 | U24, U25, U29 |
| F19 | U26, U27 |
| F20 | 직접 연결된 불명확 항목 없음 |

## 조사 결과의 경계

- 본문은 레거시 자료에 등장한 기능의 목적과 연결 관계를 설명한다.
- 부록의 대응표는 조사 대상 자료가 어느 기능군의 근거가 되었는지 보여준다.
- 자료에서 설명되지 않은 내부 동작은 추측해 채우지 않았다.
- 불명확한 설명은 부록 D에 병기했으며 이 문서에서 하나의 계약으로 확정하지 않았다.
- 새 Star-Control의 기능 범위와 구현 계약은 이 카탈로그가 아니라 새 설계 문서에서 정한다.
