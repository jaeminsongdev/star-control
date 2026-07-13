# 최종 Repository·Package·문서 구조

## 문서의 역할

이 문서는 [구현 대상 기능](../features/README.md)의 A01~D03과 15개 작업 Profile을 모두 구현했을 때 Star-Control 저장소가 가져야 할 최종 물리 구조와 책임 경계를 정한다.

문서 폴더 migration과 첫 MCP 수직 Slice의 `star-contracts`, `star-ipc`, `star-controller`, `star-mcp`, `star-cli`·검증 도구는 구현됐다. P0에서는 `star-domain`, `star-ports`, `star-project`, `star-validation`, `star-execution`, `star-application`, `star-state`, `star-evidence`의 최소 Package와 private persistence adapter를 만들었다. generated 관리 Schema·fixture와 실제 완료 판정은 [최종 구현 로드맵](../roadmap/final-implementation.md)·`PLANS.md`를 따른다. 아래 큰 module tree 중 P1 이후 module을 현재 존재하는 것으로 읽지 않는다.

이 문서가 정하는 것은 다음과 같다.

- 실행 파일과 내부 Package의 책임
- Package 사이의 허용 의존 방향
- Codex Plugin·MCP·Hook·App Server 연결의 물리 위치
- 23개 구현 기능과 Package의 대응
- 설정·Profile·검증 규칙·Schema·Corpus의 정본 위치
- 제품 문서·개발 문서·조사 자료·생성 문서의 경계
- 새 기능, adapter, 검사, Profile을 추가하는 절차
- 생성물, 사용자 상태, 로컬 레거시를 source와 분리하는 규칙

## 물리 구조 전제

최종 제품 runtime은 하나의 Rust Cargo workspace로 구성한다.

- Windows용 CLI, 장시간 Controller와 로컬 MCP server를 native 실행 파일로 배포한다.
- 제품 동작은 Rust Package에 두고 PowerShell script에는 build·test·package 호출만 둔다.
- Plugin의 manifest·Skill·Hook·MCP 설정은 공식 형식에 맞는 JSON·Markdown으로 관리한다.
- 사용자 설정은 TOML, 공유 선언은 TOML·versioned contract, local management persistence는 backend-neutral repository 뒤의 DB, 큰 증거와 export는 version이 있는 JSON·JSONL·artifact를 사용한다.
- 기계 계약은 Rust의 직렬화 type을 정본으로 삼고 JSON Schema와 reference 문서는 생성한다.
- 구체적인 Rust library, installer 기술과 외부 도구는 구현 단계 직전에 다시 조사한다.

언어 선택이 나중에 바뀌더라도 이 문서의 process 경계, Package 책임과 의존 방향은 유지한다.

## 구조 설계 원칙

1. 기능 하나마다 Package를 만들지 않는다. 독립적인 변경 이유와 외부 의존 경계가 있을 때만 Package를 나눈다.
2. CLI, MCP와 Controller에는 업무 판단을 넣지 않는다. 모든 진입점은 같은 application use case를 사용한다.
3. 파일, process, Git, Codex와 원격 서비스 접근은 port를 거쳐 adapter에서만 수행한다.
4. 계획, 배정, 권한, 검사와 병합 engine은 구체적인 외부 도구를 알지 않는다.
5. built-in 검사 9개는 하나의 Package 안에서 module로 나누고 공통 validation engine을 재사용한다.
6. 15개 개발 작업은 별도 engine이 아니라 data-driven Profile로 유지한다.
7. 직렬화 계약, 상태 migration, 설정 병합과 증거 생성의 정본을 한 곳씩만 둔다.
8. 생성 파일과 사람이 편집하는 정본을 같은 폴더에 섞지 않는다.
9. 이름이 모호한 `common`, `shared`, `utils`, `misc` Package를 만들지 않는다.
10. local-only `legacy/`와 실행 산출물은 현재 설계와 release 입력에서 제외한다.

## 최종 상위 구조

```text
Star-Control/
├─ .github/                         # 공개 저장소 운영과 CI 정의
├─ .star-control/                   # Star-Control 자체를 dogfooding하는 프로젝트 설정
├─ apps/                            # 사용자가 실행하는 3개 얇은 binary
├─ crates/                          # 제품 runtime Package
│  ├─ foundation/                   # 계약·domain·port·설정
│  ├─ control/                      # project·application·실행·검증 engine
│  ├─ infrastructure/               # DB·artifact 같은 concrete local adapter
│  └─ adapters/                     # Codex·Windows·Git·원격·IPC 연결
├─ integrations/                    # Codex Plugin 배포 source
├─ catalog/                         # built-in Profile·정책·도구·검사 metadata 정본
├─ specs/                           # 생성된 기계 계약과 호환성 fixture
├─ corpus/                          # 검증기 보호용 정상·실패·회귀 사례
├─ evals/                           # 배정·검증 규칙 비교 평가 자료
├─ tests/                           # Package 밖 통합·E2E·복구·보안 검사
├─ tools/                           # schema·문서·catalog·release 생성용 개발 binary
├─ scripts/                         # 위 도구와 Cargo 명령을 호출하는 얇은 wrapper
├─ packaging/                       # Windows 설치·update·uninstall·release 입력
├─ examples/                        # 사용자 설정과 확장 예시
├─ docs/                            # 사람이 읽는 현재 설계·계약·운영 문서
├─ licenses/                        # 제3자 고지와 license 자료
├─ legacy/                          # 로컬 읽기 전용 과거 자료, release 제외
├─ AGENTS.md
├─ README.md
├─ PLANS.md                         # 현재 작업만 담는 짧은 원장
├─ CHANGELOG.md
├─ CONTRIBUTING.md
├─ SECURITY.md
├─ LICENSE
├─ Cargo.toml                       # workspace와 공통 dependency 정책
├─ Cargo.lock
├─ rust-toolchain.toml
├─ deny.toml                        # dependency·license 정책
├─ .editorconfig
├─ .gitattributes
└─ .gitignore
```

`target/`, `dist/`, coverage 결과, `.ai-runs/`, 임시 worktree와 사용자 secret은 이 source 구조에 포함하지 않는다.

최종 runtime code는 3개 실행 파일과 bounded 내부 Package 집합으로 구성한다. P0는 위 8개 책임 Package를 실제 workspace member로 사용하고 새 DB 전용 public Package를 만들지 않는다. `star-state`와 `star-evidence`는 `crates/infrastructure/`, project·validation·execution·application은 `crates/control/`, contract·domain·port는 `crates/foundation/`에 둔다. Package 수를 기능 수와 맞추지 않고, 9개 검증 기능은 `star-checks` module로, 15개 작업 유형은 `catalog/profiles` data로 흡수한다.

## 실행 파일 구조

세 binary는 서로 다른 사용자·protocol 경계를 담당하지만 내부 판단을 중복 구현하지 않는다.

```text
apps/
├─ star-cli/
│  ├─ Cargo.toml
│  ├─ README.md
│  └─ src/
│     ├─ main.rs                    # argument parsing과 종료 코드
│     ├─ commands/                  # project·scan·finding·change·validate·management와 기존 명령
│     ├─ render/                    # terminal text·JSON 출력
│     └─ bootstrap/                 # Controller 시작 확인과 doctor 진입
├─ star-controller/
│  ├─ Cargo.toml
│  ├─ README.md
│  └─ src/
│     ├─ main.rs                    # 유일한 composition root
│     ├─ bootstrap/                 # config·catalog·management repository·live Registry·adapter 조립
│     ├─ registry_runtime/          # candidate state·demand scan·atomic publish·LKG
│     ├─ lifecycle/                 # 시작, 정상 종료, crash 재개
│     ├─ server/                    # 로컬 IPC 요청 처리
│     ├─ workers/                   # stage·validation·merge worker
│     └─ health/                    # readiness와 진단 snapshot
└─ star-mcp/
   ├─ Cargo.toml
   ├─ README.md
   └─ src/
      ├─ main.rs                    # STDIO MCP server 시작
      ├─ protocol/                  # rmcp lifecycle·고정 capability
      ├─ surface/                   # 고정 search·describe·operation·risk lane 목록
      ├─ instructions/              # 고정 generic 사용 순서
      └─ translate/                 # 고정 MCP call과 typed IPC 변환
```

### 실행 파일별 금지 책임

| 실행 파일 | 담당 | 넣지 않는 것 |
|---|---|---|
| `star.exe` | 사람이 쓰는 terminal 명령과 표시 | DB·artifact·상태 직접 접근, 계획·권한 판단, Git 직접 실행, Codex·App Server·AI·OpenAI API 호출 |
| `star-controller.exe` | 상태·관리 DB의 단일 writer와 전체 use case 조립 | 사용자 UI, 자체 AI 호출, HTTP API server |
| `star-mcp.exe` | 고정 MCP surface와 Controller IPC 변환 | TOML·Registry·tool별 handler·EXE path·parser, DB·artifact 직접 접근, 별도 상태·정책, Codex App Server 직접 제어 |

CLI와 MCP는 모두 local IPC client다. Controller만 application use case를 실행하고 상태를 쓴다. 이 원칙으로 같은 명령이 진입점마다 다르게 동작하는 일을 막는다.

## Cargo Workspace Package 구조

### 1. Foundation Package

```text
crates/foundation/
├─ star-contracts/
│  └─ src/
│     ├─ ids.rs                     # Goal·Stage·ProjectRevision·Scan·Finding·Symbol·Artifact ID
│     ├─ goal.rs                    # GoalSpec·TaskContract
│     ├─ stage.rs                   # StageSpec·StageGraph
│     ├─ route.rs                   # model_role·reasoning_effort·stage_mode·execution_mode·CapabilitySnapshot
│     ├─ context.rs                 # ContextPack summary와 source reference
│     ├─ management.rs              # Project·Revision·WorkspaceSnapshot·ScanRun·StoreStatus
│     ├─ source_graph.rs            # CanonicalSource·Symbol·SymbolReference
│     ├─ finding.rs                 # Rule·Finding·Occurrence·Suppression·Baseline·Disposition
│     ├─ change.rs                  # ChangePlan·PatchSet·ChangeRecipe·ValidationResult
│     ├─ permission.rs              # PermissionPlan·ApprovalRequest
│     ├─ validation.rs              # ValidationPlan·result·gate decision
│     ├─ diagnostic.rs              # 공통 diagnostic 형식
│     ├─ evidence.rs                # EvidenceBundle·provenance·ReviewPack
│     ├─ checkpoint.rs              # Checkpoint·handoff
│     ├─ merge.rs                   # MergePlan·conflict·result
│     ├─ cost.rs                    # usage·time·rework metric
│     ├─ recovery.rs                # recovery plan·reproduction pack
│     ├─ event.rs                   # append-only event envelope
│     ├─ ipc.rs                     # CLI·MCP·Controller local protocol DTO
│     └─ version.rs                 # contract version과 compatibility
├─ star-domain/
│  └─ src/
│     ├─ goal/                      # 목표와 단계 invariant
│     ├─ state_machine/             # 허용 상태 전이
│     ├─ decision/                  # 확정·미확인·차단 표현
│     ├─ risk/                      # 위험 종류와 합성
│     ├─ approval/                  # 승인 유효성·만료
│     ├─ budget/                    # 비용·시간·동시 실행 한도
│     ├─ scope/                     # 경로·행동 범위 계산
│     └─ error/                     # stable error category
├─ star-ports/
│  └─ src/
│     ├─ clock.rs
│     ├─ id_generator.rs
│     ├─ state_store.rs
│     ├─ management_repository.rs   # backend-neutral query·transaction·lifecycle port
│     ├─ artifact_store.rs
│     ├─ project_root_binding.rs    # opaque binding과 process-memory path resolution
│     ├─ lock.rs
│     ├─ filesystem.rs
│     ├─ process.rs
│     ├─ tool_executor.rs
│     ├─ codex.rs
│     ├─ git.rs
│     ├─ remote_git.rs
│     ├─ secret_store.rs
│     └─ telemetry.rs
└─ star-config/
   └─ src/
      ├─ model.rs                   # typed config
      ├─ loader.rs                  # 제품·사용자·프로젝트·Goal·CLI 계층
      ├─ merge.rs                   # 단 하나의 병합 구현
      ├─ provenance.rs              # effective value 출처
      ├─ validate.rs                # unknown·invalid 설정 처리
      ├─ migration.rs               # config version 이동
      ├─ registry/                  # 외부 선언 Registry 공통 loader
      │  ├─ source.rs               # release·user·project tools.d 발견
       │  ├─ manifest.rs             # ToolPackageManifest parse
       │  ├─ trust.rs                # manifest·Schema·EXE fingerprint
       │  ├─ resolve.rs              # ID·Schema·backend reference 해석
       │  ├─ update_policy.rs        # pinned·compatible·follow-path 해석
       │  ├─ search_index.rs         # generic search·describe용 index
       │  └─ snapshot.rs             # immutable ToolRegistrySnapshot 후보 생성
      └─ effective.rs               # 최종 설정과 설명
```

Foundation Package는 구체적인 filesystem, process, Codex, Git library에 의존하지 않는다. `star-contracts`의 직렬화 type이 JSON Schema 생성의 유일한 정본이며 같은 type을 다른 Package에서 다시 선언하지 않는다.

### 2. Control·Infrastructure Package

아래 tree는 최종 logical package 목록을 한 번에 보여준다. `[infrastructure]` 표기가 있는 두 Package의 실제 root는 `crates/infrastructure/`이고 나머지는 `crates/control/`이다. P0 현재 구현은 각 Package의 `src/lib.rs` 수직 Slice이며 아래 세부 module directory는 해당 책임이 커질 때만 만든다.

```text
crates/control/
├─ star-planning/
│  └─ src/
│     ├─ clarification/             # A01 질문과 모호함 해소
│     ├─ task_contract/             # 목표·범위·완료 조건
│     ├─ decompose/                 # A02 성격 기반 단계 분해
│     ├─ stage_graph/               # 의존·병렬 관계
│     ├─ replan/                    # 새 사실과 범위 변화
│     └─ completion/                # 단계·목표 완료 조건
├─ star-project/
│  └─ src/
│     ├─ roots/                     # project·workspace root 발견
│     ├─ classify/                  # source·test·docs·generated 등
│     ├─ toolchain/                 # 언어·build·package manager 발견
│     ├─ guidance/                  # AGENTS·README·정본 우선순위
│     ├─ context/                   # A03 Context Pack 선택
│     ├─ impact/                    # A04 변경 영향
│     ├─ risk_paths/                # 위험 경로와 confidence
│     ├─ freshness/                 # revision·신선도·누락 가능성
│     ├─ revision/                  # ProjectRevision·WorkspaceSnapshot
│     ├─ scan/                      # source enumeration·scan generation
│     ├─ source_graph/              # CanonicalSource·Symbol·Reference
│     └─ cache/                     # 선택적 discovery cache
├─ star-routing/
│  └─ src/
│     ├─ capability/                # Codex 지원 기능 snapshot 해석
│     ├─ constraints/               # 필수 능력·권한 hard constraint
│     ├─ model_role/                # Sol·Terra·Luna 역할
│     ├─ reasoning/                 # 생각 깊이 선택
│     ├─ execution_mode/            # Max·병렬·검토 방식
│     ├─ score/                     # 품질·위험·비용·시간 비교
│     ├─ fallback/                  # 한도·미지원 시 대체
│     ├─ explain/                   # 사람이 읽는 배정 이유
│     └─ shadow/                    # 실제 작업을 바꾸지 않는 비교
├─ star-policy/
│  └─ src/
│     ├─ action/                    # 행동 종류 분류
│     ├─ permission/                # auto·prompt·deny
│     ├─ approval/                  # 승인 요청·유효성·재승인
│     ├─ scope/                     # 허용·금지 경로와 범위 확대
│     ├─ budget/                    # 유료·불명 비용·한도
│     ├─ isolation/                 # process·network·environment 제한
│     ├─ secret/                    # secret 후보와 전달 제한
│     ├─ handoff/                   # 외부 전달 자료 정책
│     └─ decision/                  # 정책 결과와 설명
├─ star-execution/
│  └─ src/
│     ├─ run/                       # Goal run lifecycle
│     ├─ stage/                     # Stage 실행 lifecycle
│     ├─ attempt/                   # 재시도·승급·중단
│     ├─ queue/                     # 실행·검증·병합 대기열
│     ├─ parallel/                  # 동시 실행 한도와 조정
│     ├─ checkpoint/                # 경계별 Checkpoint
│     ├─ handoff/                   # 다음 Codex용 최소 인계
│     ├─ recovery/                  # Controller 중단 뒤 재개
│     └─ cancellation/              # pause·cancel·interrupt
├─ star-validation/
│  └─ src/
│     ├─ plan/                      # 검사 계획과 단계
│     ├─ selector/                  # 변경·위험 기반 검사 선택
│     ├─ registry/                  # check descriptor 발견
│     ├─ runner/                    # tool 실행과 timeout
│     ├─ normalize/                 # 공통 diagnostic 변환
│     ├─ claim/                     # 완료 주장과 근거 대조
│     ├─ suppression/               # 이유·fingerprint·만료
│     ├─ findings/                  # Rule 결과·Occurrence·Finding projection
│     ├─ baseline/                  # existing·new·changed 비교
│     ├─ disposition/               # local triage와 stale 판정
│     ├─ gate/                      # AUTO_PASS·HUMAN_REVIEW·BLOCK
│     └─ review_pack/               # Review Pack과 재작업 지시
├─ star-checks/
│  └─ src/
│     ├─ change_scope/              # B01 diff·범위·증거
│     ├─ test_trust/                # B02 테스트 약화·회귀 증거
│     ├─ validator_guard/           # B03 검증기 자기보호
│     ├─ contract_architecture/     # B04 계약·구조·설정·migration
│     ├─ security_supply_chain/     # B05 secret·dependency·workflow
│     ├─ failure_recovery/          # B06 재현·원인·복구
│     ├─ docs_environment/          # B07 문서·설정·개발 환경
│     ├─ performance_build/         # B08 성능·자원·build
│     └─ release_deploy/            # B09 CI·release·배포 준비
├─ [infrastructure] star-state/
│  └─ src/
│     ├─ layout/                    # 사용자·프로젝트 상태 위치
│     ├─ repository/                # global/project ManagementRepositorySet과 backend adapter
│     ├─ transaction/               # store-local event·projection·idempotency·revision
│     ├─ coordination/              # cross-store operation·participant receipt·recovery
│     ├─ scan_generation/           # invisible batch와 atomic visible publish
│     ├─ journal/                   # append-only event
│     ├─ artifacts/                 # content-addressed artifact
│     ├─ atomic_write/              # 안전한 교체
│     ├─ locks/                     # run·stage writer lock
│     ├─ migration/                 # state version 이동
│     ├─ recovery/                  # 손상·임시 파일 검사와 복구본
│     ├─ backup/                    # consistent backup·manifest·restore
│     ├─ integrity/                 # structure·relation·fingerprint·artifact 검사
│     └─ retention/                 # 보존·정리 plan
├─ [infrastructure] star-evidence/
│  └─ src/
│     ├─ provenance/                # 누가·무엇을·어떤 revision에서
│     ├─ changes/                   # 실제 diff와 baseline
│     ├─ validations/               # 실행·실패·미실행 근거
│     ├─ costs/                     # 시간·사용량·재작업
│     ├─ risks/                     # 남은 위험과 미확인
│     ├─ report/                    # 최종 보고
│     ├─ export/                    # 진단 Pack·공개 보고서
│     └─ redaction/                 # 저장·출력 전 가림
├─ star-vcs/
│  └─ src/
│     ├─ baseline/                  # 시작 revision과 dirty state
│     ├─ overlap/                   # 병렬 수정 겹침 판단
│     ├─ worktree/                  # 작업 복사본 plan
│     ├─ local_review/              # 로컬 검토 요청 정보
│     ├─ merge_queue/               # 의존 순서와 준비 상태
│     ├─ conflict/                  # 충돌 분류·해결 후 검사
│     ├─ remote_state/              # branch·PR·check 연결
│     └─ multi_repo/                # 여러 프로젝트 graph
├─ star-evaluation/
│  └─ src/
│     ├─ corpus/                    # 실제 성공·실패 사례 loading
│     ├─ grader/                    # 결정적 채점 우선
│     ├─ metrics/                   # 품질·안전·시간·비용
│     ├─ comparison/                # 배정·검증 규칙 A/B 비교
│     ├─ shadow/                    # 비개입 평가
│     ├─ regression/                # 정책 변경 회귀
│     └─ recommendation/            # 검토 가능한 변경 제안
└─ star-application/
   └─ src/
      ├─ commands/                  # 상태를 바꾸는 use case
      │  ├─ start_goal.rs
      │  ├─ manage_project.rs
      │  ├─ scan_findings.rs
      │  ├─ decide_finding.rs
      │  ├─ plan_patch_validate.rs
      │  ├─ manage_store.rs
      │  ├─ clarify_goal.rs
      │  ├─ plan_goal.rs
      │  ├─ approve_plan.rs
      │  ├─ run_stage.rs
      │  ├─ pause_resume_cancel.rs
      │  ├─ validate_review.rs
      │  ├─ merge.rs
      │  ├─ close_goal.rs
      │  ├─ recover.rs
      │  └─ export.rs
      ├─ queries/                   # project·scan·finding·change·store와 기존 조회
      ├─ coordinator/               # Package 사이 workflow 조정
      ├─ transaction/               # 상태·event·artifact commit 경계
      └─ service.rs                 # Controller가 호출하는 단일 façade
```

`star-checks` 안의 9개 module은 서로의 내부 구현을 import하지 않는다. 공통 동작은 `star-validation`의 공개 계약을 사용한다. 특정 검사군이 독립 dependency와 별도 release 주기를 가질 정도로 커졌을 때만 별도 Package로 분리한다.

### 3. Adapter와 IPC Package

```text
crates/adapters/
├─ star-adapter-codex/
│  └─ src/
│     ├─ app_server/                # STDIO JSONL client와 protocol 변환
│     ├─ capability/                # model·기능 조회
│     ├─ thread/                    # start·resume·fork
│     ├─ turn/                      # start·interrupt·event
│     ├─ review/                    # 독립 Codex review
│     ├─ process/                   # App Server lifecycle
│     └─ normalize/                 # Codex event·error 정규화
├─ star-adapter-windows/
│  └─ src/
│     ├─ filesystem/                # Windows path·ACL·atomic file
│     ├─ watcher/                   # ReadDirectoryChangesW·overflow rescan
│     ├─ identity/                  # final path·file ID·hash·Authenticode lease
│     ├─ process/                   # suspended child·handle list·Job Object
│     ├─ appcontainer/              # 호환 adapter brokered isolation
│     ├─ environment/               # env 전달과 가림
│     ├─ user_data/                 # APPDATA·LOCALAPPDATA 위치
│     ├─ root_binding/              # raw path를 DB 밖 opaque binding으로 보호
│     ├─ secret_store/              # Windows 자격 증명 저장소 경계
│     ├─ startup/                   # Controller 시작 방식
│     └─ clock/
├─ star-adapter-git/
│  └─ src/
│     ├─ repository/
│     ├─ status_diff/
│     ├─ worktree/
│     ├─ branch_commit/
│     ├─ merge_conflict/
│     └─ capability/
├─ star-adapter-remote-git/
│  └─ src/
│     ├─ discovery/                 # 사용 가능한 GitHub·remote 도구 확인
│     ├─ branch_pr/
│     ├─ checks/
│     ├─ merge/
│     ├─ release/
│     └─ normalize/
└─ star-ipc/
   └─ src/
      ├─ protocol/                  # version handshake와 envelope
      ├─ client/                    # CLI·MCP 공용 client
      ├─ server/                    # Controller local server
      ├─ named_pipe/                # Windows named pipe transport
       ├─ access/                    # 현재 사용자 ACL·PID image 확인
       ├─ auth/                      # DPAPI per-user key·HMAC handshake
      └─ framing/                   # 요청·응답·event stream framing
```

Adapter는 외부 결과를 공통 type으로 바꾸기만 한다. 유료 여부, 승인, 재시도, 검사 생략과 완료 판단은 adapter가 결정하지 않는다.

## Package 의존 방향

```text
apps
  ├─ star-controller ──> star-application ──> control engines
  │                           │                    │
  │                           └──────────────> star-ports
  │                                                ▲
  ├─ star-cli ──> star-ipc ──> star-contracts      │
  └─ star-mcp ──> star-ipc ──> star-contracts
                                                   │
                                                   │
adapters ──────────────────────────────────────────┘

control engines ──> foundation
star-checks ──────> star-validation + star-ports + star-contracts
star-evaluation ──> evidence·routing·validation의 공개 결과만 사용
```

### 허용 규칙

- `foundation`은 다른 workspace Package에 의존하지 않는다. 단, `star-ports`는 `star-contracts`와 `star-domain`만 사용할 수 있다.
- `control`은 `foundation`과 같은 계층의 명시된 공개 Package에만 의존한다.
- P0의 `star-project`는 read-only project filesystem·Git 관찰, `star-execution`은 exact-hash local patch effect를 소유하는 명시적 boundary Package다. 두 Package는 management DB, network, AI client와 사용자 root locator 저장소를 알 수 없다.
- `infrastructure`는 `star-ports`를 구현하고 concrete local dependency를 가질 수 있지만 application 판단을 소유하지 않는다.
- `adapters`는 `star-ports`, `star-contracts`, 필요한 protocol library에만 의존한다.
- `star-application`은 여러 engine을 조정할 수 있지만 engine 내부 type을 다시 소유하지 않는다.
- `star-controller`만 concrete adapter를 골라 port와 연결한다.
- `star-cli`와 `star-mcp`는 `star-application`과 adapter를 직접 import하지 않는다.
- DB backend dependency는 `star-state`의 private adapter에서만 사용하고 domain·application·CLI·MCP public type에 노출하지 않는다.
- project root의 raw 절대 경로는 Windows adapter의 root binding 해석 중 process memory에서만 사용한다.
- test fake는 실제 port를 구현하며 제품 code에 별도 fake 분기를 만들지 않는다.

### 금지 규칙

- 위에 명시한 `star-project` 관찰·`star-execution` patch boundary 밖의 engine에서 `std::fs`, raw process 실행, network client 또는 Git command 직접 호출
- adapter에서 승인·위험·완료 여부 판단
- CLI·MCP handler에서 상태 파일 직접 읽기·쓰기
- CLI·MCP·향후 Codex entry adapter에서 management DB handle·SQL·ArtifactStore 직접 사용
- 한 Package가 다른 Package의 database·폴더 배치를 알고 접근
- `star-contracts` 밖에서 같은 직렬화 type 재정의
- module 사이 순환 의존과 feature flag로 숨긴 순환 구조
- 공용 편의를 이유로 모든 Package가 의존하는 비대한 helper Package 생성

CI는 workspace dependency graph와 금지 import를 검사해 이 규칙을 기계적으로 지킨다.

## 0단계 소유권과 repository abstraction

[공통 개발 관리 계약](../contracts/development-management.md)은 새 Package가 아니라 기존 책임에 다음처럼 배치한다.

| 계약·행동 | 의미 소유 | persistence·I/O | 진입·조정 |
|---|---|---|---|
| Project·Revision·WorkspaceSnapshot·CanonicalSource | `star-contracts`, `star-domain`, `star-project` | `star-project` read-only filesystem·Git observer, project repository | `star-application` |
| ScanRun·Symbol·Reference | `star-project` | `star-state` scan generation | `star-application` |
| Rule·Finding·Occurrence·Suppression·Baseline·Disposition | `star-validation` | `star-state` projection, `star-evidence` artifact | `star-application` |
| ChangeRecipe·ChangePlan·PatchSet | `star-contracts`, `star-application`, `star-execution` | `star-state`, `star-evidence`, `star-execution` exact-hash filesystem effect | `star-application` |
| ValidationResult·GateDecision·ArtifactRef | `star-validation`, `star-evidence` | `star-state`, ArtifactStore | `star-application` |
| global Project directory·cross-project relation·coordination | `star-domain`, `star-application` | `star-state` global repository | `star-application` |
| DB version·migration·backup·integrity·rebuild·retention | `star-state` | global/project private backend adapter | Controller lifecycle·application command |

`star-ports::ManagementRepositorySet`은 `GlobalManagementRepository`, ProjectId별 `ProjectManagementRepository`, lifecycle, coordination, artifact와 root-binding port를 조립한다. 각 repository는 store-local transaction, project/source, scan generation, decision, change, event, cursor query와 retention operation만 노출한다. SQL row·table·connection·pragma·backend 오류는 port 밖으로 나오지 않는다. public input·result는 `star-contracts` type과 stable repository error category만 사용한다.

Controller의 single-store transaction 순서는 `artifact finalize -> repository begin -> expected revision·idempotency 검증 -> event+projection+store revision commit -> evidence export`다. DB commit에 실패한 artifact는 orphan으로 격리하고 성공 evidence로 노출하지 않는다. cross-store 작업은 `global prepared -> project participant transaction+receipt -> global completed` 순서이며 partial 상태를 ACID 성공으로 숨기지 않는다.

CLI-only composition은 filesystem·Git·tool runner·repository·artifact port만 조립한다. `star-adapter-codex`, App Server, 다른 AI provider와 OpenAI API client를 생성하거나 lazy-load하지 않는다. 향후 Codex 연동은 같은 `ManagementApplicationService` command를 호출하는 별도 entry adapter이며 별도 writer나 별도 engine을 만들지 않는다.

## Codex Integration 구조

```text
integrations/
└─ codex-plugin/
   ├─ .codex-plugin/
   │  └─ plugin.json                # 필수 Plugin manifest
   ├─ skills/
   │  └─ star-control/
   │     ├─ SKILL.md                # 개발 작업을 Star-Control로 시작하는 절차
   │     ├─ references/             # 필요한 최소 사용자 안내
   │     └─ assets/                 # Skill 전용 정적 자료
   ├─ hooks/
   │  └─ hooks.json                 # lifecycle event와 star hook 명령 연결
   ├─ .mcp.json                     # 설치된 star-mcp STDIO server 등록
   ├─ assets/                       # icon·logo·screenshot
   ├─ README.md
   ├─ PRIVACY.md
   └─ THIRD_PARTY_NOTICES.md
```

현재 공식 Plugin 구조에 따라 `.codex-plugin/` 안에는 `plugin.json`만 둔다. `skills/`, `hooks/`, `.mcp.json`, `assets/`는 Plugin root에 둔다. [.app.json](https://learn.chatgpt.com/docs/build-plugins#plugin-structure)은 connector app이 필요할 때만 추가하며 현재 제품에는 넣지 않는다.

### 연결 원칙

- Plugin은 설치와 안내 묶음이며 제품 상태를 소유하지 않는다.
- Hook 정의는 `star hook <event>`를 호출하고 정책 판단을 script에 복사하지 않는다.
- MCP server는 [공식 MCP 지원 방식](https://learn.chatgpt.com/docs/extend/mcp)에 맞춰 local STDIO로 실행한다.
- MCP의 server instructions에는 전체 도구에 공통인 시작 순서·제약·승인 경계만 둔다.
- Codex App Server는 Controller 안의 `star-adapter-codex`가 제어한다.
- App Server 연결은 [공식 기본 transport](https://learn.chatgpt.com/docs/app-server#protocol)인 STDIO JSONL을 사용한다.
- 실험적인 App Server WebSocket과 자체 HTTP API는 기본 구조에 넣지 않는다.
- Codex 기능 이름과 protocol version은 adapter 밖으로 새지 않게 해 제품 core가 Codex 변경에 직접 흔들리지 않게 한다.

## Built-in Catalog 구조

동작을 하드코딩하지 않아야 하지만 실행 code와 선언 data의 책임도 섞지 않는다.

```text
catalog/
├─ defaults/
│  └─ product.toml                  # 제품 불변 기본값
├─ policy-profiles/
│  ├─ safe_default.toml             # 공개 권한·비용 기본값
│  └─ personal_auto.toml            # 개인 자동 진행 정책
├─ profiles/
│  ├─ project_understanding.toml
│  ├─ change_planning.toml
│  ├─ refactor_codemod.toml
│  ├─ dependency_upgrade.toml
│  ├─ language_platform_migration.toml
│  ├─ data_config_db_migration.toml
│  ├─ api_contract_change.toml
│  ├─ test_correctness.toml
│  ├─ architecture_quality.toml
│  ├─ debug_recovery.toml
│  ├─ performance_build.toml
│  ├─ docs_config_environment.toml
│  ├─ ci_release_deploy.toml
│  ├─ security_supply_chain.toml
│  └─ ai_development_validation.toml
├─ tool-packages/
│  └─ star-control-core.toml        # required core action도 Registry 선언
├─ policies/
│  ├─ actions.toml                  # 행동 분류와 기본 승인 성격
│  ├─ risk-paths.toml               # auth·migration·workflow 등
│  ├─ redaction.toml
│  └─ retention.toml
├─ validators/
│  ├─ registry.toml                 # B01~B09 Check·Rule descriptor
│  ├─ gates.toml                    # 통과·검토·차단 기본 규칙
│  └─ suppressions.example.toml
├─ change-recipes/
│  ├─ README.md                     # ChangeRecipe 작성·fingerprint 경계
│  └─ registry.toml                 # built-in recipe 선언
├─ tools/
│  ├─ task-kinds.toml               # format·lint·build·test 등
│  ├─ protocols.toml                # argv_v1·star_json_stdio_v1 허용 경계
│  └─ README.md                     # 프로젝트 도구 등록 계약
└─ routing/
   ├─ roles.toml                    # Sol·Terra·Luna 역할
   ├─ defaults.toml
   ├─ escalation.toml
   └─ fallback.toml
```

Catalog에는 실행 명령 원문을 무제한으로 넣지 않는다. 프로젝트가 등록한 구조화 명령을 참조하며 shell 해석과 실행은 port를 거친다.

사용자·프로젝트 외부 EXE manifest는 repository의 built-in Catalog에 복사하지 않고 `%APPDATA%\Star-Control\tools.d`와 `<project>\.star-control\tools.d`에서 읽는다. 자세한 경계는 [외부 Tool Registry 계약](../contracts/external-tool-registry.md)이 소유한다.

### Catalog 편집 규칙

- 모든 항목은 stable ID와 format version을 가진다.
- Product default 변경은 기존 사용자 설정과 합쳐진 결과를 회귀 검사한다.
- 작업 Profile은 단계·필요 Context·도구 종류·검사·증거 요구만 선언한다.
- 정책 Profile은 승인·한도·최소 검사만 선언하고 프로젝트 설정은 이를 더 넓힐 수 없다.
- 특정 언어 도구 이름은 대상 프로젝트 설정 또는 adapter capability에 둔다.
- Catalog 변경은 Schema 검사와 positive·negative fixture를 반드시 통과한다.
- Rule과 ChangeRecipe는 stable ID·version·definition fingerprint를 가지며 raw shell, AI prompt와 DB query를 포함하지 않는다.

## 기계 계약과 생성물 구조

```text
specs/
├─ schemas/
│  ├─ v1/                           # star-contracts에서 생성, checked-in
│  │  ├─ goal.schema.json
│  │  ├─ project.schema.json
│  │  ├─ project-revision.schema.json
│  │  ├─ workspace-snapshot.schema.json
│  │  ├─ canonical-source.schema.json
│  │  ├─ scan-run.schema.json
│  │  ├─ rule.schema.json
│  │  ├─ finding.schema.json
│  │  ├─ occurrence.schema.json
│  │  ├─ symbol.schema.json
│  │  ├─ symbol-reference.schema.json
│  │  ├─ suppression.schema.json
│  │  ├─ baseline.schema.json
│  │  ├─ disposition.schema.json
│  │  ├─ change-plan.schema.json
│  │  ├─ patch-set.schema.json
│  │  ├─ change-recipe.schema.json
│  │  ├─ validation-result.schema.json
│  │  ├─ management-store-status.schema.json
│  │  ├─ stage.schema.json
│  │  ├─ route.schema.json
│  │  ├─ permission.schema.json
│  │  ├─ validation.schema.json
│  │  ├─ diagnostic.schema.json
│  │  ├─ evidence.schema.json
│  │  ├─ checkpoint.schema.json
│  │  ├─ merge.schema.json
│  │  ├─ event.schema.json
│  │  ├─ config.schema.json
│  │  ├─ profile.schema.json
│  │  ├─ policy-profile.schema.json
│  │  ├─ tool-package-manifest.schema.json
│  │  ├─ tool-registry-snapshot.schema.json
│  │  ├─ tool-trust-record.schema.json
│  │  ├─ tool-registry-cache.schema.json
│  │  ├─ external-tool-request.schema.json
│  │  ├─ external-tool-response.schema.json
│  │  └─ ipc.schema.json
│  └─ manifest.json                 # ID·version·hash 목록
├─ examples/
│  ├─ valid/                        # Schema별 최소·전체 예시
│  └─ invalid/                      # 거부 이유가 고정된 예시
├─ compatibility/
│  ├─ config/                       # 이전 config migration fixture
│  ├─ state/                        # 이전 state migration fixture
│  └─ ipc/                          # 지원 protocol version fixture
└─ generated/README.md              # 생성 명령과 직접 편집 금지 안내
```

정본과 생성물은 다음처럼 한 방향으로만 흐른다.

```text
star-contracts의 typed contract
  -> tools/schema-gen
  -> specs/schemas
  -> tools/doc-gen
  -> docs/generated
```

생성된 Schema나 reference를 직접 고치는 대신 contract type 또는 설명 source를 고친 뒤 다시 생성한다. CI는 재생성 diff가 남으면 실패한다.

## Corpus와 평가 자료

```text
corpus/
├─ change_scope/
├─ test_trust/
├─ validator_guard/
├─ contract_architecture/
├─ security_supply_chain/
├─ failure_recovery/
├─ docs_environment/
├─ performance_build/
└─ release_deploy/
   └─ 각 기능군/
      ├─ positive/                  # 허용해야 하는 사례
      ├─ negative/                  # 잡아야 하는 사례
      ├─ edge/                      # 경계 사례
      ├─ regression/                # 실제로 다시 막아야 하는 결함
      └─ adversarial/               # 검사 우회 시도

evals/
├─ routing/                         # 모델·생각 깊이·방식 배정 비교
├─ planning/                        # 단계 분해와 재계획 품질
├─ validation/                      # TP·FP·FN과 흔들림
├─ cost_time/                       # 재작업 포함 총효율
├─ end_to_end/                      # 실제 목표 단위 평가
├─ manifests/                       # dataset version·출처·가림 상태
└─ baselines/                       # 승인된 비교 기준
```

Corpus는 검증 규칙의 기계적 회귀 자료이고, evals는 배정과 전체 효용을 비교하는 자료다. 실제 사용자 작업을 넣을 때는 secret과 개인 경로를 제거하고 출처·동의·보존 정책을 기록한다.

## Test 구조

Package 내부 unit test는 해당 module 옆에 둔다. 여러 Package나 실제 process 경계를 검증하는 항목만 top-level `tests/`에 둔다.

```text
tests/
├─ mcp/                             # MCP 구현 검증 행렬의 exact test ID
│  ├─ conformance/
│  ├─ gateway/
│  ├─ ipc/
│  ├─ registry/
│  ├─ manifests/
│  ├─ process-runtime/
│  ├─ security/
│  ├─ recovery/
│  └─ codex-e2e/
├─ management/
│  ├─ contracts/                    # ID·fingerprint·redaction golden
│  ├─ repository/                   # backend-neutral conformance
│  ├─ scan-generation/              # batch·crash·atomic publish
│  ├─ migration-rebuild/            # backup·future version·corruption
│  └─ cli-only/                     # AI·App Server·network API 호출 0회
├─ contract/                        # Schema·catalog·protocol 호환
├─ conformance/
│  ├─ codex_adapter/                # recorded protocol fixture 기반
│  ├─ git_adapter/
│  ├─ remote_git_adapter/
│  ├─ state_store/
│  └─ check_runner/
├─ integration/
│  ├─ planning_to_routing/
│  ├─ execution_to_evidence/
│  ├─ validation_gate/
│  ├─ worktree_merge/
│  └─ multi_project/
├─ e2e/
│  ├─ cli_controller/
│  ├─ mcp_controller/
│  ├─ codex_app_server/
│  ├─ install_first_run/
│  ├─ update_rollback/
│  └─ uninstall_preserve_data/
├─ resilience/
│  ├─ crash_resume/
│  ├─ atomic_write/
│  ├─ lock_contention/
│  ├─ truncated_state/
│  └─ interrupted_merge/
├─ security/
│  ├─ path_escape/
│  ├─ approval_staleness/
│  ├─ secret_redaction/
│  ├─ ipc_access/
│  └─ plugin_hook_tamper/
└─ fixtures/
   ├─ projects/                     # 작고 합성된 여러 언어 프로젝트
   ├─ app_server/                   # 민감정보 없는 recorded event
   ├─ git_repositories/
   ├─ remote_responses/
   └─ state_versions/
```

실제 유료 호출, 원격 변경과 배포를 기본 CI에서 수행하지 않는다. 해당 검사는 별도 승인된 환경에서만 실행하고 로컬 deterministic fixture와 구분한다.

## 개발 도구·Script·Packaging 구조

```text
tools/
└─ xtask/
   ├─ Cargo.toml
   └─ src/
      ├─ main.rs
      └─ commands/
         ├─ schema.rs               # contract에서 Schema 생성·diff 확인
         ├─ docs.rs                 # CLI·MCP·config reference 생성
         ├─ catalog.rs              # Profile·policy·validator 정합성
         ├─ dependency.rs           # Package 의존 규칙 검사
         ├─ corpus.rs               # fixture manifest와 기대 결과 검사
         ├─ package.rs              # release staging
         └─ release_manifest.rs     # file list·hash·provenance

scripts/
├─ dev.ps1                          # 개발 환경 확인과 빠른 실행
├─ check.ps1                        # format·lint·type·contract
├─ test.ps1                         # 검증 단계 선택 wrapper
├─ package.ps1                      # xtask package 호출
├─ ci/
│  ├─ quick.ps1
│  ├─ full.ps1
│  ├─ docs.ps1
│  └─ release.ps1
└─ maintenance/
   ├─ verify-generated.ps1
   └─ verify-repository-policy.ps1

packaging/
└─ windows/
   ├─ README.md
   ├─ installer/                    # installer source, 기술은 후속 조사
   ├─ assets/                       # icon·license·설명
   ├─ manifests/                    # 설치·update·uninstall file ownership
   ├─ migrations/                   # 설치 layout migration
   ├─ plugin/                       # integrations/codex-plugin staging 규칙
   └─ tests/                        # clean install·upgrade·rollback·uninstall
```

`scripts/`에는 정책 계산, Schema 해석, 상태 migration 같은 제품 logic을 두지 않는다. 복잡한 검사는 typed `xtask` command로 올려 Windows와 CI에서 같은 code를 사용한다.

### Release 산출물

```text
dist/                               # 생성물, Git 제외
├─ star-control-plugin-<version>.zip
├─ star-control-windows-x64-<version>.<installer>
├─ star-control-windows-x64-<version>.zip
├─ star-control-windows-arm64-<version>.<installer>
├─ star-control-windows-arm64-<version>.zip
├─ checksums.sha256
├─ release-manifest.json
├─ sbom.spdx.json
└─ provenance.json
```

Codex Plugin과 Windows runtime은 같은 제품 version과 compatibility manifest를 사용하지만 source folder와 산출물은 분리한다. 사용자는 하나의 설치 흐름으로 경험하며, installer 기술과 Plugin 배포 방식은 구현 직전 공식 자료를 다시 확인해 확정한다.

## 사용자 예시 구조

```text
examples/
├─ project-config/
│  ├─ minimal.toml
│  ├─ safe-default.toml
│  └─ personal-auto.toml
├─ tasks/
│  ├─ rust.toml
│  ├─ node.toml
│  └─ mixed-workspace.toml
├─ external-tools/
│  ├─ fake-json-stdio.toml          # contract test용 fake EXE
│  ├─ ripgrep.toml                  # argv_v1 예시
│  └─ schemas/
│     └─ fake-result.schema.json
├─ contracts/
│  ├─ api.toml
│  ├─ cli.toml
│  └─ architecture.toml
├─ profiles/
│  ├─ custom-profile.toml
│  └─ profile-override.toml
├─ policies/
│  ├─ approval.toml
│  ├─ risk-paths.toml
│  └─ redaction.toml
└─ reports/
   ├─ review-pack.md
   └─ final-report.md
```

예시는 실제 Schema 검사를 통과해야 하며 문서에 같은 설정 전문을 복사하지 않는다.

## 최종 문서 구조

현재 `docs/`는 아래 책임별 구조로 전환을 마쳤다. 이후 문서 이동은 링크와 정본 소유권을 함께 갱신하는 별도 migration으로 처리한다.

```text
docs/
├─ README.md                         # 전체 문서 지도와 읽는 순서
├─ product/
│  ├─ vision.md                     # 제품 정의·사용자·지원 범위
│  ├─ user-flow.md                  # Codex 앱에서 시작하는 전체 경험
│  ├─ scope.md                      # 포함·제외 기능의 상위 경계
│  └─ glossary.md                   # 사용자 용어
├─ architecture/
│  ├─ system-overview.md            # process와 전체 component
│  ├─ repository-layout.md          # 이 문서의 최종 위치
│  ├─ dependency-rules.md           # Package 의존 허용·금지
│  ├─ process-and-ipc.md            # CLI·MCP·Controller·App Server
│  ├─ codex-integration.md          # Plugin·Skill·Hook·MCP·App Server
│  ├─ state-and-artifacts.md        # 저장 위치·journal·migration
│  ├─ security-and-permissions.md   # 승인·격리·secret
│  ├─ worktrees-and-merge.md        # 병렬 실행과 여러 repo
│  └─ windows-tool-runtime.md       # watcher·identity·process·격리 exact 계약
├─ contracts/
│  ├─ README.md                     # 계약 목록과 version 정책
│  ├─ development-management.md     # 0단계 관리 domain·DB·rebuild 정본
│  ├─ goal-and-stage.md
│  ├─ routing.md
│  ├─ config-and-catalog.md
│  ├─ validation-and-evidence.md
│  ├─ events-and-state.md
│  ├─ mcp-tools.md
│  ├─ external-tool-registry.md
│  ├─ mcp-implementation-contract.md
│  ├─ tool-package-manifest-reference.md
│  ├─ local-ipc.md
│  ├─ errors-and-diagnostics.md
│  └─ versioning-and-migrations.md
├─ features/
│  ├─ README.md                     # A01~D03 전체 대응
│  ├─ core-control.md               # A01~A10
│  ├─ validation.md                 # B01~B09
│  ├─ profiles.md                   # C01 공통 계약
│  └─ operations.md                 # D01~D03
├─ profiles/
│  ├─ README.md                     # Profile 문서 template
│  ├─ project-understanding.md
│  ├─ change-planning.md
│  ├─ refactor-codemod.md
│  ├─ dependency-upgrade.md
│  ├─ language-platform-migration.md
│  ├─ data-config-db-migration.md
│  ├─ api-contract-change.md
│  ├─ test-correctness.md
│  ├─ architecture-quality.md
│  ├─ debug-recovery.md
│  ├─ performance-build.md
│  ├─ docs-config-environment.md
│  ├─ ci-release-deploy.md
│  ├─ security-supply-chain.md
│  └─ ai-development-validation.md
├─ operations/
│  ├─ installation.md
│  ├─ first-run-and-doctor.md
│  ├─ controller-lifecycle.md
│  ├─ configuration.md
│  ├─ privacy-and-data.md
│  ├─ backup-export-recovery.md
│  ├─ update-and-rollback.md
│  ├─ uninstall.md
│  └─ troubleshooting.md
├─ testing/
│  └─ mcp-verification-matrix.md    # 실제 Codex same-session release gate
├─ development/
│  ├─ setup-windows.md
│  ├─ commands.md
│  ├─ testing-strategy.md
│  ├─ repository-policy.md
│  ├─ add-package.md
│  ├─ add-adapter.md
│  ├─ add-check.md
│  ├─ add-profile.md
│  ├─ contract-change.md
│  ├─ release-process.md
│  └─ compatibility-matrix.md
├─ decisions/
│  ├─ README.md                     # ADR index와 상태
│  ├─ ADR-0001-최종-설계-기준.md
│  ├─ ADR-0002-데이터-계약과-설정-정본.md
│  ├─ ADR-0003-외부-도구-레지스트리와-MCP-Gateway.md
│  ├─ ADR-0004-무재시작-고정-MCP와-Live-Tool-Registry.md
│  ├─ ADR-0005-MCP-구현-계약-동결.md
│  ├─ ADR-0006-공통-개발-관리와-로컬-관리-DB-경계.md
│  ├─ ADR-0007-P0-하이브리드-저장소와-운영-정책.md
│  └─ ADR-0008-P0-embedded-relational-backend.md
├─ research/
│  ├─ README.md                     # 참고 근거이며 설계 계약이 아님
│  ├─ codex-capabilities/
│  ├─ tool-selection/
│  ├─ windows-packaging/
│  └─ validation-methods/
├─ roadmap/
│  ├─ final-implementation.md
│  └─ release-readiness.md
├─ history/
│  ├─ legacy-feature-catalogue.md
│  └─ source-selection-record.md
├─ generated/
│  ├─ README.md                     # 직접 편집 금지
│  ├─ cli-reference.md
│  ├─ mcp-reference.md
│  ├─ config-reference.md
│  ├─ contract-index.md
│  └─ profile-reference.md
└─ assets/
   ├─ diagrams/
   └─ screenshots/
```

### 문서 종류별 정본 규칙

| 문서 위치 | 역할 | 정본 여부 |
|---|---|---|
| `product/` | 사용자 목적·범위·경험 | 상위 제품 정본 |
| `architecture/` | 책임·process·dependency 경계 | 구조 정본 |
| `contracts/` | 사람이 읽는 semantic 계약 | 계약 설명 정본 |
| `features/` | A01~D03 책임과 연결 | 기능 정본 |
| `profiles/` | 15개 작업 유형의 목적·적용 경계 | Profile 설명 정본 |
| `operations/` | 설치된 제품을 사용하는 절차 | 운영 정본 |
| `development/` | 저장소를 수정하는 절차 | 기여·검증 정본 |
| `decisions/` | 중요한 선택과 변경 이유 | 결정 기록 정본 |
| `research/` | 나중에 다시 확인할 조사 근거 | 비정본 참고 자료 |
| `history/` | 과거 사실과 출처 대응 | 현재 계약 아님 |
| `generated/` | code·catalog에서 생성한 reference | 직접 편집 금지 |

같은 규칙을 여러 문서에 전문으로 복사하지 않는다. 설명이 필요하면 정본 문서의 정확한 section으로 연결한다.

### 완료된 문서 이동 대응

| 현재 문서 | 최종 소유 위치 |
|---|---|
| `00_프로젝트_헌장.md` | `product/vision.md` |
| `01_사용자_경험과_전체_흐름.md` | `product/user-flow.md` |
| `02_전체_구조.md` | `architecture/system-overview.md` |
| `03_단계_분해와_실행_계약.md` | `contracts/goal-and-stage.md` |
| `04_모델_추론_실행모드_배정.md` | `contracts/routing.md` |
| `05_...`~`10_...` | `architecture/`, `contracts/`, `product/`의 책임별 문서 |
| `11_설정과_공개배포.md` | `contracts/config-and-catalog.md`와 `operations/installation.md` |
| `12_최종_구현_로드맵.md` | `roadmap/final-implementation.md` |
| `13_용어.md` | `product/glossary.md` |
| `14_레거시_기능_카탈로그.md` | `history/legacy-feature-catalogue.md` |
| `15_1인개발자_구현대상_기능.md` | `features/`와 `history/source-selection-record.md` |
| `16_최종_레포_패키지_문서_구조.md` | `architecture/repository-layout.md` |

기존 번호 파일은 제거했고 새 경로만 정본이다. 이후 문서 추가는 해당 책임 폴더에서 시작한다.

## 저장소 자체의 프로젝트 설정

```text
.star-control/
├─ project.toml                     # 공유 ProjectId와 source ownership
├─ config.toml                      # 이 repo의 승인·scan·병렬·보존 기본값
├─ tasks.toml                       # format·lint·build·test·docs·package
├─ contracts.toml                   # 보호할 공개·지속 계약
├─ risk-paths.toml                  # policy·schema·installer 등
├─ rules.toml                       # project Rule enable·parameter·override
├─ suppressions.toml                # review된 shared Suppression
├─ baselines/                       # versioned shared Baseline 선언
├─ change-recipes/                  # project shared ChangeRecipe
└─ profiles/
   └─ overrides.toml                # 이 repo에 필요한 Profile 보정
```

이 폴더에는 사용자 secret, 사용자 이름, 절대 경로, 개인 비용 한도와 local Disposition을 넣지 않는다. 개인 설정은 `%APPDATA%\Star-Control\config.toml`, local-only decision과 projection은 관리 DB에 둔다.

## Runtime과 생성 폴더

source tree와 runtime 상태를 섞지 않는다.

### 사용자 전체 상태

```text
%LOCALAPPDATA%\Star-Control\
├─ controller/
│  ├─ instance.json
│  ├─ health.json
│  └─ controller.lock
├─ management/
│  ├─ active-set.json               # global+project generation hash manifest
│  ├─ global/
│  │  ├─ active/                    # backend-neutral global generation
│  │  ├─ generations/
│  │  ├─ backups/
│  │  └─ recovery/
│  ├─ projects/
│  │  └─ <project-id>/
│  │     ├─ active/                 # 이 project의 opaque generation
│  │     ├─ generations/
│  │     ├─ backups/
│  │     └─ recovery/
│  └─ backup-sets/                  # 호환 generation vector manifest
├─ root-bindings/                   # current-user protected opaque root locator
├─ worktrees/
│  └─ <project-id>\<run-id>\<stage-id>\
├─ cache/                           # 다시 만들 수 있는 discovery cache
├─ logs/                            # 보존 정책과 redaction 적용
├─ updates/                         # update staging과 rollback metadata
└─ recovery/                        # DB 밖 제품 lifecycle 복구본
```

실제 DB filename, extension, connection string과 backend setting은 이 layout의 공개 계약이 아니다. directory name에는 ProjectId 외 project 이름·repository 이름·사용자 이름·source path를 넣지 않는다. 관리 DB에는 `root_binding_id`만 두고 raw project absolute path는 저장하지 않는다.

### 대상 프로젝트 증거

```text
<project>\.ai-runs\star-control\
├─ runs/
│  └─ <run-id>/
│     ├─ manifest.json
│     ├─ goal.json
│     ├─ plan.json
│     ├─ capability-snapshot.json
│     ├─ events.jsonl
│     ├─ stages/
│     │  └─ <stage-id>/
│     │     ├─ stage.json
│     │     ├─ route.json
│     │     ├─ context-summary.json
│     │     ├─ permission-plan.json
│     │     ├─ validation-plan.json
│     │     ├─ attempts/
│     │     ├─ checkpoint.json
│     │     └─ result.json
│     ├─ evidence/
│     │  ├─ changes.json
│     │  ├─ diagnostics.jsonl
│     │  ├─ validations.json
│     │  ├─ costs.json
│     │  ├─ risks.json
│     │  └─ provenance.json
│     ├─ review/
│     │  ├─ review-pack.md
│     │  └─ decision.json
│     ├─ merge/
│     │  ├─ plan.json
│     │  ├─ conflicts.json
│     │  └─ result.json
│     └─ reports/
│        ├─ final-report.md
│        └─ handoff.md
└─ management/
   ├─ scans/<scan-run-id>/
   ├─ patches/<patch-set-id>/
   └─ validations/<validation-result-id>/
```

`.ai-runs/`는 기본적으로 Git에서 제외한다. 사용자가 명시적으로 내보낸 redacted report만 별도 경로에서 commit할 수 있다.

### Source·생성물·로컬 자료 분류

| 분류 | 위치 | 처리 |
|---|---|---|
| 사람이 편집하는 source | `apps/`, `crates/`, `catalog/`, `docs/`의 비생성 문서 | review와 CI 대상 |
| 생성 후 commit하는 계약 | `specs/schemas/`, `docs/generated/` | generator로만 갱신, drift 검사 |
| test·평가 source | `tests/`, `corpus/`, `evals/` | version·출처·기대 결과 관리 |
| build·release 생성물 | `target/`, `dist/`, coverage | Git 제외, 다시 생성 |
| runtime 상태 | `%LOCALAPPDATA%`의 backend-neutral 관리 DB·root binding, 대상 repo `.ai-runs/` | source release와 분리 |
| local-only 과거 자료 | `legacy/` | 읽기 전용, 현재 설계와 package 입력 제외 |

## 23개 구현 기능의 소유 Package

각 기능은 기본 소유 Package를 하나만 가진다. 보조 Package는 외부 연결이나 공통 계약만 제공한다.

| ID | 기본 소유 위치 | 주요 보조 위치 |
|---|---|---|
| A01 목표·작업 계약 | `star-planning/task_contract` | `star-contracts`, `star-application` |
| A02 단계 계획·재계획 | `star-planning/decompose·stage_graph·replan` | `star-domain`, `star-application` |
| A03 프로젝트 이해·Context Pack | `star-project`의 revision·scan·source graph | `star-contracts`, `star-ports`, `catalog/profiles` |
| A04 변경 영향·위험 | `star-project/impact·risk_paths`와 `star-validation/findings` | `star-checks/change_scope`, `star-adapter-git` |
| A05 Codex 단계별 배정 | `star-routing` | `star-adapter-codex/capability`, `catalog/routing` |
| A06 Codex 실행·터미널 제어 | `star-application`과 `star-execution` | `star-config/registry`, `apps/`, `star-ipc`, Codex·Windows adapter |
| A07 상태·Checkpoint·자체 복구 | `star-state`의 management repository·migration·recovery | `star-execution/checkpoint·recovery`, `star-evidence` |
| A08 권한·승인·격리·secret | `star-policy` | `star-adapter-windows`, `star-evidence/redaction` |
| A09 Worktree·병렬·병합 | `star-vcs` | `star-execution/parallel`, `star-adapter-git` |
| A10 Task·Tool·Validation·Profile Registry | `star-config/registry`와 `catalog/`·`tools.d` | `star-mcp`, `star-validation/registry`, `star-ports` |
| B01 diff·범위·주장·증거 | `star-validation/findings·claim·gate·review_pack` | `star-application` change plan, `star-checks/change_scope`, `star-evidence` |
| B02 테스트 신뢰성 | `star-checks/test_trust` | `star-validation`, `corpus/test_trust` |
| B03 검증기 보호·Corpus | `star-checks/validator_guard` | `corpus/validator_guard`, `star-evaluation` |
| B04 계약·구조·설정·migration | `star-checks/contract_architecture` | `star-state`, `star-project`, `specs/`, `star-config` |
| B05 보안·dependency·공급망 | `star-checks/security_supply_chain` | `star-policy`, `star-evidence/redaction` |
| B06 실패 재현·대상 복구 | `star-checks/failure_recovery` | `star-project`, `star-evidence`, tool port |
| B07 문서·설정·개발 환경 | `star-checks/docs_environment` | `star-project/toolchain`, tool port |
| B08 성능·자원·build | `star-checks/performance_build` | `star-evidence/costs`, `star-evaluation` |
| B09 CI·release·배포 준비 | `star-checks/release_deploy` | `star-adapter-remote-git`, `packaging/`, `star-evidence` |
| C01 15개 개발 작업 Profile | `catalog/profiles` | `star-config`, `docs/profiles` |
| D01 여러 project·원격 Git·조사 | `star-application` | `star-vcs/multi_repo`, Codex·remote Git adapter |
| D02 비용·평가·규칙 개선 | `star-evaluation` | `evals/`, `star-evidence`, `star-routing/shadow` |
| D03 Windows 배포·제품 수명주기 | `packaging/windows` | `integrations/codex-plugin`, Windows adapter, state·config migration |

이 표에 없는 Package가 새 기능의 판단을 소유하면 구조 위반이다. 기능 소유권이 바뀌면 이 표와 관련 ADR을 함께 갱신한다.

## 주요 정본과 단일 Writer

| 정보 | 정본 | Writer |
|---|---|---|
| 직렬화 계약 | `star-contracts` | contract 변경 작업 |
| Project stable identity·shared Rule·Recipe·Suppression·Baseline | Git의 `.star-control`·Catalog | review된 source 변경 작업 |
| 생성 Schema | `specs/schemas` | `xtask schema`만 |
| 제품 기본 설정 | `catalog/defaults` | 설정 변경 작업 |
| 15개 Profile | `catalog/profiles` | Profile 변경 작업 |
| release Tool package 선언 | `catalog/tool-packages` | Registry 계약 변경 작업 |
| 사용자·프로젝트 외부 tool | 각 `tools.d/*.toml` | 사용자와 trusted project 설정 |
| 실행 시 live tool 목록 | ToolRegistrySnapshot | Controller `registry_runtime` 한 process만 |
| 정책·검사 metadata | `catalog/policies`, `catalog/validators` | 해당 정책·검사 변경 작업 |
| 실행 중 Goal·Stage 상태 | Controller user-data state | `star-controller` 한 process만 |
| Project directory·cross-project coordination | global management repository | Controller application transaction만 |
| ProjectRevision·Scan·Symbol·Finding projection | ProjectId별 management repository | `star-controller`가 주입한 `star-state` adapter 한 process만 |
| local Suppression·Disposition·ChangePlan | ProjectId별 management repository | Controller application transaction만 |
| DB backend·table·migration 구현 | `star-state` private adapter | 승인된 persistence 변경 작업 |
| 프로젝트 실행 증거 | `.ai-runs/star-control/<run-id>` | Controller의 state·evidence transaction |
| 배정·검증 평가 자료 | `evals` | 승인된 평가 갱신 작업 |
| 검증기 회귀 사례 | `corpus` | 규칙 변경 작업 |
| 사람이 읽는 현재 설계 | `docs`의 비생성 정본 | 설계 변경 작업 |
| CLI·MCP·Schema reference | `docs/generated` | `xtask docs`만 |

## 확장 절차

### 새 Rule 또는 ChangeRecipe 추가

1. stable ID, version, definition fingerprint와 소유 Catalog·project 선언 위치를 정한다.
2. Rule은 identity anchor·redaction parameter·Occurrence contract, Recipe는 precondition·path scope·idempotency·rollback·validation을 선언한다.
3. `star-contracts` valid·invalid·fingerprint golden과 해당 `corpus/` 사례를 추가한다.
4. source-derived 결과는 `star-project`·`star-validation` public contract로 만들고 DB query·table을 analyzer나 Recipe에 노출하지 않는다.
5. 큰 scan output과 patch는 ArtifactRef로 만들고 DB에는 요약·관계만 저장한다.
6. existing Finding identity, suppression·baseline stale 판정과 GateDecision 회귀를 검사한다.

Rule·Recipe 추가만으로 CLI·MCP handler, management repository port와 DB backend를 바꾸지 않는다. 새 persisted 의미가 필요할 때만 계약·migration 변경 절차로 승격한다.

### 새 외부 개발 도구 EXE 추가

1. 단순 CLI면 `argv_v1`, 복잡한 결과면 adapter EXE의 `star_json_stdio_v1`을 선택한다.
2. user 또는 project `tools.d/<package>.toml`에 update policy, executable identity, tool Schema, argument binding과 필요한 Permission ActionId set을 선언한다. project package는 `pinned_hash`만 사용한다.
3. 복잡한 input·output Schema는 manifest 옆 파일로 둔다.
4. `star tools validate`로 candidate package를 검사하고 safe_default user package 또는 project package면 `star tools trust`를 수행한다.
5. 저장 직후 `star tools status` 또는 `star_tool_registry_status`에서 새 revision과 진단을 확인한다.
6. 같은 Codex 작업에서 search→describe→지정 risk lane 호출과 timeout·error를 smoke test한다. MCP·Controller·Codex를 재시작하지 않는다.

path만 바꾸거나 `follow_path` EXE를 같은 path에서 교체할 때도 같은 절차를 쓴다. 이 절차는 `star-mcp`, Controller와 Rust Package 변경을 요구하지 않는다. 새 process protocol, 고정 MCP lane 또는 permission 의미가 필요할 때만 계약 변경 절차로 승격한다.

### 새 개발 작업 Profile 추가

1. 기존 A·B 기능 조합으로 표현 가능한지 먼저 확인한다.
2. `catalog/profiles/<id>.toml`에 trigger, 단계, Context, 검사와 증거 요구를 선언한다.
3. `docs/profiles/<id>.md`에 목적, 적용 경계와 하지 않는 일을 설명한다.
4. Profile Schema, 최소 example, 잘못된 example과 기존 Profile 회귀를 추가한다.
5. generated Profile reference를 갱신한다.

기존 engine으로 표현 가능하면 Rust code와 새 Package를 추가하지 않는다.

### 새 built-in 검사 추가

1. B01~B09 중 소유 기능군을 정한다.
2. `star-checks`의 해당 module에 공통 check 계약을 구현한다.
3. `catalog/validators/registry.toml`에 stable rule ID와 metadata를 등록한다.
4. positive, negative, edge, regression fixture를 `corpus/`에 추가한다.
5. diagnostic과 억제·만료·gate 동작을 검사한다.
6. 기능 문서와 생성 reference를 갱신한다.

검사는 raw shell을 직접 실행하지 않고 `ToolExecutorPort`를 사용한다.

### 새 프로젝트 도구 연결

1. 기존 구조화 command와 result parser 등록으로 충분한지 확인한다.
2. 충분하면 대상 프로젝트 `.star-control/tasks.toml` 또는 catalog metadata만 추가한다.
3. 별도 인증, protocol 또는 lifecycle이 있을 때만 새 port 능력을 검토한다.
4. 새 adapter가 필요하면 conformance fixture와 capability discovery를 함께 구현한다.
5. adapter에는 승인과 완료 판단을 넣지 않는다.

Compiler, LSP, test runner, scanner, debugger, profiler와 CI 도구마다 별도 Package를 만들지 않는다.

### 새 Codex 기능 대응

1. 공식 문서와 현재 App Server capability를 다시 확인한다.
2. protocol 차이는 `star-adapter-codex`에서 흡수한다.
3. core에는 generic `CapabilitySnapshot`과 route constraint만 전달한다.
4. 미지원·실험 기능에는 fallback과 명확한 readiness 상태를 둔다.
5. Plugin manifest나 Hook 경계가 바뀌면 설치·신뢰 문서와 E2E를 함께 갱신한다.

### 새 persisted 계약이나 설정 추가

1. `star-contracts` 또는 `star-config`의 유일한 정본 type을 변경한다.
2. 이전 version과의 읽기·쓰기 정책을 정한다.
3. migration, valid·invalid·old-version fixture를 추가한다.
4. Schema와 generated reference를 재생성한다.
5. App·IPC·state·report 소비자 compatibility를 검사한다.

### 새 Package를 분리할 조건

다음 조건 중 여러 개를 만족할 때만 module을 Package로 분리한다.

- 독립적인 외부 dependency가 있음
- 다른 Package와 다른 보안·권한 경계가 있음
- 3개 이상의 소비자가 안정된 공개 계약을 사용함
- 독립 conformance test가 필요함
- optional 배포 또는 feature 조합으로 분리할 가치가 있음
- 변경 빈도와 실패 범위가 기존 소유 Package와 명확히 다름

파일 수나 줄 수만으로 Package를 나누지 않는다.

## 유지보수 규칙

### Package README

각 Package에는 다음을 담은 짧은 `README.md`를 둔다.

- 한 문장 책임
- 소유하는 기능 ID
- 공개 API와 입력·결과
- 허용 dependency
- 하지 않는 일
- 주요 검사와 fixture
- contract 또는 migration 영향

### 공개 표면

- 기본은 private module이며 실제 Package 소비자에게 필요한 type만 공개한다.
- 다른 Package가 내부 폴더를 우회해 import하지 못하게 한다.
- 공개 type은 `star-contracts` type을 사용하고 편의 DTO를 새로 만들지 않는다.
- 오류는 stable category와 source를 함께 보존하며 문자열 parsing에 의존하지 않는다.

### Feature flag

- optional 외부 adapter와 고비용 개발 도구에만 사용한다.
- domain 동작을 flag 조합마다 다르게 만들지 않는다.
- 기본 build와 release build의 feature 조합을 manifest로 고정한다.
- CI에서 지원하는 모든 조합을 검사한다.

### Migration

- config, state, IPC, Plugin과 catalog version을 분리한다.
- reader는 지원 범위 안의 과거 version을 읽고, 알 수 없는 필드를 가능한 한 보존한다.
- migration은 dry-run, backup, 실행, 검증, rollback 단계로 나눈다.
- destructive migration은 승인 없이 실행하지 않는다.

### 의존성 관리

- workspace dependency version은 root `Cargo.toml`에서 한 번 관리한다.
- runtime dependency 추가는 목적, 대안, license, 보안과 binary 크기 영향을 기록한다.
- adapter 전용 dependency를 core Package로 끌어오지 않는다.
- `Cargo.lock`은 Windows release와 CI에서 고정한다.

### CI 단계

```text
quick
  -> format + compile + changed-package unit + contract drift

target
  -> affected package + integration + relevant corpus

full
  -> workspace lint + all tests + corpus + resilience + docs links

release
  -> clean Windows E2E + install/update/rollback/uninstall
     + package manifest + checksum + SBOM + provenance
```

`.github/workflows/`에는 `quick.yml`, `full.yml`, `docs.yml`, `security.yml`, `release.yml`만 두고 실제 command 선택은 `scripts/`와 `xtask`가 소유한다. CI YAML에 제품 logic을 복사하지 않는다.

### 문서 유지

- code·catalog에서 알 수 있는 목록은 생성 문서로 만든다.
- 이유, 경계, trade-off와 운영 절차만 사람이 직접 쓴다.
- 연구 자료에는 URL, 확인 날짜와 적용 판단을 남긴다.
- ADR은 결정을 대체하지 않고 현재 정본 문서로 연결한다.
- 문서 삭제나 이동 시 내부 링크 검사와 정본 중복 검사를 함께 실행한다.

## 구현 단계별 생성 순서

폴더를 한꺼번에 빈 껍데기로 만들지 않는다.

| Roadmap | 처음 생성할 구조 |
|---|---|
| D0 | 현재 설계 문서와 이 repository 구조 문서 |
| P0 | management contract type·fixture, `ManagementRepositorySet` port·fake conformance, 승인된 embedded relational adapter, global/project lifecycle·coordination, Project scan·Finding·Patch·Validation application slice |
| P1 | foundation 4개 Package, rmcp 2.2 고정 Gateway, authenticated IPC, live Registry·Win32 runtime, Schema·fixture와 MCP matrix 수직 slice |
| P2 | Controller·CLI app skeleton, core Tool package, `integrations/codex-plugin` |
| P3 | P0 `star-project`를 확장하고 `star-planning`, `star-routing`, `star-policy` 생성 |
| P4 | P0 `star-application`·`star-state`·`star-execution`을 Goal/Codex lifecycle로 확장하고 `star-evidence`, Codex adapter 생성 |
| P5 | P0 `star-validation`을 전체 검사 engine으로 확장하고 `star-checks`, `corpus/`와 관련 tests 생성 |
| P6 | `star-vcs`, Git adapter, worktree·merge tests |
| P7 | remote Git adapter, multi-project·remote integration tests |
| P8 | `star-evaluation`, `evals/`, shadow comparison |
| P9 | `packaging/windows`, release workflow와 최종 운영 문서 |

P0의 backend·dependency는 별도 승인 뒤에만 추가한다. 이미 구현된 P1 MCP 수직 Slice는 그 역사와 검증 상태를 유지하지만 P0 관리 계약이 구현됐다는 근거가 되지 않는다. 각 단계는 다음 단계가 실제로 필요로 하는 공개 계약까지만 먼저 만들고 미래 Package의 빈 폴더와 사용되지 않는 추상화는 만들지 않는다.

## 구조 검증 항목

다음 검사는 repository 정책으로 자동화한다.

- Cargo workspace의 Package가 이 문서의 허용 계층을 거스르지 않는지
- engine이 filesystem·process·network·Git을 직접 호출하지 않는지
- CLI와 MCP가 Controller 상태를 직접 쓰지 않는지
- CLI·MCP·Codex entry adapter가 management DB·ArtifactStore를 직접 열지 않는지
- CLI-only dependency graph와 E2E가 Codex·App Server·다른 AI·OpenAI API를 호출하지 않는지
- 모든 project-scoped DB relation이 ProjectId partition과 project-relative path를 사용하는지
- management store migration·backup·corruption·read-only recovery·rebuild fixture가 있는지
- DB backend 이름·SQL type이 public contract와 StarConfig에 노출되지 않는지
- `star-contracts` 밖에 중복 wire type이 없는지
- 23개 기능 ID와 15개 Profile에 소유 위치가 있는지
- Catalog 항목과 Profile 문서·generated reference가 일치하는지
- Schema와 generated 문서를 다시 생성했을 때 diff가 없는지
- 모든 migration version에 old-version fixture가 있는지
- 각 built-in check에 필요한 Corpus 종류가 있는지
- Plugin root 구조와 manifest 경로가 공식 형식에 맞는지
- release staging에 `legacy/`, `.ai-runs/`, secret, 사용자 절대 경로가 없는지
- 모든 Markdown 내부 연결과 package README 링크가 실제 파일을 가리키는지

## 최종 구조 완료 조건

1. `star`, Controller와 MCP가 같은 application use case와 상태를 사용한다.
2. 23개 기능마다 기본 소유 위치가 하나 있고 중복 engine이 없다.
3. 15개 작업 유형이 Catalog Profile만으로 공통 engine을 조합한다.
4. 외부 도구와 Codex 변경이 adapter 밖의 Package에 직접 번지지 않는다.
5. Controller가 상태의 단일 writer이며 crash 뒤 복구 가능한 transaction 경계를 가진다.
6. Git 정본, local management projection·operation state와 `.ai-runs` evidence의 경계가 구현과 문서에서 일치한다.
7. 계약·설정·Profile·정책·증거와 생성 문서의 정본이 각각 하나다.
8. built-in 검사군은 공통 diagnostic·gate·Review Pack 계약을 사용한다.
9. Package 의존 graph에 cycle과 모호한 공용 Package가 없다.
10. clean Windows에서 설치, 첫 실행, update, rollback, uninstall을 검사할 수 있다.
11. 현재 문서만으로 source, runtime 상태, 생성물과 local-only legacy 경계를 이해할 수 있다.

이 구조는 모든 최종 기능을 담는 목표 구조다. 구현 과정에서 세부 기술을 바꿀 수는 있지만 Package 책임, 정본 위치, 단일 Writer, adapter 경계와 기능 소유권을 바꾸려면 먼저 ADR과 이 문서를 갱신해야 한다.
