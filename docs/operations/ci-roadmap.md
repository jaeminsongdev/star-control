# CI 운영 로드맵

## 목적

Star-Control의 CI는 AI가 만든 변경사항을 사람이 최종 검토하기 전에 자동으로 확인하는 검증 게이트다. 초기 CI는 빌드보다 문서, 스키마, manifest, 명칭 정책을 우선한다.

## 현재 1차 CI 범위

- 저장소 필수 구조 확인
- JSON / YAML / TOML 파싱 가능 여부 확인
- Star Sentinel manifest 최소 계약 확인
- Star Sentinel 명칭과 legacy alias 사용 위치 확인

## 1차 CI에 포함하지 않는 항목

아직 루트에 `Cargo.toml`, `package.json`, `pyproject.toml`이 없으므로 다음 검사는 보류한다.

- Rust fmt / check / clippy / test
- Node 또는 TypeScript lint / typecheck / test
- Python ruff / pytest
- Docker build
- release / publish / deploy

## 다음 단계 CI

### 2단계: 구현 패키지 생성 후

`packages/` 아래 실제 구현이 들어오면 언어별 검증을 추가한다.

Rust 패키지가 생기면 다음 job을 추가한다.

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

TypeScript 패키지가 생기면 다음 job을 추가한다.

```bash
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
```

Python 패키지가 생기면 다음 job을 추가한다.

```bash
python -m compileall .
ruff check .
pytest
```

### 3단계: PR 보호 설정

CI가 안정적으로 통과하는 것을 확인한 뒤 `main` branch에 다음 보호 설정을 건다.

- PR 없이 merge 금지
- 필수 status check 통과 전 merge 금지
- conversation resolution 전 merge 금지
- force push 금지
- branch 삭제 금지

필수 status check 후보는 다음과 같다.

- `repository-policy-check`
- `data-format-check`
- `manifest-contract-check`
- `naming-policy-check`

### 4단계: 정책 검사 강화

초기 CI는 경량 검사만 둔다. 추후 다음 검사를 별도 PR로 추가한다.

- 실행 산출물 위치 검사
- 민감 파일명 검사
- 외부 검사 도구 연동
- workflow 변경 위험도 검사

### 5단계: Star Sentinel selfcheck 연동

`packages/star-sentinel/` 구현이 생기면 다음 명령을 CI에 추가한다.

```bash
star-sentinel selfcheck --profile quick
star-sentinel check --profile quick
```

이 단계부터 Star Sentinel은 CI 결과를 읽어 review pack과 approval gate를 생성하는 방향으로 확장한다.

## 운영 원칙

- CI workflow는 기본적으로 `contents: read` 권한만 가진다.
- 초기 CI에서는 외부 API 호출이나 배포 작업을 하지 않는다.
- `pull_request_target`은 사용하지 않는다.
- release, publish, deploy는 별도 승인 전까지 CI에 넣지 않는다.
- Codex 또는 다른 AI가 CI를 수정하는 PR은 고위험 변경으로 본다.
- 테스트 실패를 테스트 삭제나 완화로 해결하는 변경은 허용하지 않는다.
