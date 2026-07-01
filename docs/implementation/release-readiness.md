# Release Readiness Reserved Pack

## 목적

이 문서는 Star-Control release/deploy/publish 자동화 전에 필요한 readiness artifact와 gate 기준을 고정한다. 현재 release automation은 RESERVED이며, 이 문서는 실제 배포를 수행하지 않는다.

## machine-readable contracts

```text
specs/schemas/release-readiness.schema.json
examples/release-contracts/release-readiness.example.json
```

위 example은 `scripts/ci/check_schema_examples.py`에서 검증한다.

## ReleaseReadiness

필수 필드:

```text
schema_version
release_id
target
version
status
checks
blockers
approvals
generated_at
```

status 후보:

```text
reserved
not_ready
ready
blocked
```

현재 repository 단계에서는 `reserved` 또는 `not_ready`만 사용한다. `ready`는 실제 release process, signing, changelog, rollback, publish policy가 구현된 뒤에만 사용한다.

M9f 구현 위치:

```text
packages/star-control-release
```

M9f는 `ReleaseReadinessWriter`를 제공한다. writer는 schema-valid ReleaseReadiness JSON을 `.ai-runs/{job_id}/release/release-readiness.json`에 한 번만 쓰고, ArtifactRef는 `kind=other`, `producer=star-control-release`, `schema_path=specs/schemas/release-readiness.schema.json`을 사용한다. 현재 slice에서는 `ready` status를 거부하고, `reserved` status에는 blocker explanation을 요구한다.

M9g 구현 위치:

```text
packages/star-control-api
```

M9g는 `ApiReadOnlyService`에 `GET /projects/{project_id}/jobs/{job_id}/release-readiness`를 추가한다. endpoint는 existing ReleaseReadiness artifact를 읽고 `api-response.schema.json` envelope으로 반환하며, missing artifact는 `release_readiness_not_found` structured error로 반환한다. 이 endpoint는 StateStore artifact를 수정하지 않고, HTTP server나 release/deploy/publish automation을 추가하지 않는다.

## readiness checks

초기 check 후보:

```text
required-ci-passed
release-profile-passed
no-block-diagnostics
no-unreviewed-human-review
changelog-updated
version-consistent
artifact-signing-ready
rollback-plan-ready
package-publishing-approved
```

각 check는 `pass`, `fail`, `warn`, `not_applicable`, `reserved` 중 하나를 사용한다.

## blockers

release를 막아야 하는 조건:

- required CI 실패
- Star Sentinel release profile 실패
- open BLOCK diagnostic 존재
- unreviewed HUMAN_REVIEW decision 존재
- version/changelog 불일치
- artifact signing 미정
- rollback policy 미정
- package publishing approval 없음

## approvals

release/deploy/publish는 외부 계정과 사용자 배포 환경을 바꿀 수 있으므로 approval required다. 자동으로 GitHub release, package registry publish, cloud deploy를 수행하지 않는다.

## 금지 사항

- release automation을 이 문서 추가 PR에 섞지 않는다.
- branch protection, repository settings, package registry 설정을 자동 변경하지 않는다.
- approval 없이 release/deploy/publish를 실행하지 않는다.
- readiness status를 증거 없이 `ready`로 표시하지 않는다.
- M9f writer는 기존 readiness artifact를 조용히 덮어쓰지 않는다.
- M9g API endpoint는 readiness artifact를 읽기만 하고 release action을 실행하지 않는다.

## 테스트 기준

1. ReleaseReadiness example schema validation
2. reserved release example은 publish/deploy를 수행하지 않음
3. `ready` 상태는 blockers가 비어 있고 approvals가 있어야 함
4. release profile failure는 blocked로 mapping 가능
5. version/changelog mismatch는 blocker로 기록
6. API read-only endpoint는 missing readiness를 structured error로 반환하고 artifact를 수정하지 않음

## Codex 구현 지시

Release 관련 구현은 다음 순서로 분리한다.

1. release readiness artifact writer
2. API read-only release readiness surface
3. changelog/version consistency checker
4. release profile validation integration
5. release review pack 생성
6. manual approval flow
7. artifact signing policy
8. publish/deploy automation

7~8은 별도 승인 전까지 구현하지 않는다.
