# CI 운영 로드맵

## 0단계: 현재 적용된 초기 CI

현재 `main`에는 최소 안정 CI만 적용한다.

- 저장소 기본 구조 확인
- Star Sentinel manifest 최소 계약 확인

이 단계의 목적은 AI 작업 PR에 대해 가장 낮은 비용의 자동 검증선을 먼저 세우는 것이다.

## 1단계: 데이터 형식 검사

정본 경로 기준으로 데이터 파일 파싱 검사를 추가한다.
전체 저장소를 한 번에 검사하면 원본 흡수 문서와 예시 산출물 때문에 오탐 가능성이 있으므로, 초기에는 아래 경로부터 시작한다.

- `.github/workflows/`
- `configs/`
- `specs/`
- `builtin-tools/`
- `builtin-providers/`
- `examples/`

검사 대상은 JSON, YAML, TOML이다.

## 2단계: 문서 품질 검사

설계 문서가 늘어나면 문서 전용 검사를 추가한다.

- 빈 링크 검사
- 내부 상대 링크 검사
- 문서 제목 중복 검사
- 문서 읽는 순서 문서와 실제 파일 존재 여부 비교
- 오래된 원본 문서와 정본 문서의 혼동 방지 검사

## 3단계: 명칭 정책 검사

Star-Control은 명칭과 package 경계가 중요하므로 별도 검사를 둔다.

- Star Sentinel 정식 명칭 사용 여부
- legacy alias 사용 위치 제한
- provider-neutral package 경계 확인
- core package 이름에 특정 provider 제품명이 들어가지 않는지 확인
- builtin provider manifest와 core package의 책임 경계 확인

## 4단계: 스키마 검증

`specs/`가 안정되면 schema 기반 검사를 추가한다.

- schema 파일 자체의 파싱 가능 여부
- manifest 예시가 schema를 만족하는지 확인
- provider manifest 검증
- tool manifest 검증
- capability registry 검증
- run ledger, approval, review pack 관련 산출물 schema 검증

## 5단계: 구현 패키지 생성 후 언어별 검사

`packages/` 아래 실제 구현이 생기면 언어별 CI를 추가한다.

Rust 패키지가 생기면 다음 성격의 검사를 추가한다.

- formatting 검사
- workspace check
- clippy 기반 lint
- workspace test

TypeScript 패키지가 생기면 다음 성격의 검사를 추가한다.

- dependency lock 준수
- lint
- typecheck
- test

Python 패키지가 생기면 다음 성격의 검사를 추가한다.

- import / compile 검사
- lint
- test

## 6단계: Star Sentinel selfcheck

Star Sentinel 구현이 생기면 자체 검증 명령을 CI에 연결한다.

- quick profile selfcheck
- quick profile check
- policy corpus 검사
- approval gate 판정 샘플 검사
- review pack 생성 샘플 검사

## 7단계: PR 보호 설정

초기 CI가 안정적으로 통과한 뒤 `main`에 보호 규칙을 건다.

- PR 없이 merge 금지
- 필수 status check 통과 전 merge 금지
- 대화 해결 전 merge 금지
- 강제 push 금지
- branch 삭제 금지

초기 필수 status check 후보는 다음과 같다.

- `repository-policy-check`
- `manifest-contract-check`

데이터 형식 검사와 명칭 정책 검사가 안정화되면 필수 status check에 추가한다.

## 8단계: 보안 및 운영 정책 검사

초기에는 오탐을 줄이기 위해 강한 정책 검사를 넣지 않는다. 이후 별도 PR로 다음 검사를 추가한다.

- 민감정보 포함 여부 검사
- 실행 산출물 위치 검사
- workflow 변경 위험도 검사
- 외부 action 사용 정책 검사
- 권한 상승 가능성이 있는 workflow 패턴 검사
- 의존성 추가 여부 검사

## 9단계: 비용과 시간 최적화

CI가 무거워지면 비용과 시간을 줄이는 정책을 추가한다.

- 변경 경로 기반 job 실행
- 문서만 바뀐 PR에서는 코드 테스트 생략
- 코드가 바뀐 PR에서는 관련 package 테스트 실행
- 캐시 사용 기준 문서화
- 긴 테스트와 빠른 테스트 분리

## 운영 원칙

- CI는 검증자이고 구현자가 아니다.
- CI workflow 기본 권한은 읽기 중심으로 유지한다.
- 초기 CI에서는 배포나 공개 작업을 하지 않는다.
- Codex 또는 다른 AI가 CI를 수정하는 PR은 고위험 변경으로 본다.
- 실패한 검사를 삭제하거나 약화해서 통과시키지 않는다.
- 단계별로 작은 PR을 만들어 안정화한 뒤 필수 status check로 승격한다.
