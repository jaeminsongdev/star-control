# Schema Validator 구현 계약

## 목적

Schema Validator는 Star-Control과 Star Sentinel artifact가 정해진 JSON schema subset을 만족하는지 검증하는 공통 계층이다. 초기 구현은 외부 `jsonschema` runtime dependency 없이 repository 내부 validator로 시작한다.

## 기본 원칙

- 외부 `jsonschema` package 도입은 명시 승인 전까지 금지한다.
- 현재 CI의 `scripts/ci/check_schema_examples.py`가 지원하는 subset을 기준으로 시작한다.
- runtime validator와 CI validator의 동작 차이를 최소화한다.
- validator는 검증 실패를 숨기거나 warning으로 낮추면 안 된다.

## 지원할 JSON Schema subset

초기 validator는 아래 keyword를 지원한다.

```text
const
enum
type
required
properties
items
additionalProperties
minLength
pattern
```

`type`은 string 또는 string array를 지원한다.

지원 type:

```text
null
boolean
object
array
string
number
integer
```

## 초기에는 지원하지 않는 keyword

```text
$ref
oneOf
anyOf
allOf
not
if/then/else
format
minimum
maximum
minItems
maxItems
uniqueItems
dependentRequired
patternProperties
```

지원하지 않는 keyword를 schema에서 발견했을 때 초기 구현은 error로 처리하지 않아도 된다. 다만 validator 문서와 테스트에서 무시되는 keyword임을 명확히 해야 한다.

## validation 대상

Core-level artifact:

```text
specs/schemas/job.schema.json
specs/schemas/run-state.schema.json
specs/schemas/route.schema.json
specs/schemas/workspec.schema.json
specs/schemas/report.schema.json
```

Star Sentinel artifact:

```text
builtin-tools/star-sentinel/schemas/sentinel-task.schema.json
builtin-tools/star-sentinel/schemas/diagnostic.schema.json
builtin-tools/star-sentinel/schemas/approval.schema.json
builtin-tools/star-sentinel/schemas/review-pack.schema.json
builtin-tools/star-sentinel/schemas/validation-run.schema.json
builtin-tools/star-sentinel/schemas/ledger-event.schema.json
builtin-tools/star-sentinel/schemas/repo-map.schema.json
builtin-tools/star-sentinel/schemas/changed-lines.schema.json
```

## 권장 API

```text
load_schema(schema_path) -> Schema
validate_json(document, schema) -> ValidationResult
validate_file(document_path, schema_path) -> ValidationResult
assert_valid(document, schema) -> None
```

`ValidationResult` 후보:

```text
ok: boolean
errors: list[ValidationError]
```

`ValidationError` 후보:

```text
location: string
message: string
expected: optional string
actual: optional string
schema_path: optional string
document_path: optional string
```

## location 표기

location은 사람이 이해하기 쉬운 dotted path를 사용한다.

예시:

```text
$.schema_version
$.assignments.implement.provider
$.files[0].hunks[0].lines[1].kind
```

CI validator가 현재 쓰는 단순 location 형식과 달라도 되지만, 오류 메시지는 문서 경로와 필드 위치를 함께 보여야 한다.

## type matching 규칙

- `integer`는 bool을 integer로 취급하지 않는다.
- `number`도 bool을 number로 취급하지 않는다.
- `null`은 JSON null만 허용한다.
- `object`는 JSON object만 허용한다.
- `array`는 JSON array만 허용한다.

## object validation

- `required` 필드가 없으면 error.
- `properties`에 정의된 필드가 존재하면 child schema를 검증.
- `additionalProperties`가 object인 경우 추가 key도 해당 schema로 검증.
- `additionalProperties: false`는 초기 subset에서 MAY로 둔다. 지원 시 unknown field를 error로 처리한다.

## array validation

- `items`가 object이면 모든 element를 해당 schema로 검증한다.
- tuple validation은 지원하지 않는다.

## string validation

- `minLength`가 있으면 문자열 길이를 검증한다.
- `pattern`은 전체 match를 기본으로 한다.
- regex engine 차이가 생길 수 있으므로 복잡한 pattern 사용은 피한다.

## schema loading

- schema file은 UTF-8 JSON이어야 한다.
- YAML schema는 초기 구현 대상이 아니다.
- schema root는 object여야 한다.
- schema file parse 실패는 validation failure가 아니라 schema loading failure다.

## document loading

- JSON document는 UTF-8이어야 한다.
- JSON parse 실패는 `InvalidJson` 오류로 반환한다.
- YAML document validation은 초기 구현 대상이 아니다. YAML은 data-format-check에서 파싱만 확인한다.

## CI validator와 runtime validator 관계

현재 CI는 `scripts/ci/check_schema_examples.py`로 example과 schema를 검증한다. runtime validator 구현 이후에도 CI script를 바로 제거하지 않는다.

권장 단계:

1. runtime validator 구현
2. runtime validator unit test 추가
3. CI script와 runtime validator 결과 비교
4. 안정화 후 CI script가 runtime validator를 호출하도록 전환 검토

## 금지 사항

- schema 검증 실패를 성공으로 위장하지 않는다.
- CI 실패를 해결하기 위해 schema-example-check case를 삭제하지 않는다.
- example을 schema에 맞추기 위해 의미 없는 빈 필드만 추가하지 않는다.
- 외부 dependency를 승인 없이 추가하지 않는다.

## 테스트 기준

최소 테스트:

1. `const` 성공/실패
2. `enum` 성공/실패
3. primitive `type` 성공/실패
4. union `type` 성공/실패
5. required 누락 감지
6. nested properties 검증
7. array items 검증
8. additionalProperties schema 검증
9. minLength 검증
10. pattern 검증
11. bool을 integer/number로 취급하지 않음
12. invalid schema root 감지
13. invalid JSON document 감지

## Codex 구현 지시

Schema Validator 구현 PR은 다음 파일군만 수정해야 한다.

```text
packages/star-control-schema/ 또는 선택된 schema package
관련 unit tests
필요한 docs 업데이트
```

StateStore, RouterEngine, ProviderAdapter 구현을 같은 PR에 섞지 않는다.
