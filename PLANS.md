# PLANS.md

## 목적

현재 작업 상태를 짧게 유지하는 원장이다. 상세 로그, 전체 diff, 반복 검증 출력은 여기에 누적하지 않는다. 장기 보존이 필요한 근거는 `docs/decisions/*`, report, changelog, commit history에 둔다.

## Context Pack

### 현재 목표

- Star-Control 설계 흡수 스캐폴드는 완료됨.
- `PLANS.md`는 bounded snapshot으로 유지한다.

### 반드시 지켜야 할 제약

- 원본 설계 폴더는 별도 승인 없이 삭제하지 않는다.
- 의존성 추가, 패키지 매니저 도입, 원격 공개 작업은 명시 요청이 있을 때만 한다.
- 실행 결과는 Star-Control repo가 아니라 대상 프로젝트 `.ai-runs/`에 둔다.

### 이미 끝난 것

- v3/v4 원본 237개 파일을 정규 구조로 흡수했다.
- 흡수 감사에서 mapped target 누락 0개, content absorption failure 0개를 확인했다.
- GitHub `origin/main`에 설계 흡수 커밋까지 반영했다.

### 아직 남은 것

- 실제 구현 언어와 패키지 매니저 결정.
- 원본 설계 폴더 삭제 여부는 사용자 별도 승인 필요.

### 건드리면 안 되는 것

- `D:/개발/관제/star-control_design_v3`
- `D:/개발/관제/custom_dev_verification_platform_design_v4_curated`
- 사용자 승인 없는 의존성 설치, 파일 삭제, 테스트 약화.

### 먼저 확인할 파일

- `README.md`
- `docs/decisions/source-absorption-map.md`
- `docs/decisions/source-absorption-audit.md`
- `configs/registries/builtin-provider-registry.yaml`

### 먼저 실행할 명령

```powershell
powershell -ExecutionPolicy Bypass -File ./scripts/test.ps1
python scripts/audit_source_absorption.py
```

### 현재 차단 요소

- 없음.

## 현재 활성 작업

| ID | 상태 | 목표 | 주요 파일 | 다음 조치 |
|---|---|---|---|---|

## 열린 리스크

| ID | 내용 | 영향 | 다음 조치 |
|---|---|---|---|
| R-0001 | 실제 구현 언어와 패키지 매니저가 미정 | 다음 구현 단계의 build/test 전략 미확정 | 구현 착수 전 결정 |
| R-0002 | 원본 설계 폴더 삭제 보류 | 디스크에는 중복 원본이 남음 | 삭제 요청 시 흡수 감사 문서 확인 후 별도 처리 |

## Archive References

| 항목 | 위치 |
|---|---|
| 원본 파일별 흡수 위치 | `docs/decisions/source-absorption-map.md` |
| 흡수 감사 결과 | `docs/decisions/source-absorption-audit.md` |
| 설계 흡수 보강 커밋 | `c321f11` |

## 완료 작업

| ID | 완료일 | 한 줄 요약 | 근거 |
|---|---|---|---|
| P-0001 | 2026-06-28 | v3/v4 설계를 Star-Control monorepo 스캐폴드와 정본 문서로 흡수 | `7ccdce5` |
| P-0002 | 2026-06-28 | 원본 237개 파일 흡수 누락 재검토 및 provider/source 보존 보강 | `c321f11` |
| P-0003 | 2026-06-28 | `PLANS.md`와 plan-ledger 운영을 bounded snapshot 기준으로 압축 | git history |
