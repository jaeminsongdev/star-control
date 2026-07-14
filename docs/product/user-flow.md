# 사용자 경험과 전체 흐름

## 사용자가 보는 시작점

사용자는 두 시작점을 가진다.

1. Codex 앱에서 평범한 말로 개발 목표를 입력하고 Plugin을 통해 Star-Control 흐름을 시작한다.
2. 터미널에서 `star` CLI로 Project 조회·변경 계획·검증·release·평가 command를 직접 실행한다.

CLI-only 경로는 Codex·AI 호출 없이 결정적 Project Catalog·Code Index·영향 분석·검사·Gate와 상태 기록을 사용한다. Codex 경로도 별도 상태기계를 만들지 않고 같은 Controller application command를 호출한다.

    이 프로젝트의 설정 구조를 정리하고 오류 처리를 통일한 뒤 전체 검사를 해줘.

사용자는 별도 Star-Control 화면을 열지 않는다. Codex를 사용할 때는 설치된 Star-Control Plugin이 개발 작업인지 판단하고, CLI-only에서는 사용자가 typed command와 필요한 TaskSpec을 직접 제공한다.

단순 설명, 번역, 아이디어 대화처럼 파일이나 외부 상태를 바꾸지 않는 요청은 일반 Codex 대화로 처리할 수 있다. 파일 수정, 명령 실행, 원격 작업처럼 실제 개발 동작을 시작할 때는 Star-Control 실행 기록이 필요하다.

## 전체 흐름

1. 사용자가 Codex 앱에서 목표를 입력하거나 CLI에서 typed TaskSpec을 제공한다.
2. Star-Control이 목표의 대상, 완료 모습, 제한, 위험을 확인한다.
3. 애매한 내용이 결과를 바꿀 수 있으면 지원하는 entry에서 사용자 결정을 받는다.
4. 목표를 성격이 같은 실행 단계로 나누거나 CLI-only pure planner가 TaskSpec을 결정적 stage graph로 계산한다.
5. deterministic local 단계에는 Tool·권한·검사만 배정하고, Codex executor 단계에만 모델·생각 깊이·실행 방식을 추가 배정한다.
6. 전체 계획·권한·검사와 해당하는 경우 Codex 배정 결과를 entry에 출력한다.
7. 사용자가 단계 내용과 순서를 수정하면 새 ScopeRevision으로 재계획한다.
8. 사용자가 계획을 승인하면 승인 범위 안의 일반 실행을 진행한다.
9. Controller가 deterministic application command를 실행하고, 선택된 Codex 단계만 Codex 작업을 만들어 같은 상태·증거로 수집한다.
10. 실패하면 계약에 선언된 횟수·idempotency·승급 규칙 안에서만 다시 시도한다.
11. ValidationPlan에 선택된 검사를 실행한다.
12. 통과 여부, 변경 내용, 남은 위험과 실제 측정된 비용을 기록한다.
13. 다음 단계가 있으면 필요한 typed handoff만 이어서 전달한다.
14. 모든 완료 조건이 current evidence로 충족되면 완료 처리한다.
15. CI·release 대상이면 같은 Task ID·revision·tool·config·Profile로 local quick→target→full→release Gate를 진행한다.
16. 한 번 build·package해 봉인한 artifact를 검증하고 같은 digest를 승격한다.
17. release가 `ready`여도 publish·deploy는 실행하지 않고 exact 사용자 승인을 기다린다.
18. 승인된 원격 동작 뒤 실제 after-state를 확인했을 때만 `published`로 표시한다.
19. Rule·Check·Profile·Recipe의 효용은 별도 EvaluationRun에서 baseline/candidate로 비교하고 자동으로 설정을 바꾸지 않는다.

일반 로컬 목표의 최종 완료에는 별도 사람 승인을 강제하지 않는다. 단, 사용자가 설정으로 최종 승인을 요구할 수 있다. publish·deploy·원격 변경·유료 동작·파괴적 rollback/purge는 목표 완료와 별개이며 현재 action에 결합한 명시적 승인이 필요하다.

## 질문 원칙

다음 경우에는 먼저 질문한다.

- 같은 문장이 서로 다른 결과를 만들 수 있을 때
- 완료 기준을 알 수 없을 때
- 유료 동작이 필요할 때
- 현재 설정으로 허용되지 않는 동작이 필요할 때
- 여러 안전한 선택 중 사용자의 의도가 결과를 크게 바꿀 때

사소한 구현 세부사항은 계획과 프로젝트 규칙 안에서 스스로 결정한다.

## 계획 출력에 보여줄 내용

- 목표 요약
- 실행 단계와 순서
- 동시에 실행할 수 있는 단계
- Codex executor 단계에 배정된 모델과 생각 깊이
- 해당하는 경우 Max, Ultra, 독립 검토 사용 여부
- 수정 예정 범위
- 실행 예정 검사
- 예상 비용 등급과 유료 동작
- 자동 재시도 횟수
- 완료 조건

## 진행 상황

Codex 앱에는 현재 단계, 진행 중인 작업, 최근 결과, 다음 단계를 보여준다. 터미널에서는 더 자세한 상태와 기록을 확인할 수 있다.

기본 명령은 다음 책임을 가진다.

    star start
    star plan
    star approve
    star status
    star pause
    star resume
    star cancel
    star evidence
    star close
    star release plan
    star release status
    star eval compare

실제 명령 이름과 옵션은 구현 계약 단계에서 확정한다.

## Release와 설치 수명주기

사용자는 release 상태를 다음처럼 본다. 정확한 Gate·remote action·평가 흐름은 [10단계 CI·Release·평가 정본](../contracts/ci-release-evaluation-and-product-completion.md)이 소유한다.

- `candidate`: artifact byte가 봉인됐지만 release 검사가 끝나지 않음
- `ready`: clean Windows·package·install lifecycle 검사가 통과함
- `approved`: exact artifact·channel의 주 publication action을 사용자가 승인함
- `published`: 원격 after-state가 실제 version·source·artifact digest를 확인함
- `publish_outcome_unknown`: 원격 effect를 확인할 수 없어 read-only 재확인이 필요함
- `rollback_required`: install·update·deploy 실패 뒤 복구가 필요함

deploy·withdraw·remote rollback은 target별 remote action에서 별도 `approved|running|verified|outcome_unknown|rollback_required`를 보여준다. deploy 승인이 이미 확인된 top-level `published`를 되감거나 한 target의 성공이 다른 target의 결과를 대신하지 않는다.

clean install은 `safe_default`로 첫 실행한다. update는 이전 artifact·상태 backup을 보존하고, 실패하면 검증된 이전 version으로 rollback한다. uninstall은 기본적으로 program payload와 startup entry만 제거하고 user config·management state·project evidence를 보존한다. 사용자 자료 purge는 별도 파괴 동작이다.

## 규칙 평가 흐름

평가는 실행 중인 작업을 몰래 바꾸지 않는 offline·replay·shadow mode부터 시작한다. baseline과 candidate는 같은 case·source·tool·environment에서 비교하고, 실제 결함·false positive·flaky·suppression·재작업·실패·시간을 기록한다. 비용은 provider가 검증 가능한 자료를 제공했을 때만 기록한다.

추천은 `keep`, `trial`, `accept`, `reject`, `needs_review` 중 하나다. 추천만으로 Catalog·Profile·Rule을 자동 수정하지 않으며, 검증기·Corpus·Gate를 약화해 통과율을 높인 candidate는 accept할 수 없다. CLI-only와 Codex 연동 결과는 별도 평가 context다.

## 중단과 이어하기

- 사용자는 작업을 멈추거나 취소할 수 있다.
- 앱을 닫아도 실행 상태와 이어하기 기록은 남는다.
- 재개할 때 전체 대화를 다시 읽지 않는다.
- 실패한 단계는 실패 이유와 이미 시도한 방법을 다음 실행에 전달한다.
- 재시도 횟수와 자동 승급 여부는 설정 파일과 명령어로 바꿀 수 있다.

## 프로젝트별 설정

프로젝트마다 다음 내용을 저장할 수 있다.

- 우선 읽을 문서
- 수정 허용 범위
- 건드리면 안 되는 경로
- 자주 사용하는 검사
- 비용 한도
- 자동 승인 범위
- 병렬 작업 한도
- 원격 저장소 사용 규칙
- 결과 보관 기간
