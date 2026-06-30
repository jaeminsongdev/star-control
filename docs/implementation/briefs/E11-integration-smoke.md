# E11 Integration Smoke Brief

## 목표

v0 fake flow 전체를 end-to-end smoke로 검증한다.

## 선행 문서

```text
docs/implementation/testing-ci-release.md
docs/implementation/cli-command-reference.md
docs/implementation/validation-engine.md
docs/implementation/codex-validation-report.md
```

## 수정 허용 파일

```text
integration smoke tests
examples/projects/** 필요 최소 범위
examples/runs/** 필요 최소 범위
관련 docs/report 업데이트
```

## 수정 금지 파일

```text
cloud/local provider 실제 연결
daemon/API/UI 구현
release automation
external account 변경
```

## 핵심 작업

```text
fake project 준비
star-control run 실행
J-0001 생성
route.json 생성
fake provider output 생성
Star Sentinel P0 validation 실행
report 생성
terminal state 확인
```

## 검증 명령

```text
python scripts/ci/run_all.py
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

## 완료 기준

fake provider만으로 route -> execute -> validate -> report 흐름이 통과하고 terminal state가 확인되어야 한다.

## handoff

실사용 디버깅 전 남은 known issue, manual steps, unsupported provider scope를 PR 보고에 남긴다.

## 중단 조건

cloud/local provider, daemon/API/UI, release automation을 smoke 통과 조건에 넣어야 할 것 같으면 멈춘다.
