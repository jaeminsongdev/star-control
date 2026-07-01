# API Read-Only Contract

## 목적

API는 UI와 외부 도구가 Star-Control state를 읽고 제한된 mutation을 수행하게 하는 장기 surface다. M7c에서는 HTTP server를 열지 않고, `packages/star-control-api`의 read-only request/router library로 StateStore artifact 조회와 response envelope을 먼저 고정한다.

## machine-readable contracts

```text
specs/schemas/api-response.schema.json
examples/surface-contracts/api-job-response.example.json
```

위 example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## 공통 응답 규칙

- JSON만 반환한다.
- response envelope은 `api-response.schema.json`을 따른다.
- error는 structured error object로 반환한다.
- secret raw value를 반환하지 않는다.
- artifact path는 project-relative path를 기본으로 한다.
- remote exposure는 별도 security 문서와 명시 승인 이후 구현한다.

## read-only endpoint 후보

```text
GET /daemon/state
GET /projects
GET /projects/{project_id}/jobs
GET /projects/{project_id}/jobs/{job_id}
GET /projects/{project_id}/jobs/{job_id}/events
GET /projects/{project_id}/jobs/{job_id}/report?stage={stage}
```

M7c 구현은 위 endpoint를 in-process `ApiReadOnlyService` path dispatch로 제공한다. HTTP server, socket listener, remote exposure는 아직 구현하지 않는다.

## mutation endpoint 후보

```text
POST /projects/{project_id}/jobs
POST /projects/{project_id}/jobs/{job_id}/approve
POST /projects/{project_id}/jobs/{job_id}/cancel
POST /projects/{project_id}/jobs/{job_id}/resume
```

M7d 구현은 HTTP server 없이 `packages/star-control-api`의 in-process `ApiControlService`로 mutation path dispatch를 제공한다. HTTP server, socket listener, auth/session, remote exposure는 아직 구현하지 않는다.

## API response envelope

필수 필드:

```text
schema_version
status
data
```

선택 필드:

```text
error
warnings
```

status 후보:

```text
success
failed
blocked
waiting_approval
```

## 금지 사항

- remote API를 기본으로 열지 않는다.
- 인증/권한 없이 mutation endpoint를 만들지 않는다.
- UI 편의를 위해 StateStore schema를 우회하지 않는다.
- API가 provider process를 직접 실행하지 않는다.
- API가 Star Sentinel rule을 직접 구현하지 않는다.
- API read-only endpoint가 StateStore artifact를 수정하지 않는다.
- API response에 secret-like raw value를 그대로 포함하지 않는다.

## M7c 구현 범위

구현함:

- `packages/star-control-api` crate
- `ApiReadOnlyService`
- `GET /daemon/state`
- in-memory project registry
- read-only GET path dispatch
- `api-response.schema.json` validation
- missing project/job/report structured error
- mutation method rejection
- secret-like string/key redaction
- M8a `UiReadOnlyShell` consumer 지원

아직 구현하지 않음:

- HTTP server
- socket listener
- remote exposure
- auth/session
- daemon background worker와의 live scheduling integration
- browser UI shell

## M7d 구현 범위

구현함:

```text
ApiControlService
POST /projects/{project_id}/jobs/{job_id}/approve
POST /projects/{project_id}/jobs/{job_id}/cancel
POST /projects/{project_id}/jobs/{job_id}/resume
approval-response artifact writer
run-state update
events.jsonl audit event append
structured mutation error envelope
```

아직 구현하지 않음:

```text
HTTP server
socket listener
remote exposure
auth/session
daemon background worker integration
provider execution scheduling
```

## 테스트 기준

1. API response example schema validation
2. read-only endpoint는 daemon state와 StateStore artifact를 직접 변형하지 않음
3. mutation endpoint는 approval/cancel/resume 계약을 따름
4. secret raw value가 response에 포함되지 않음
5. missing artifact는 structured error로 반환
6. mutation method는 read-only API에서 거부
7. control mutation은 approval/cancel/resume precondition을 지킴
