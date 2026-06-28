# Policy Profiles 구현 계약

## 목적

Policy profile은 Star Sentinel과 ValidationEngine이 어떤 강도의 검증을 수행할지 정하는 named profile이다. profile은 작업 위험도, stage, release 여부, validator 자체 변경 여부에 따라 선택된다.

## profile 목록

현재 manifest와 policy 기준 profile:

```text
quick
near
full
security
release
validator
```

## 기본 원칙

- profile은 검증 강도를 낮추기 위한 우회 수단이 아니다.
- risk가 높을수록 더 강한 profile을 선택한다.
- validator/policy/schema/CI 관련 변경은 `validator` profile 후보로 본다.
- release/deploy 관련 작업은 `release` profile 후보로 본다.
- secret, credential, permission 관련 작업은 `security` profile 후보로 본다.

## quick

목적:

- 빠른 P0 검증
- 작은 문서/schema/example 변경
- fake provider smoke

필수 rule 후보:

```text
task.scope.allowed_paths
test.no_deletion
test.no_skip_only_ignore
dependency.requires_approval
secret.no_plaintext_secret
```

사용 기준:

- LOW risk
- SMALL size
- runtime code가 없거나 제한적
- release/deploy 아님

## near

목적:

- quick보다 조금 더 넓은 evidence/report 검증
- 일반 구현 PR 검증

추가 rule 후보:

```text
claim.validation_evidence_required
report.changed_files_match_diff
```

사용 기준:

- MEDIUM risk
- runtime code 변경
- report와 changed files 일치성이 중요함

## full

목적:

- 큰 변경에 대한 전체 검증
- 여러 package 연동
- integration smoke 포함

추가 후보:

```text
full.repo_map_consistency
full.provider_output_consistency
full.workspec_scope_consistency
full.report_artifact_consistency
```

사용 기준:

- LARGE size
- 여러 package 변경
- provider/router/execution/validation 연동

## security

목적:

- 보안 민감 변경 검증
- secret, credential, permission, workflow 관련 검증

추가 후보:

```text
security.secret_candidate_scan
security.workflow_permission_review
security.credential_reference_only
security.dangerous_command_review
```

사용 기준:

- credential 관련 변경
- workflow permission 변경
- auth/config/security path 변경
- secret exposure 가능성

## release

목적:

- release/deploy 전 검증
- versioning, changelog, artifact, CI 상태 확인

추가 후보:

```text
release.ci_required_checks_passed
release.changelog_present
release.version_consistency
release.no_unreviewed_risk
```

사용 기준:

- release 준비
- package publish
- deploy
- tag 생성

현재 repository 단계에서는 release profile은 RESERVED이며 실제 release automation을 추가하지 않는다.

## validator

목적:

- Star Sentinel, schema, policy, CI validation 자체 변경 검증
- validator self-change가 자동 통과되지 않도록 확인

필수 rule 후보:

```text
validator.no_self_bypass
validator.policy_change_requires_approval
validator.schema_example_consistency
validator.fixture_policy_consistency
validator.naming_policy_consistency
```

사용 기준:

- `scripts/ci/` 변경
- `builtin-tools/star-sentinel/policies/` 변경
- `builtin-tools/star-sentinel/schemas/` 변경
- `builtin-tools/star-sentinel/fixtures/` 변경
- `builtin-tools/star-sentinel/examples/` 변경
- naming policy 변경

## profile 선택 규칙

RouterEngine과 ValidationEngine은 다음 순서를 참고한다.

```text
validator-sensitive change -> validator
release/deploy change -> release
security-sensitive change -> security
LARGE or CRITICAL -> full
MEDIUM -> near
LOW/SMALL -> quick
```

여러 profile 후보가 있으면 더 엄격한 profile을 선택한다.

엄격도 후보:

```text
quick < near < full < security < release < validator
```

단, `security`, `release`, `validator`는 단순 선형 상하관계가 아니라 목적별 special profile이다. 해당 조건이 있으면 우선 선택한다.

## approval과 profile

다음 profile은 기본적으로 approval required 후보를 만든다.

```text
security
release
validator
```

`full`도 high-risk change가 있으면 approval required로 전환할 수 있다.

## profile output

ValidationRun에는 선택된 profile을 기록한다.

```text
profile: quick
```

ReviewPack에도 사용한 profile과 주요 rule 결과를 포함할 수 있다.

## profile 변경 규칙

profile 정의를 바꾸는 PR은 다음을 포함해야 한다.

- policy file 변경
- fixture 또는 example 변경
- schema-example-check 또는 selfcheck 영향 검토
- approval/review note

profile을 약화하는 변경은 자동 merge하면 안 된다.

## fixture 전략

각 profile은 최소 하나 이상의 대표 fixture를 가져야 한다.

후보:

```text
fixtures/p0/scope-violation.case.yaml
fixtures/p0/dependency-approval.case.yaml
fixtures/security/secret-exposure.case.yaml
fixtures/validator/policy-change.case.yaml
```

초기에는 P0 fixture부터 유지한다.

## 테스트 기준

최소 테스트:

1. LOW/SMALL -> quick
2. runtime code change -> near
3. multi-package change -> full
4. secret candidate -> security
5. release request -> release
6. policy/schema/CI validation change -> validator
7. validator profile은 approval required 후보
8. unknown profile -> error

## Codex 구현 지시

Policy profile 구현은 Star Sentinel P0 rule evaluator 이후 진행한다. profile selection은 RouterEngine 또는 ValidationEngine에서 시작할 수 있지만, rule execution은 Star Sentinel에 둔다.
