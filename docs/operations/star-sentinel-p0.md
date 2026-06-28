# Star Sentinel P0 운영 메모

이 문서는 Star Sentinel P0 자산의 위치와 역할을 짧게 고정한다.

## 목적

P0는 구현 전 단계에서 최소 검증 계약을 고정하기 위한 기준선이다. 실행 코드가 아니라 policy, schema, fixture, example의 정합성을 먼저 맞추는 데 목적이 있다.

## 파일 위치

- `builtin-tools/star-sentinel/policies/p0-policy.yaml`: P0 정책 기준
- `builtin-tools/star-sentinel/schemas/`: P0 산출물 schema
- `builtin-tools/star-sentinel/fixtures/p0/`: 정책 판정용 fixture
- `builtin-tools/star-sentinel/examples/p0/`: schema 검증용 example
- `scripts/ci/check_schema_examples.py`: 일부 example을 schema와 연결해 검증

## 판정 의미

- `AUTO_PASS`: 자동 통과 가능
- `HUMAN_REVIEW`: 사람 확인 필요
- `BLOCK`: 자동 진행 금지

## 운영 기준

- Star-Control core는 Star Sentinel 구현 세부사항에 직접 결합하지 않는다.
- Star Sentinel은 builtin tool 경계 안에서 policy와 evidence 기반 검증을 담당한다.
- P0 정책, schema, fixture, example 변경은 작은 PR로 나누고 CI 결과를 확인한다.
- 신규 의존성이나 package manager 도입은 명시 승인 전까지 하지 않는다.
