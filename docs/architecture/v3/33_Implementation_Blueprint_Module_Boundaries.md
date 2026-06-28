> 흡수 출처: `star-control_design_v3/docs/33_Implementation_Blueprint_Module_Boundaries.md`
> 정리 상태: v3 상세 설계를 Star-Control 정규 문서 트리로 흡수.

# 33. Implementation Blueprint와 Module Boundaries

## 목적

이 문서는 실제 구현자가 Star-Control의 첫 코드 구조를 어떻게 잡아야 하는지 설명한다. 언어는 아직 확정하지 않아도 되지만, 모듈 경계는 먼저 확정해야 한다.

## 권장 모듈

```text
star_control/
  cli/
  config/
  schemas/
  state/
  router/
  policy/
  provider/
  renderer/
  hooks/
  quality/
  context/
  vcs/
  observability/
```

## 모듈 책임

| 모듈 | 책임 |
|---|---|
| `cli` | 명령 파싱, 사용자 입출력 |
| `config` | 설정 계층 로딩/병합 |
| `schemas` | JSON/YAML schema 검증 |
| `state` | run-state, artifact 저장소 |
| `router` | job → route → workspec 생성 |
| `policy` | risk/approval/command/scope/budget 판정 |
| `provider` | Codex/Local/Gemini 등 실행 adapter |
| `renderer` | 공통 원본 → provider 산출물 변환 |
| `hooks` | lifecycle hook 실행 |
| `quality` | validation/review/evidence |
| `context` | Context Pack, memory, compaction |
| `vcs` | git status/diff/worktree/patch |
| `observability` | event log, metrics, cost |

## 핵심 인터페이스

### ProviderAdapter

```text
prepare(workspec) -> ProviderRunRequest
start(request) -> ProviderRunHandle
poll(handle) -> ProviderRunStatus
collect(handle) -> ProviderRunResult
normalize(result) -> ReportSpec
cancel(handle) -> CancelResult
```

### PolicyEngine

```text
evaluate_job(job) -> RiskAssessment
evaluate_workspec(workspec) -> PolicyDecision
evaluate_command(command) -> CommandDecision
evaluate_changed_files(files, workspec) -> ScopeDecision
```

### StateStore

```text
create_job(request) -> job_id
write_artifact(job_id, path, content)
read_artifact(job_id, path)
update_run_state(job_id, patch)
append_event(job_id, event)
```

### HookEngine

```text
run(event_name, context) -> HookResult[]
```

## 금지할 결합

- ProviderAdapter가 직접 policy를 결정하지 않는다. PolicyEngine을 호출한다.
- Worker prompt가 run-state를 직접 수정하지 않는다. StateStore만 수정한다.
- Renderer가 원본 정책을 바꾸지 않는다. 산출물만 만든다.
- Local model adapter가 직접 파일 수정 권한을 갖지 않는다.

## 최소 코드 구현 순서

1. `StateStore` 파일 기반 구현.
2. schema validator.
3. FakeProviderAdapter.
4. CodexProviderAdapter.
5. RouterEngine minimal.
6. CLI `run` command.
7. Report collector.
8. ValidationEngine minimal.
