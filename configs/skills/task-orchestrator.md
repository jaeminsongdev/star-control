# task-orchestrator

## 목적
사용자 요청을 설계, 구현, 검증, 리뷰, 폴리싱, 보고 단계로 분해하는 절차다.

## 절차
1. 요청을 기능/품질/검증 기준으로 재정의한다.
2. 작업 규모를 SMALL / MEDIUM / LARGE / CRITICAL로 분류한다.
3. 위험도를 LOW / MEDIUM / HIGH / CRITICAL로 분류한다.
4. PLANS.md 또는 RunState 갱신 필요 여부를 판단한다.
5. model-routing policy에 따라 stage별 role/provider를 정한다.
6. WorkSpec을 작성한다.
7. 승인 필요 항목이 있으면 NEEDS_APPROVAL로 중단한다.
8. worker를 순차 실행한다.
9. validation과 review 결과를 수집한다.
10. BLOCKER가 없으면 polish/report로 진행한다.
