> 흡수 출처: `star-control_design_v3/docs/28_Adapter_Conformance_Test_Suite.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 28. Adapter Conformance Test Suite

## 목적

ProviderAdapter가 늘어나면 Codex/Gemini/Claude/Local이 서로 다른 방식으로 실패한다. Star-Control은 모든 adapter가 같은 계약을 만족하는지 테스트해야 한다.

## 테스트 레벨

| 레벨 | 이름 | 설명 |
|---|---|---|
| L0 | Static Config Test | provider yaml/schema 검증 |
| L1 | Fake Provider Test | 실제 AI 없이 run 흐름 검증 |
| L2 | Dry Run Test | AI 호출 없이 명령 생성만 검증 |
| L3 | Provider Smoke Test | 실제 provider로 간단 요청 실행 |
| L4 | End-to-End Test | Stopwatch MVP 전체 실행 |

## Conformance 항목

모든 ProviderAdapter는 아래를 구현해야 한다.

```text
- prepare(input: WorkSpec) -> ProviderRunRequest
- start(request) -> ProviderRunHandle
- poll(handle) -> ProviderRunStatus
- collect(handle) -> ProviderRunResult
- normalize(result) -> ReportSpec
- cancel(handle)
```

## 필수 테스트

### 1. schema validation

- provider config가 `provider.schema.json` 통과.
- provider feature file이 capability registry에 없는 기능을 참조하지 않음.

### 2. prompt rendering

- WorkSpec이 provider prompt로 변환됨.
- forbidden_actions가 빠지지 않음.
- required_outputs가 포함됨.

### 3. result parsing

- 정상 output → DONE report.
- schema 깨진 output → MODEL_OUTPUT_INVALID.
- 빈 output → PROVIDER_EMPTY_OUTPUT.

### 4. exit code mapping

- exit 0 + valid report → DONE.
- exit 0 + invalid report → FAILED.
- exit nonzero → PROVIDER_LAUNCH_FAILED or PROVIDER_EXEC_FAILED.

### 5. timeout/cancel

- timeout 시 PROVIDER_TIMEOUT.
- cancel 호출 후 상태 CANCELED.

## Fake Provider

Fake Provider는 실제 AI 대신 정해진 fixture를 반환한다.

```yaml
provider: fake
roles:
  router-low:
    fixture: examples/fake/route-done.json
  worker-impl:
    fixture: examples/fake/impl-report-done.json
```

Fake Provider로 먼저 Run Engine을 완성해야 한다.

## Golden Test

```text
tests/golden/
  stopwatch-small/
    request.md
    expected-route.json
    expected-final-report.md
```

## 성공 기준

- [ ] Fake Provider로 MVP E2E 통과.
- [ ] Codex Adapter smoke test 통과.
- [ ] 실패 fixture 5종 분류 통과.
- [ ] schema invalid output repair flow 1회 통과.
- [ ] scope violation 감지 통과.
