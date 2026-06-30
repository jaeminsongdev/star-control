# CI Contract Validation

## 목적

이 문서는 Star-Control의 현재 CI가 어떤 계약을 막는지 설명한다. 현재 repository는 구현 전 계약 고정 단계이므로, CI는 코드 빌드보다 문서, schema, example, manifest, naming policy, current work queue 정합성을 우선 검증한다.

## local runner

Codex와 구현자는 로컬에서 아래 명령을 우선 실행한다.

```text
python scripts/ci/run_all.py
```

`run_all.py`는 현재 contract validator를 GitHub Actions와 같은 의도 순서로 실행한다. 개별 실패를 디버깅할 때만 아래 job별 명령을 따로 실행한다.

## CI jobs

```text
repository-policy-check
data-format-check
manifest-contract-check
naming-policy-check
schema-example-check
implementation-documentation-check
work-queue-consistency-check
```

## repository-policy-check

명령:

```text
python scripts/ci/check_repo_policy.py
```

역할:

- 필수 root path 존재 확인
- Star-Control repository 내부에 실행 artifact가 저장되지 않도록 확인
- `.ai-runs/`가 repository root에 생기지 않도록 확인

## data-format-check

명령:

```text
python scripts/ci/check_data_formats.py
```

역할:

- JSON/YAML/TOML parse 확인
- configs, specs, examples, builtin provider/tool metadata의 format 오류를 조기에 차단

## manifest-contract-check

명령:

```text
python scripts/ci/check_manifest_contracts.py
```

역할:

- Star Sentinel builtin tool manifest 필수 필드 확인
- command/profile/output 계약 확인
- legacy alias 위치 제한 확인

## naming-policy-check

명령:

```text
python scripts/ci/check_star_sentinel_naming.py
```

역할:

- 공식 명칭 `Star Sentinel` 기준 확인
- legacy alias 사용 위치 제한 확인

## schema-example-check

명령:

```text
python scripts/ci/check_schema_examples.py
```

역할:

- canonical JSON examples가 schema subset을 만족하는지 확인
- core/provider/config/router/execution/validation/CLI/surface/security/Star Sentinel examples 검증
- 새 schema를 추가하면 최소 하나 이상의 canonical example과 validation case를 함께 추가해야 함

## implementation-documentation-check

명령:

```text
python scripts/ci/check_implementation_docs.py
```

역할:

- 구현자가 반드시 읽어야 하는 implementation docs와 decisions 존재 확인
- canonical example directory 존재 확인
- CI workflow가 핵심 validator를 실제로 호출하는지 확인
- local runner가 핵심 validator를 모두 호출하는지 확인

## work-queue-consistency-check

명령:

```text
python scripts/ci/check_work_queue_consistency.py
```

역할:

- 현재 구현 착수 큐의 우선권 문구 확인
- E01~E11 EPIC heading 확인
- EPIC별 handoff section marker 확인
- E08/E09 split guidance 확인
- RESERVED section 유지 확인

## 새 계약 추가 규칙

새 schema/example/doc를 추가할 때 기본 절차:

1. schema 추가
2. canonical example 추가
3. `scripts/ci/check_schema_examples.py` validation case 추가
4. 구현 문서에 machine-readable contracts section 추가
5. 필요하면 `scripts/ci/check_implementation_docs.py` required docs 또는 example dirs에 추가
6. work queue 흐름을 바꾸면 `scripts/ci/check_work_queue_consistency.py` 영향도 검토
7. 새 validator를 추가하면 `scripts/ci/run_all.py`와 GitHub Actions wiring을 함께 검토

## 금지 사항

- 실패하는 schema/example을 CI에서 제외해서 통과시키지 않는다.
- policy를 약화해서 validator를 통과시키지 않는다.
- Star Sentinel legacy alias를 새 문서나 schema에 확산하지 않는다.
- `.ai-runs/` 실행 산출물을 repository에 커밋하지 않는다.

## Codex 구현 지시

CI validator를 수정하는 PR은 다음을 포함한다.

- 수정 이유
- 추가/변경된 validation target
- 실패 시 예상되는 error message
- 기존 validation 약화 여부 검토

validation을 약화하는 변경은 별도 approval required로 본다.
