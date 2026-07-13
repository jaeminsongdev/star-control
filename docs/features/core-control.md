# 핵심 관제 기능

상위 범위와 공통 선정 기준은 [구현 대상 기능](README.md)에서 확인한다.


## A01. 목표·작업 계약

Star-Control은 자연어 요청을 바로 실행하지 않고 다음 내용을 질문으로 확정한다.

- 최종 목표와 성공한 결과의 모습
- 포함 범위와 하지 않을 일
- 대상 프로젝트와 허용·금지 경로
- 필요한 결과물과 완료 조건
- 돈이 드는 행동, 외부 변경, 파괴 행동의 승인 조건
- 사용자가 이미 정한 기술·설계·우선순위

확정된 계약은 이후 단계 계획, 권한, 검사와 최종 판정의 기준이 된다. 작업 도중 목표가 바뀌면 원래 계약을 조용히 덮지 않고 변경 이유와 새 범위를 기록한다.

## A02. 단계 계획과 재계획

작업은 파일 수나 코드 줄 수가 아니라 모델·생각 깊이·실행 방식·검증 방식이 달라지는 경계로 나눈다.

- 조사, 설계, 구현, 검증, 검토, 병합처럼 성격이 다른 단계 분리
- 단계 사이 선행 조건과 결과 전달 관계 기록
- 서로 독립적인 단계의 병렬 가능성 판단
- 같은 성격의 큰 작업은 지나치게 잘게 나누지 않음
- 단계별 목표, 입력 자료, 결과, 완료 조건, 실패 처리 정의
- 예상 밖 변경, 새 위험, 검사 실패, 범위 확대 시 재계획
- 원래 계획과 실제 실행의 차이 기록

## A03. 프로젝트 이해와 Context Pack

매 작업마다 전체 저장소를 다시 읽지 않도록 current checkout의 최소 사실과 현재 작업에 필요한 자료를 snapshot으로 묶는다. 상세 identity·discovery·index·freshness 계약은 [읽기 전용 Project Catalog와 Code Index](../contracts/project-catalog-and-code-index.md)가 소유한다.

- 여러 explicit root, Git·non-Git Project, nested repository, submodule, build workspace와 linked worktree 발견
- stable ProjectId와 local CheckoutId를 분리하고 같은 Project의 dirty worktree를 서로 다른 WorkspaceSnapshot으로 유지
- source, test, docs, config, schema, migration, generated, vendor, cache, output과 fixture·docs example facet 분류
- 언어, build system, package manager, toolchain, lockfile, 주요 명령 발견
- 적용 scope가 있는 AGENTS, README, 설계 문서, 정책과 프로젝트별 정본 우선순위·충돌 확인
- text search, syntax index와 available semantic index를 실제 tier·coverage·limitation과 함께 사용
- package·module·symbol·definition·reference와 project·contract·dependency graph 탐색
- config key, Schema ID, error code, 전역 상수, public surface와 hardcoding Finding 후보 탐색
- 작업 유형별로 필요한 파일, 계약, 최근 변경과 검증 명령 선택
- 각 Context 항목에 ProjectId·CheckoutId, source hash, 포함 이유, source authority, index tier, freshness와 누락 가능성 기록
- token·자료량 한도와 단계별 Context Profile
- working tree의 staged·unstaged·untracked actual byte를 HEAD·default branch보다 최신 사실로 반영

Project Catalog와 Code Index는 Git source를 대체하지 않는 derived projection이다. 최초 scan은 CLI에서 수동 실행하고 이후 Git revision·file hash 기반 incremental scan을 사용한다. semantic adapter가 없으면 syntax·text로 fallback하고 그 한계를 숨기지 않는다. 이 기능은 project source를 수정하거나 자체 scheduler·AI 호출을 요구하지 않는다.

현재 A03의 이 확장은 **1단계 목표 설계**이며 scanner·parser·DB·watcher와 CLI 제품 구현 완료를 뜻하지 않는다.

## A04. 변경 영향·위험 분석

요청과 실제 변경을 비교해 무엇을 검사하고 어느 수준으로 다뤄야 하는지 계산한다.

- add, modify, delete, rename, mode, binary, submodule을 포함한 Git 변경 구조화
- 변경 파일·줄·심볼·패키지·계약·설정·테스트 관계 수집
- source, test, dependency, workflow, schema, migration, security, release 등 변경 종류 분류
- auth, secret, dependency manifest, CI, validator, policy, migration, release 등 위험 경로 표시
- 직접 영향과 전이 영향, 확인된 영향과 추정 영향을 구분
- 분석 결과에 출처, confidence, limitation과 no-result 이유 기록
- 불확실성이 높거나 위험이 크면 검사 범위를 package·workspace·전체로 승격
- 요청과 관계없는 변경, 과도한 diff와 숨은 변경 탐지
- 관련 테스트와 계약, 검토 지점 추천

## A05. Codex 능력 확인과 단계별 배정

Star-Control은 다른 AI 제공자를 선택하지 않는다. 실행자는 Codex 하나이며 다음 Codex 내부 선택만 관리한다.

- 실행 시점에 사용할 수 있는 모델, 생각 깊이, Max, 병렬 기능과 도구 능력 확인
- 단계별 필수 능력과 권한을 먼저 적용하는 hard constraint
- 작업 복잡도, 위험, 검증 가능성, 비용·한도에 따른 배정
- 설계·구현·검증·독립 검토 단계의 서로 다른 배정
- 지원되지 않거나 한도에 걸린 선택의 안전한 대체와 중단
- 적합한 실행 방식이 없을 때 억지로 배정하지 않고 질문 또는 중단
- 배정 이유와 대체 이유를 사람이 읽을 수 있게 표시
- 사용자의 수동 배정이 자동 선택보다 우선

## A06. Codex 실행 제어와 터미널 조작

Codex의 공식 통합 지점을 사용해 계획된 단계를 실제 작업으로 연결한다.

- Plugin·MCP·Hook을 통한 Star-Control 시작과 진입 검사
- Codex 제어 기능 초기화와 지원 기능 확인
- 단계별 새 작업 생성, 기존 작업 재개, 분기, 중단과 상태 조회
- 모델·생각 깊이·권한·Context Pack과 단계 지시 전달
- 단계 결과와 다음 단계의 인계 자료 수집
- 장시간 작업을 감시하고 상태를 복구하는 Windows 배경 Controller
- 목표 목록, 현재 단계, 진행 상태, 질문, 중단, 재개, 취소를 다루는 터미널 명령
- Plugin·Hook·MCP가 꺼졌거나 신뢰되지 않을 때 닫힌 상태로 중단

Controller는 계획된 작업을 이어주는 역할만 한다. 반복 시간표와 예약 실행은 Codex가 제공하는 기능을 사용한다.

## A07. 상태·Checkpoint·이어하기·자체 복구

Star-Control 자신의 장시간 작업 상태는 로컬 파일에 안전하게 보존한다.

- 목표, 단계 계획, 배정, 권한, 상태, 질문, 검사, 비용, 병합과 최종 결과 저장
- 요청, 실행 중, 검사 중, 승인 대기, 차단, 실패, 취소, 완료 상태 구분
- 원자적 저장, 경로 이탈 방지, 추가 전용 사건 기록과 artifact 참조
- 중복 실행과 같은 단계의 동시 변경을 막는 lock
- 단계·병합·외부 행동 전 Checkpoint
- 앱 종료, 대화 변경과 작업 중단 뒤 재개
- 새 대화가 바로 이어갈 수 있는 목표·진행·변경·검사·남은 일 요약
- 손상 JSON, 잘린 기록, 남은 임시 파일과 누락 artifact의 읽기 전용 검사
- 원본을 보존하는 복구본, dry-run 계획, 승인된 정리·교체와 복구 결과 기록
- 기록 보존 기간과 정리 명령

## A08. 권한·승인·격리·비밀정보 보호

권한은 사람 수가 아니라 행동의 영향으로 판단한다.

- 행동별 자동 실행, 본인 확인, 금지 설정
- 공개 배포용 `safe_default`와 개인용 `personal_auto` 분리
- 개인 기본값은 유료 사용, 외부 상태 변경, 삭제·덮어쓰기처럼 되돌리기 어려운 행동을 확인 대상으로 설정
- 프로젝트 경로, 명령 종류, network, environment, secret 접근과 실행 시간 제한
- dependency·workflow·validator·policy·release·계정·권한 변경의 별도 취급
- 승인 요청에 행동, 영향 대상, 비용, 위험, 증거와 되돌리기 방법 표시
- 계획이나 대상이 바뀐 오래된 승인을 재사용하지 않음
- raw shell 문자열보다 등록된 명령과 구조화 인자 우선
- secret·token·개인정보 후보를 Context, log, report와 외부 전달에서 가림
- 어떤 자료를 어디에 전달했는지 기록

## A09. Worktree·병렬 작업·병합

혼자 여러 Codex 작업을 동시에 진행할 때 기존 변경과 결과를 잃지 않게 한다.

- 시작 전 branch, dirty state, base revision과 사용자 변경 확인
- 독립 단계별 Git worktree 생성·식별·정리
- 같은 파일·심볼·계약을 수정할 가능성이 있는 단계의 병렬 실행 차단
- 병렬 실행 수와 자원 한도
- 각 worktree의 단계 결과, diff, 검사와 review 자료 보존
- 병합 대기열과 의존 순서
- 병합 전 최신 base와 충돌 가능성 확인
- 충돌 원인과 양쪽 의도를 보여주고 Codex 수정 또는 사용자 판단 연결
- 병합 뒤 통합 검사와 목표 전체 완료 조건 재검사
- 로컬 병합과 원격 검토·병합 상태를 구분

## A10. 작업·도구·검증·프로필 Registry

여러 프로젝트의 반복 절차를 코드에 박아 넣지 않고 선언한다.

- 프로젝트 Task ID와 format, lint, build, test, docs, security, release 명령
- 도구의 목적, 입력·결과, side effect, 권한, timeout과 결과 parser
- 검증 Profile과 선행 관계, 실패 정책, cache 가능 여부
- 위험 경로, 계약, 허용·금지 행동, 승인 정책
- 개발 작업 Profile의 단계·Context·도구·검사·증거 기본값
- 설정 계층과 project·user·run override
- effective config 조회와 출처 설명
- 설정·템플릿·정책 version과 변경 기록

이 descriptor Catalog는 A03의 Project Catalog와 다르다. A03은 실제 Project·Checkout·source를 관찰한 snapshot이고, A10은 Task·Tool·Rule·Profile 선언의 정본이다. 발견한 manifest script·문서 명령은 provenance와 confidence를 가진 command 후보일 뿐 이 단계에서 실행하지 않는다. hardcoding detector threshold와 class별 제외 규칙은 versioned Rule·Policy로 선언하고 scanner code에 고정하지 않는다.
