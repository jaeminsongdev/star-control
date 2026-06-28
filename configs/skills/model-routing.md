# model-routing

## 목적
A cloud, B local, high-cloud, low-cloud 등 provider/model tier를 stage별로 배정하는 절차다.

## 기준
- SMALL: router-low 또는 local draft 가능
- MEDIUM: cloud worker 기본, local은 보조
- LARGE: high-cloud 필수, local은 요약/초안만
- CRITICAL: high-cloud + security/review 필수, 사용자 승인 필요

## 로컬 모델 금지
- 보안/권한 최종 판단
- DB/스키마 최종 설계
- 공개 API 변경
- 대형 리팩토링
- 테스트 실패 우회 판단
