# P-0076 Star-Control 자체 Code Health 적용 — 2026-07-29

## 범위와 판정 경계

- 기준 source는 `a5dec7cc50a4c602d0d975c4ab73666e4750d52c`에서 시작한 `codex/p0076-self-code-health` 작업 묶음이다.
- 목표는 Star-Control의 `project.register → scan.run → index.status|search → finding.list|diagnostic.list` 경로를 Star-Control source 자체에 적용하고, 선행 결함을 고친 뒤 실제 finding 하나를 bounded refactor로 닫는 것이다.
- source, installed Runtime, derived management/index state를 분리한다. 이 Slice는 source 구현과 격리 dogfood를 소유하며 Runtime 교체·Codex cache/DB 직접 수정·dependency 설치·remote push는 수행하지 않는다.

## 재현한 선행 결함

| 경계 | 재현 | 판정 |
|---|---|---|
| durable goal | installed `goal.start` operation `opn_01KYP34WA7RB4VWSY4QA3AQ39B` | `completion_evidence_ref` 추가 전 v1 GoalRecord fingerprint를 현재 reader가 거부해 `GOAL_STORE_CORRUPT` |
| installed self-scan | operation `opn_01KYP3GA46VTFT98NAH5M9XB3S`, scan `scn_01KYP3GAMABHNTG110YM0ZTG89` | operation은 성공했지만 scan은 `incomplete`; `sensitive_literal_discarded` 때문에 current/LKG finding projection으로 승격되지 않음 |
| source full-scan persistence | 격리 source dogfood의 첫 두 시도 | 약 21MB aggregate `CodeIndexProjection`을 16MiB SQLite length limit이 `string or blob too big`으로 거부 |
| machine-code closure | 첫 source FULL | 새 redaction limitation 5개가 closed stable-error catalog에 없어 계약 테스트가 fail-closed |

installed project identity는 project `prj_01KYMDB5QKB1ZJYEZ2V7GZXP4S`, checkout `cko_01KYMDB5QKRMBC8NDK51H8804S`로 확인했다. incomplete generation을 current finding 결과로 승격하거나 installed state 파일을 고쳐 성공처럼 만들지 않았다.

## 구현한 교정

1. `GoalRecord` reader가 `completion_evidence_ref=None`인 record에 한해 exact legacy fingerprint를 허용한다. 현재 fingerprint와 legacy fingerprint 모두 tamper는 거부하며, 다음 정상 write는 현재 fingerprint로 reseal한다.
2. project observation은 민감 literal이 있는 파일도 상대 경로·digest inventory에 유지한다. raw text는 index projection 직전 폐기하고 `INDEX_*_REDACTED` limitation으로 설명한다.
3. syntax/semantic adapter가 돌려준 name·kind·visibility·limitation parameter도 persistence redactor를 통과한 값만 projection에 남긴다.
4. text adapter version을 2로 올려 과거 raw text projection 재사용을 차단한다.
5. SQLite management document limit을 32MiB로 올리고 `SQLITE_TOOBIG`을 `QuotaExceeded`로 분류한다. 실제 self-scan snapshot 21,263,878 bytes에 대해 약 11MiB의 bounded headroom을 둔다.
6. 새 limitation code 5개를 stable-error catalog에 additive 등록하고 catalog 기대 개수를 533으로 동기화했다. generated Schema는 `star-schema-gen` 정식 경로로 재생성했다.

## self-scan과 finding 기반 리팩터링

격리 source full scan `scn_01KYP58EJXA5A5QX1H6XQHNZ9J`은 `succeeded`했고 1,103개 source에서 Finding 3,151개를 만들었다.

| rule | finding 수 |
|---|---:|
| hardcoding candidate | 2,076 |
| unused surface candidate | 721 |
| structural clone candidate | 354 |

required partition은 모두 성공했고 기존 blocker `sensitive_literal_discarded`는 사라졌다. optional/frontier limitation 6개는 성공으로 재작성하지 않고 보존했다.

첫 bounded refactor는 `apps/star-controller/src/main.rs`의 search/status cursor codec 중복이다. 처음에는 두 decoder가 126 normalized token clone으로 잡혔고, wrapper만 남긴 중간 변경에서는 wrapper와 encoder clone이 다시 잡혔다. 최종적으로 canonical encode/decode generic helper를 직접 호출하게 합쳤다. post-refactor incremental scan `scn_01KYP66XD8XVPG9S4S0MEN733J`은 `succeeded`, 전체 Finding 3,168개였고 기존 cursor codec 위치의 structural-clone occurrence는 0이었다. 전체 Finding 수 차이는 Slice 중 추가된 source·test·catalog byte까지 포함한 새 snapshot 차이이며 품질 향상 지표로 사용하지 않는다.

## 회귀 검증

| 검증 | 결과 |
|---|---|
| legacy GoalRecord contract | 1/1 pass; exact legacy accept, current reseal, tamper reject |
| legacy GoalStore append | 1/1 pass; legacy store load 뒤 새 goal append |
| cursor contract | 3/3 pass; canonical/noncanonical/duplicate-key/stale binding 유지 |
| `star-project` | 31/31 pass; sensitive inventory, toolchain/guidance와 adapter projection redaction 포함 |
| SQLite aggregate document | 1/1 pass; 16MiB 초과 허용과 overflow `QuotaExceeded` 분류 |
| schema/catalog/inventory | Schema `--check` pass, feature 23/23, Schema 217, Profile 16/16, Runtime EXE 4/4, stable error 533, MCP 170/170 |
| pre-document FULL | `target/validation/20260729T055903012Z-1456/report.json`, 11/11 pass, 210,974ms |
| document-inclusive closure candidate | `target/validation/20260729T060431994Z-13004/report.json`, 11/11 pass, 145,875ms |

위 두 report를 이 문서와 `PLANS.md`의 마지막 byte에 대한 증거로 대신하지 않는다. 원장 DONE 전환 뒤 `catalog/product-source-evidence.json`을 다시 생성하고 source 문서를 더 수정하지 않은 채 final current-byte FULL과 STRICT 리뷰를 수행한다. exact final report는 생성 validation artifact와 최종 handoff가 소유한다.

## 남은 경계

- installed Runtime은 이 source를 포함하지 않는다. 따라서 installed `goal.start` 복구와 installed self-scan 성공은 별도 Runtime 교체 승인 뒤 재검증해야 한다.
- 격리 dogfood state는 테스트 파생 상태이며 current source 정본이 아니다. 사용자 management state와 Codex runtime DB/cache는 직접 수정하지 않았다.
- linked worktree가 공유하는 `target/`에서 증분 compilation finalize `access denied` 경고가 있었지만 test/Gate exit는 성공했다. 정책상 `target/`을 정리하지 않았다.
- 서명, installer, publish, push는 이 Slice 범위 밖이다.
