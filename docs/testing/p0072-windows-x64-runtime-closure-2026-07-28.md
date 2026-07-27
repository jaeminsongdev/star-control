# P-0072 Windows x64 Runtime closure

## 범위

P-0071 뒤 남은 Windows x64 source/runtime 경계를 닫는다. Authenticode·trusted timestamp·public publish와 Windows x64가 아닌 architecture는 이 Slice에서 제외한다. 설치 전환은 fixed Codex integration을 바꾸지 않는 `star update stage → inspect → apply` Runtime-only 경로만 사용하며, `apply` mutation owner는 installed `star-updater.exe`다.

External provider는 정본 계약대로 별도 취급한다. 이 PC에서 `cargo-mutants 27.1.0`과 pinned `rust-analyzer` executable은 관찰됐지만 mutation·semantic-refactor port에 맞는 registered descriptor/protocol/result artifact가 없는 executable을 임의 provider 완료로 승격하지 않는다. Scorecard·OpenRewrite는 설치하지 않는다. 해당 real result는 `unavailable|unverified`로 유지한다.

## Source closure

- `RegisteredProcessCheckExecutor`는 Windows child를 `CREATE_SUSPENDED`로 시작한다.
- launcher는 child를 `KILL_ON_JOB_CLOSE|DIE_ON_UNHANDLED_EXCEPTION` Job Object에 배정한 뒤 ToolHelp로 찾은 initial thread만 resume한다. create/assign/resume 실패는 provider code 실행이나 uncontained fallback 없이 fail-closed다.
- timeout·wait error는 Job 전체와 exact direct child에 종료를 요청하고 bounded Job accounting이 `ActiveProcesses == 0`을 증명하지 못하면 `outcome_unknown`이다. blocking `wait()`로 validation을 무기한 정지시키지 않는다.
- `pwsh → pwsh` 실제 descendant fixture는 parent timeout과 조기 success exit 양쪽에서 recorded child PID가 사라지는지 확인한다. 조기 success exit 뒤 bounded accounting settle에도 active descendant가 남아 있으면 cleanup 성공과 별개로 결과를 `outcome_unknown/unverified`로 보존한다.

Focused evidence:

- `cargo test -p star-validation --locked process_executor::tests -- --nocapture`: 10/10 pass.
- descendant fixture: timeout과 조기 success exit 2/2 pass; 조기 success는 fail-closed `outcome_unknown` corpus다.
- dependency 변화는 기존 workspace pin `windows 0.62.2`를 `star-validation`의 Windows target에 연결한 것뿐이며 package/version 설치는 없다.

## Source 검증

- inventory: feature 23/23, Schema 217, MCP 170/170, Profile 16/16, Runtime executable 4/4.
- `target/validation/20260727T172403972Z-36576/report.json`: FULL 11/11 complete/stable/pass, 209,390ms.
- STRICT/HIGH review는 Windows handle 수명, fail-closed launch/cleanup, 정상 parent 종료 뒤 descendant 잔존, 실제 PID fixture를 확인했다.

## 남은 Gate

- clean x64 Runtime generation 생성·stage·inspect
- updater-owned runtime apply 후 Codex UI/child PID·creation time과 fixed MCP/integration hash 불변 확인
- installed generation/source revision readback
- local commit, `main` push, remote SHA readback

이 문서는 위 live Gate가 실제 통과하기 전에는 설치 또는 배포 완료를 주장하지 않는다.
