# PLANS.md

## 목적

현재 작업 상태를 짧게 유지하는 원장이다. 상세 로그, 전체 diff, 반복 검증 출력은 여기에 누적하지 않는다. 장기 보존이 필요한 근거는 `docs/decisions/*`, report, changelog, commit history에 둔다.

## Context Pack

### 현재 목표

- Star-Control repository는 현재 스캐폴드와 정본 설계 문서 상태다.
- `PLANS.md`는 현재 상태만 남기는 bounded snapshot으로 유지한다.

### 반드시 지켜야 할 제약

- 의존성 추가, 패키지 매니저 도입, 원격 공개 작업은 명시 요청이 있을 때만 한다.
- 실행 결과는 Star-Control repo가 아니라 대상 프로젝트 `.ai-runs/`에 둔다.
- 외부 보조 자료를 다시 붙이지 않고 이 repository 안의 정본 파일을 기준으로 작업한다.

### 이미 끝난 것

- Star-Control monorepo 스캐폴드, schema, contract, config, registry, provider/tool manifest를 정리했다.
- Star Sentinel 명칭, policy, schema, template, output contract를 정리했다.
- `PLANS.md`와 plan-ledger 운영 기준을 bounded snapshot 방식으로 정리했다.

### 아직 남은 것

- 실제 구현 언어와 패키지 매니저 결정.
- provider host, transport, adapter, Star Sentinel runtime 구현.

### 건드리면 안 되는 것

- 사용자 승인 없는 의존성 설치, 파일 삭제, 테스트 약화.
- schema, manifest, registry의 공개 필드명은 변경 전 영향 범위를 확인한다.

### 먼저 확인할 파일

- `README.md`
- `docs/00_개요.md`
- `docs/01_아키텍처.md`
- `docs/02_구현로드맵.md`
- `configs/registries/builtin-provider-registry.yaml`
- `builtin-tools/star-sentinel/tool.yaml`

### 먼저 실행할 명령

```powershell
powershell -ExecutionPolicy Bypass -File ./scripts/test.ps1
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
| R-0002 | runtime 구현 전 상태 | CLI/API/UI 동작 검증은 아직 불가 | 구현 스택 결정 후 MVP 범위 확정 |

## Archive References

| 항목 | 위치 |
|---|---|
| 정본 구조 결정 | `docs/decisions/0001-canonical-repository.md` |
| 이전 완료 이력 | git history |

## 완료 작업

| ID | 완료일 | 한 줄 요약 | 근거 |
|---|---|---|---|
| P-0001 | 2026-06-28 | Star-Control monorepo 스캐폴드와 정본 설계 문서 생성 | `7ccdce5` |
| P-0002 | 2026-06-28 | provider, schema, Star Sentinel 설계 보강 | `c321f11` |
| P-0003 | 2026-06-28 | `PLANS.md`와 plan-ledger 운영을 bounded snapshot 기준으로 압축 | git history |
