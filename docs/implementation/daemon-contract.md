# Daemon Queue Contract

## 목적

Daemon은 장시간 job queue, provider scheduling, cancel/resume orchestration을 담당하는 장기 surface다. M7b에서는 daemon process를 시작하지 않고, file-based queue state와 안전 precondition만 먼저 구현한다. Productization daemon app slice에서는 `apps/star-daemon serve --max-ticks`가 queued `fake-default`와 allowlisted local-process job을 실행하는 scheduler tick을 제공한다.

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

M7b queue skeleton은 caller가 넘긴 config root 아래에 daemon state를 둔다.

```text
{config_root}/daemon/state.json
```

OS별 기본 config/cache path와 logs directory는 별도 문서에서 확정한다. library는 repository root나 대상 project root에 daemon state를 자동 생성하지 않는다.

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

M7b 구현 범위:

- `packages/star-control-daemon` crate
- `DaemonConfig`
- `DaemonQueue`
- `{config_root}/daemon/state.json` 생성/검증
- StateStore job을 queue entry로 참조 등록
- terminal job queue 거부
- `WAITING_APPROVAL` job의 approved `approval-response.json` precondition
- duplicate queue entry guard

M7b에서 아직 구현하지 않는 것:

- daemon background process
- socket
- HTTP API server
- provider scheduling worker
- API mutation endpoint
- UI shell
- OS별 기본 config path 자동 선택

Productization app slice에서 실제 daemon process surface, loopback HTTP API server, `fake-default` queue scheduler tick, local-process scheduler executor는 구현한다. 아직 남은 범위는 long-running background worker, socket, remote exposure, cloud/live scheduler executor, Local/Cloud AI live connector execution이다.

## 테스트 기준

1. DaemonState example schema validation
2. daemon state가 repository root에 생성되지 않음
3. job artifact는 대상 project `.ai-runs/` 아래에 유지
4. terminal job은 queue에 재등록하지 않음
5. approval required job은 approval response 없이 실행하지 않음
6. duplicate queue entry를 등록하지 않음
