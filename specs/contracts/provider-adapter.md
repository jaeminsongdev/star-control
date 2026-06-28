# Provider Adapter Contract

`ProviderAdapter`는 Star-Control `WorkSpec`을 provider 실행 요청으로 바꾸고, provider 출력을 `ProviderResult`와 `ReportSpec`으로 정규화한다.

```text
prepare(workspec) -> ProviderRunRequest
start(request) -> ProviderRunHandle
poll(handle) -> ProviderRunStatus
collect(handle) -> ProviderRunResult
normalize(result) -> ReportSpec
cancel(handle) -> CancelResult
```
