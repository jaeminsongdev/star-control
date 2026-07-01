# Artifact Naming

## 목적

이 문서는 Star-Control run artifact의 파일명, directory name, attempt name, tmp name 규칙을 정의한다. 구현자는 같은 의미의 artifact가 여러 이름으로 생기지 않도록 이 문서를 따른다.

## 기본 원칙

- job directory 기준 상대 경로를 기본으로 한다.
- 사람이 읽는 문서와 machine-readable JSON을 구분한다.
- JSON artifact는 snake_case보다 기존 계약 파일명을 우선한다.
- stage 이름은 canonical stage enum을 사용한다.
- provider instance id는 directory name으로 사용할 수 있어야 한다.
- path traversal 가능성이 있는 이름은 거부한다.

## job id

형식:

```text
J-0001
```

규칙:

- `J-` prefix
- 4자리 이상 zero padding
- `.ai-runs/` 아래 directory name과 `job.json.job_id`가 일치

## stage name

허용 stage:

```text
route
plan
design
implement
validate
review
polish
report
```

Stage-specific artifact는 stage name을 파일명에 포함한다.

예시:

```text
workspecs/implement.json
reports/implement-report.json
```

## core artifact names

| 의미 | 파일명 |
|---|---|
| JobSpec | `job.json` |
| RunState | `run-state.json` |
| CoreEvent log | `events.jsonl` |
| RouteSpec | `route.json` |
| WorkSpec | `workspecs/{stage}.json` |
| Stage report | `reports/{stage}-report.json` |
| Final report | `reports/final-report.json` |

## provider output names

기본 layout:

```text
provider-output/{provider-instance-id}/request.json
provider-output/{provider-instance-id}/response.json
provider-output/{provider-instance-id}/stdout.txt
provider-output/{provider-instance-id}/stderr.txt
provider-output/{provider-instance-id}/logs/
provider-output/{provider-instance-id}/artifacts/
```

Provider instance id는 다음을 권장한다.

```text
fake-default
local-ollama-default
codex-cli-personal
```

금지 후보:

```text
../other
C:\temp
/provider
provider id with nul byte
```

## attempt names

장기적으로 같은 stage/provider 재실행을 지원하면 아래 형식을 사용한다.

```text
attempt-0001
attempt-0002
```

초기 구현에서는 attempt directory를 만들지 않아도 된다. 재실행이 필요하면 기존 output을 덮어쓰지 말고 별도 정책 PR에서 attempt layout을 도입한다.

## Star Sentinel names

Tool id와 package/entrypoint/artifact 표기를 구분한다.

| 목적 | 표기 |
|---|---|
| tool id | `star.sentinel` |
| package 후보 | `star-sentinel` |
| python entrypoint 후보 | `star_sentinel.main` |
| output directory | `tool-output/star-sentinel/` |
| CLI command | `review-pack` |
| JSON artifact | `review_pack.json` |
| Markdown artifact | `review_pack.md` |

Star Sentinel 공식 명칭은 다음만 사용한다.

```text
Star Sentinel
star-sentinel
star_sentinel
star.sentinel
```

## approval names

```text
approvals/approval-request.json
approvals/approval-response.json
tool-output/star-sentinel/approval.json
```

`approval-request.json`과 `approval-response.json`은 human/control-plane artifact다. `tool-output/star-sentinel/approval.json`은 Star Sentinel gate decision이다.

## review pack names

```text
tool-output/star-sentinel/review_pack.json
tool-output/star-sentinel/review_pack.md
review-packs/review_pack.json
review-packs/review_pack.md
```

`tool-output/star-sentinel/`는 tool original output이다. `review-packs/`는 user-facing copy 후보이다.

## tmp names

형식:

```text
tmp/{target-name}.tmp-{pid}-{nonce}
```

예시:

```text
tmp/run-state.json.tmp-1234-a1b2
tmp/route.json.tmp-1234-b2c3
```

규칙:

- tmp file은 job directory 내부 `tmp/` 아래에 둔다.
- tmp file은 ArtifactRef로 등록하지 않는다.
- tmp file은 정상 artifact로 읽지 않는다.
- recovery command가 생기기 전까지 tmp file을 자동 승격하지 않는다.

## path validation

모든 artifact path는 다음을 만족해야 한다.

- relative path
- NUL byte 없음
- drive prefix 없음
- `..` segment 없음
- `.git` 내부 직접 접근 없음
- resolved path가 job directory 내부

## 후속 정리 대상

- provider instance id schema
- attempt layout 도입 조건
- report artifact registry format
- cleanup/retention command naming
