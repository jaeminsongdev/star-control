# Star-Control AGENTS.md

## 기본 원칙

- 기본 응답과 문서는 한국어를 우선한다.
- 설정 키, 명령어, 코드, 파일명, 로그 원문은 원문 표기를 유지한다.
- Star-Control은 provider-neutral 관제/라우팅/실행/상태관리 본체다.
- Star Sentinel은 Star-Control 기본 탑재 검증 도구이며, 코어에 직접 결합하지 않는다.

## 작업 경계

- 이 repository의 `README.md`, `docs/`, `specs/`, `configs/`, `builtin-providers/`, `builtin-tools/star-sentinel/`를 설계 기준으로 삼는다.
- ChatGPT GitHub 작업은 `docs/operations/chatgpt-github-workflow.md`를 따른다.
- Codex 구현 작업은 `docs/implementation/README.md`, `docs/decisions/0005-full-implementation-defaults.md`, `docs/implementation/codex-long-run-workflow.md`, `docs/implementation/codex-work-queue-current.md`를 먼저 따른다.
- 실제 구현 착수 순서는 `docs/implementation/codex-work-queue-current.md`를 우선한다.
- `docs/implementation/codex-work-queue.md`는 장기 backlog이며, 현재 착수 큐와 충돌하면 `codex-work-queue-current.md`를 기준으로 한다.
- 원격 저장소 push, 외부 계정 수정, 의존성 설치, 패키지 매니저 도입은 명시 승인 전까지 하지 않는다.
- `docs/decisions/0005-full-implementation-defaults.md`의 기본 Rust dependency set은 Star-Control Rust workspace 구현 PR에서 목적, version, 검증 결과를 기록하는 조건으로 사용할 수 있다. 이 범위 밖의 dependency, Cargo 외 package manager, lockfile 정책 변경은 계속 별도 승인 대상이다.
- 실행 결과는 Star-Control repo가 아니라 대상 프로젝트의 `.ai-runs/` 아래에 둔다.
- provider 구현은 제품명 package가 아니라 transport, adapter, capability 중심으로 분리한다.

## 검증 기준

- JSON schema는 파싱 가능해야 한다.
- 정식 Star Sentinel 명칭은 `Star Sentinel`, `star-sentinel`, `star_sentinel`, `star.sentinel`만 사용한다.
- 호환 alias는 `builtin-tools/star-sentinel/tool.yaml`의 `legacy_aliases`에만 둔다.
