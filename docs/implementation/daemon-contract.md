# Daemon Reserved Contract

## 목적

Daemon은 장시간 job queue, provider scheduling, cancel/resume orchestration을 담당하는 장기 surface다. 초기 구현 대상은 아니며, CLI file-based flow가 안정화된 뒤 구현한다.

## machine-readable contracts

```text
specs/schemas/daemon-state.schema.json
examples/surface-contracts/daemon-state.example.json
```

위 example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## 책임

Daemon이 담당할 수 있는 것:

- job queue 관리
- provider execution scheduling
- cancellation propagation
- resume orchestration
- status watch
- local API server host 후보

Daemon이 담당하지 않는 것:

- Star Sentinel rule 직접 구현
- provider-specific adapter logic 직접 구현
- secret raw value 저장
- repository 내부에 daemon runtime state 저장
- user approval 없이 release/deploy 실행

## state 위치

Daemon runtime state는 Star-Control repository가 아니라 user machine의 config/cache 영역을 사용한다. Job artifact는 여전히 대상 project의 `.ai-runs/` 아래에 둔다.

후보:

```text
~/.star-control/daemon/state.json
~/.star-control/daemon/logs/
```

OS별 config/cache path는 별도 문서에서 확정한다.

## DaemonState

필수 필드:

```text
schema_version
daemon_id
status
queue
active_jobs
```

status 후보:

```text
reserved
starting
running
stopping
stopped
error
```

## 구현 전제

초기에는 `reserved` status example만 유지한다. 실제 daemon process, socket, API server, background worker는 별도 승인과 별도 PR이 필요하다.

## 테스트 기준

1. DaemonState example schema validation
2. daemon state가 repository root에 생성되지 않음
3. job artifact는 대상 project `.ai-runs/` 아래에 유지
4. terminal job은 queue에 재등록하지 않음
5. approval required job은 approval response 없이 실행하지 않음
