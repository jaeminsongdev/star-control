# PLANS.md

## 목적

현재 작업 상태를 짧게 유지하는 원장이다. 상세 로그, 전체 diff, 반복 검증 출력은 여기에 누적하지 않는다. 장기 보존이 필요한 근거는 `docs/decisions/*`, report, changelog, commit history에 둔다.

## Context Pack

### 현재 목표

- Star-Control repository는 현재 스캐폴드와 정본 설계 문서 상태다.
- v0 runtime 구현 스택은 Rust + Cargo workspace로 결정했다.
- v0 fake provider instance id는 `fake-default`로 결정했다.
- v0 Star Sentinel P0 rule set은 5개 핵심 rule로 결정했다.
- Codex 구현 착수용 EPIC brief는 `docs/implementation/briefs/`를 기준으로 한다.
- `PLANS.md`는 현재 상태만 남기는 bounded snapshot으로 유지한다.

### 반드시 지켜야 할 제약

- 의존성 추가, Cargo 외 package manager 도입, 원격 공개 작업은 명시 요청이 있을 때만 한다.
- 실행 결과는 Star-Control repo가 아니라 대상 프로젝트 `.ai-runs/`에 둔다.
- 외부 보조 자료를 다시 붙이지 않고 이 repository 안의 정본 파일을 기준으로 작업한다.

### 이미 끝난 것

- Star-Control monorepo 스캐폴드, schema, contract, config, registry, provider/tool manifest를 정리했다.
- Star Sentinel 명칭, policy, schema, template, output contract를 정리했다.
- `PLANS.md`와 plan-ledger 운영 기준을 bounded snapshot 방식으로 정리했다.
- v0 runtime stack을 Rust + Cargo workspace로 결정했다.
- v0 fake provider instance id를 `fake-default`로 통일했다.
- Star Sentinel v0 P0 scope와 E09a~E09d 구현 분할을 정리했다.
- 로컬 contract check entrypoint를 `python scripts/ci/run_all.py`로 추가했다.
- E01~E11 Codex 구현 착수용 brief를 추가했다.

### 아직 남은 것

- Cargo workspace와 최소 runtime crate 생성은 구현 PR에서 시작한다.
- provider host, transport, adapter, Star Sentinel runtime 구현.
- Codex가 E01부터 순차 구현을 시작한다.

### 건드리면 안 되는 것

- 사용자 승인 없는 의존성 설치, 파일 삭제, 테스트 약화.
- schema, manifest, registry의 공개 필드명은 변경 전 영향 범위를 확인한다.
- fake flow 안정화 전 local/cloud provider, daemon, API, UI, release automation을 앞당기지 않는다.

### 먼저 확인할 파일

- `README.md`
- `docs/implementation/README.md`
- `docs/implementation/codex-long-run-workflow.md`
- `docs/implementation/codex-work-queue-current.md`
- `docs/implementation/briefs/README.md`
- 해당 EPIC의 `docs/implementation/briefs/E*.md`

### 먼저 실행할 명령

```text
python scripts/ci/run_all.py
```

### 현재 차단 요소

- 없음.

## 현재 활성 작업

| ID | 상태 | 목표 | 주요 파일 | 다음 조치 |
|---|---|---|---|---|

## 열린 리스크

| ID | 내용 | 영향 | 다음 조치 |
|---|---|---|---|
| R-0001 | runtime 구현 전 상태 | CLI/API/UI 동작 검증은 아직 불가 | 구현 스택 결정에 따라 E01부터 순차 구현 |
| R-0002 | Cargo workspace 파일 미생성 | Cargo build/test 명령은 구현 PR 전까지 제한됨 | E01 구현 PR에서 최소 workspace 생성 |
| R-0003 | 로컬 CI 미실행 상태 | 현재 세션에서 full local validation evidence 없음 | Codex 또는 로컬 checkout에서 `python scripts/ci/run_all.py` 실행 |

## Archive References

| 항목 | 위치 |
|---|---|
| 정본 구조 결정 | `docs/decisions/0001-canonical-repository.md` |
| runtime stack 결정 | `docs/decisions/0002-runtime-stack.md` |
| fake provider instance 결정 | `docs/decisions/0003-fake-provider-instance.md` |
| Star Sentinel P0 scope 결정 | `docs/decisions/0004-star-sentinel-p0-scope.md` |
| EPIC별 brief | `docs/implementation/briefs/` |
| 이전 완료 이력 | git history |

## 완료 작업

| ID | 완료일 | 한 줄 요약 | 근거 |
|---|---|---|---|
| P-0001 | 2026-06-28 | Star-Control monorepo 스캐폴드와 정본 설계 문서 생성 | `7ccdce5` |
| P-0002 | 2026-06-28 | provider, schema, Star Sentinel 설계 보강 | `c321f11` |
| P-0003 | 2026-06-28 | `PLANS.md`와 plan-ledger 운영을 bounded snapshot 기준으로 압축 | git history |
| P-0004 | 2026-07-01 | v0 runtime stack을 Rust + Cargo workspace로 결정 | `docs/decisions/0002-runtime-stack.md` |
| P-0005 | 2026-07-01 | v0 fake provider instance id를 `fake-default`로 통일 | `docs/decisions/0003-fake-provider-instance.md` |
| P-0006 | 2026-07-01 | Star Sentinel v0 P0 scope와 E09 분할 기준 정리 | `docs/decisions/0004-star-sentinel-p0-scope.md` |
| P-0007 | 2026-07-01 | 로컬 contract check runner 추가 | `scripts/ci/run_all.py` |
| P-0008 | 2026-07-01 | E01~E11 Codex 구현 착수용 brief 추가 | `docs/implementation/briefs/` |
