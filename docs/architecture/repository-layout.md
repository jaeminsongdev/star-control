# 최종 Repository·Package·문서 구조

## 문서의 역할

이 문서는 [구현 대상 기능](../features/README.md)의 A01~D03과 최종 16개 작업 Profile을 모두 구현했을 때 Star-Control 저장소가 가져야 할 최종 물리 구조와 책임 경계를 정한다.

문서 폴더 migration과 첫 MCP 수직 Slice의 `star-contracts`, `star-ipc`, `star-controller`, `star-mcp`, `star-cli`·검증 도구는 구현됐다. P0에서는 `star-domain`, `star-ports`, `star-project`, `star-validation`, `star-execution`, `star-application`, `star-state`, `star-evidence`의 최소 Package와 private persistence adapter를 만들었다. M1 Project Catalog·Code Index, M2 변경 계획·affected 선택, M3 공통 검증·품질 Gate, M4 안전한 Patch·Refactor·codemod 엔진, M5 관리형 Symbol Registry, M6 API·계약·문서·설정·개발 환경 관리, M7 실패 재현·보안·의존성 유지보수, M8 migration·performance·language/platform, 9단계 CrossRepo ChangeBundle, 10단계 CI·Release·평가·최종 제품 완성과 M11 Rust 코드 스타일 자동 교정의 상세 module·target 계약은 현재 **설계만 확정하는 범위**이며 제품 code·Schema·DB migration·validator·runner·rewrite·worktree·merge queue·ChangeBundle·remote writer·release/evaluation/Rust style engine·Registry/contract/release manifest·generator·codemod·doctor·clean-room runner·debugger·scanner·dependency updater·network client·Corpus는 아직 구현된 것으로 보지 않는다. generated 관리 Schema·fixture와 실제 완료 판정은 [최종 구현 로드맵](../roadmap/final-implementation.md)·`PLANS.md`를 따른다. 아래 큰 module tree 중 아직 구현하지 않은 module을 현재 존재하는 것으로 읽지 않는다.

P-0031의 예외 범위는 `.star-control/project.toml`, `star-contracts::evidence::ValidationPlan` v1, `star-validation::planning` pure policy와 Controller `validation_planning` adapter다. 후속 bounded 전환은 `validation_execution`과 프로젝트별 ignored derived cache를 연결했다. 이는 아래 full M2/M3 module tree, generic runner·authoritative Gate/evidence writer가 구현됐다는 뜻이 아니다.

[7단계 의미 정본](../contracts/failure-security-and-dependency-maintenance.md)은 이 문서의 M7 module·Schema·Catalog·evidence 위치가 구현해야 할 failure/security/dependency/Radar 계약을 소유한다.

[9단계 의미 정본](../contracts/cross-repo-change-bundle.md)은 `MultiProjectGoal`, project-local worktree/merge, 비원자적 participant coordination, remote snapshot/approval과 10단계 release handoff 계약을 소유한다.

[10단계 의미 정본](../contracts/ci-release-evaluation-and-product-completion.md)은 local quick·target·full·release 계층, build-once artifact 승격, ready·approved·published 상태, install lifecycle, EvaluationRun과 최종 소유권 감사를 소유한다.

[11단계 의미 정본](../features/rust-code-style-auto-fix.md)은 stable rustfmt·exact allowlisted Clippy, package/target/feature/cfg coverage, isolated PatchSet과 `personal_auto` exact policy approval을 소유한다.

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
- 구체적인 Rust library와 외부 도구는 각 구현 단계 직전에 다시 조사한다. Windows 설치 transport는 ADR-0012에 따라 Inno Setup 6으로 확정됐다.

언어 선택이 나중에 바뀌더라도 이 문서의 process 경계, Package 책임과 의존 방향은 유지한다.

## 구조 설계 원칙

1. 기능 하나마다 Package를 만들지 않는다. 독립적인 변경 이유와 외부 의존 경계가 있을 때만 Package를 나눈다.
2. CLI, MCP와 Controller에는 업무 판단을 넣지 않는다. 모든 진입점은 같은 application use case를 사용한다.
3. 파일, process, Git, Codex와 원격 서비스 접근은 port를 거쳐 adapter에서만 수행한다.
4. 계획, 배정, 권한, 검사와 병합 engine은 구체적인 외부 도구를 알지 않는다.
5. built-in 검사 9개는 하나의 Package 안에서 module로 나누고 공통 validation engine을 재사용한다.
6. 최종 16개 개발 작업은 별도 engine이 아니라 data-driven Profile로 유지한다.
7. 직렬화 계약, 상태 migration, 설정 병합과 증거 생성의 정본을 한 곳씩만 둔다.
8. 생성 파일과 사람이 편집하는 정본을 같은 폴더에 섞지 않는다.
9. 이름이 모호한 `common`, `shared`, `utils`, `misc` Package를 만들지 않는다.
10. local-only `legacy/`와 실행 산출물은 현재 설계와 release 입력에서 제외한다.

## 최종 상위 구조

```text
Star-Control/
├─ .github/                         # 공개 저장소 운영과 CI 정의
├─ .star-control/                   # Star-Control 자체를 dogfooding하는 프로젝트 설정
│  ├─ contracts.toml                # M6 public surface·baseline·docs·environment constraint Git 정본 목표
│  ├─ migrations.toml               # M8 target·version source·chain·invariant Git 정본 목표
│  ├─ performance.toml              # M8 explicit workload·metric·noise protocol Git 정본 목표
│  └─ managed-registry/             # M5 Git 정본 목표 위치; 이번 설계에서 실제 파일은 만들지 않음
│     ├─ manifest.toml              # explicit fragment·namespace·compatibility root
│     └─ declarations/<fragment>.toml # root가 explicit list로 가리키는 review 대상 fragment
├─ apps/                            # 사용자가 실행하는 4개 얇은 binary
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
├─ rust-toolchain.toml              # 최종 pinned build 구조 목표; 현재 실제 파일은 없음
├─ deny.toml                        # dependency·license 정책
├─ .editorconfig
├─ .gitattributes
└─ .gitignore
```

`target/`, `dist/`, coverage 결과, `.ai-runs/`, 임시 worktree와 사용자 secret은 이 source 구조에 포함하지 않는다.

최종 runtime code는 4개 실행 파일과 bounded 내부 Package 집합으로 구성한다. P0는 위 8개 책임 Package를 실제 workspace member로 사용하고 새 DB 전용 public Package를 만들지 않는다. `star-state`와 `star-evidence`는 `crates/infrastructure/`, project·validation·execution·application은 `crates/control/`, contract·domain·port는 `crates/foundation/`에 둔다. Package 수를 기능 수와 맞추지 않고, 9개 검증 기능은 `star-checks` module로, 최종 16개 작업 유형은 `catalog/profiles` data로 흡수한다.

## 실행 파일 구조

네 binary는 서로 다른 사용자·protocol 경계를 담당하지만 내부 판단을 중복 구현하지 않는다.

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
└─ star-updater/
   ├─ Cargo.toml
   └─ src/
      └─ main.rs                    # one-shot stage·apply·rollback·restart entry
```

### 실행 파일별 금지 책임

| 실행 파일 | 담당 | 넣지 않는 것 |
|---|---|---|
| `star.exe` | 사람이 쓰는 terminal 명령과 표시 | DB·artifact·상태 직접 접근, 계획·권한 판단, Git 직접 실행, Codex·App Server·AI·OpenAI API 호출 |
| `star-controller.exe` | 상태·관리 DB의 단일 writer와 전체 use case 조립 | 사용자 UI, 자체 AI 호출, HTTP API server |
| `star-mcp.exe` | 고정 MCP surface와 Controller IPC 변환 | TOML·Registry·tool별 handler·EXE path·parser, DB·artifact 직접 접근, 별도 상태·정책, Codex App Server 직접 제어 |
| `star-updater.exe` | update lease, stage/apply/rollback, Codex restart와 receipt | 상주 scheduler, 일반 사용자 CLI, 직접 채팅 주입, updater self-replacement |

CLI와 MCP는 모두 local IPC client다. Controller만 application use case를 실행하고 상태를 쓴다. 이 원칙으로 같은 명령이 진입점마다 다르게 동작하는 일을 막는다.

## Cargo Workspace Package 구조

### 1. Foundation Package

```text
crates/foundation/
├─ star-contracts/
│  └─ src/
│     ├─ ids.rs                     # Goal·Stage·Project·Checkout·Catalog/Index snapshot·Finding·Symbol ID
│     ├─ goal.rs                    # GoalSpec·TaskSpec
│     ├─ stage.rs                   # StageSpec·StageGraph·ScopeRevision
│     ├─ route.rs                   # model_role·reasoning_effort·stage_mode·execution_mode·CapabilitySnapshot
│     ├─ context.rs                 # ContextPack summary와 source reference
│     ├─ management.rs              # Project·Revision·WorkspaceSnapshot·ScanRun·StoreStatus
│     ├─ project_catalog.rs         # ProjectCheckout·ProjectCatalogSnapshot·discovery relation
│     ├─ code_index.rs              # CodeIndexSnapshot·partition·tier·freshness·query quality
│     ├─ managed_registry.rs        # M5 manifest·declaration·alias·binding·consumer·snapshot 계약
│     ├─ compatibility.rs           # M6 ProjectContractManifest·surface snapshot·report·consumer impact
│     ├─ documentation.rs           # M6 DocumentationSnapshot·config trace·assumption observation
│     ├─ environment.rs             # M6 EnvironmentSnapshot·DoctorReport·CleanRoomSpec·7단계 handoff
│     ├─ dependency.rs              # M7 Dependency/SupplyChain snapshot·update plan 계약
│     ├─ maintenance.rs             # M7 ExternalData·Radar snapshot 계약
│     ├─ migration.rs               # M8 Project migration manifest·plan·checkpoint·attempt·restore 계약
│     ├─ performance.rs             # M8 workload·raw run·comparison 계약
│     ├─ language_migration.rs      # M8 behavior·coexistence·equivalence·9단계 handoff 계약
│     ├─ change_bundle.rs           # 9단계 MultiProjectGoal·CrossRepoChangeBundle·participant·release handoff
│     ├─ source_graph.rs            # CanonicalSource·Symbol·SymbolReference
│     ├─ finding.rs                 # Rule·Finding·Occurrence·Suppression·Baseline·Disposition
│     ├─ impact.rs                  # ImpactAnalysis·ImpactEdge·RiskPathFinding
│     ├─ change.rs                  # ChangePlan v1/v2·ChangeRecipe v1/v2·PatchSet v1/v2·TargetSelector·RecipeExecution·PatchApplication
│     ├─ permission.rs              # PermissionPlan·ApprovalRequest
│     ├─ validation.rs              # ChangeSet·ValidationPlan·ValidationRun·subject binding·affected record
│     ├─ diagnostic.rs              # RuleRef·공통 Diagnostic·remediation
│     ├─ evidence.rs                # Claim/Evaluation·RunSatisfaction·GateDecision·EvidenceBundle·ReviewPack
│     ├─ rust_style.rs              # M11 4개 nested binding/policy/coverage/step type; 별도 top-level run 없음
│     ├─ checkpoint.rs              # Checkpoint·handoff
│     ├─ merge.rs                   # project-local MergePlan v2·queue·conflict·ProjectMergeResult
│     ├─ remote.rs                  # RemoteStateSnapshot v2·RemoteOperationRecord
│     ├─ cost.rs                    # usage·time·rework metric
│     ├─ recovery.rs                # M7 FailureRecord·ReproductionPack·RegressionRecord·RecoveryPlan
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
│     ├─ environment_probe.rs       # 등록된 read-only OS·filesystem·toolchain observation만
│     ├─ migration_target.rs        # opaque target snapshot/copy/activate/restore capability
│     ├─ measurement.rs             # monotonic/resource collector 경계; profiler/analyzer는 Tool adapter
│     ├─ rewrite_transformer.rs     # bounded preview transform, concrete parser/tool 비노출
│     ├─ source_mutation.rs         # exact operation·receipt·reverse precondition
│     ├─ worktree.rs                # opaque project-local create/inspect/retain/discard·ownership port
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
│     ├─ task_contract/             # 사용자 TaskSpec 정규화·revision
│     ├─ scope_revision/            # requested·analysis·change·validation scope와 user decision
│     ├─ seed/                      # TaskSpec·ChangeSet에서 typed impact seed 생성
│     ├─ impact/                    # direct/transitive·confirmed/possible graph traversal
│     ├─ risk_paths/                # RiskPathDescriptor 평가와 confidence·limitation
│     ├─ change_plan/               # ImpactAnalysis에서 source-write 없는 ChangePlan v2 draft
│     ├─ decompose/                 # A02 성격 기반 단계 분해
│     ├─ stage_graph/               # 의존·병렬 관계
│     ├─ replan/                    # 새 사실과 범위 변화
│     └─ completion/                # 단계·목표 완료 조건
├─ star-project/
│  └─ src/
│     ├─ discovery/                 # multi-root·nested repo·workspace·worktree·non-Git 후보
│     ├─ identity/                  # stable Project와 local checkout/worktree identity
│     ├─ inventory/                 # bounded source enumeration·hash·ignore provenance
│     ├─ classify/                  # primary class·facet·conflict와 generated ownership
│     ├─ toolchain/                 # 언어·build·package manager 발견
│     ├─ rust_workspace/            # M11 Cargo package·target·feature·required-feature inventory
│     ├─ rust_style_config/         # toolchain/rustfmt/Clippy config source·edition discovery; source read-only
│     ├─ dependency_inventory/      # manifest·lockfile·direct/transitive/internal relation 관찰
│     ├─ contract_surface/          # API·CLI·Schema·format·config·error current 관찰
│     ├─ documentation/             # docs entry·generated provenance·assumption inventory
│     ├─ environment/               # redacted Windows·toolchain·manifest·lockfile 관찰
│     ├─ guidance/                  # AGENTS·README·정본 우선순위
│     ├─ text_index/                # 항상 가능한 exact text·token index
│     ├─ syntax_index/              # language adapter의 definition·reference 후보
│     ├─ semantic_index/            # 지원 환경에서만 symbol resolution; fallback 필수
│     ├─ graph/                     # project·package·contract·dependency graph; M7 입력만 제공
│     ├─ hardcoding/                # 근거 있는 candidate 생성, 결함 확정은 하지 않음
│     ├─ managed_registry/          # Git manifest resolve와 managed/candidate/local 분류
│     ├─ registry_binding/          # definition·reference·Schema·docs·generated/consumer 관찰
│     ├─ context/                   # A03 Context Pack 선택
│     ├─ freshness/                 # partition probe·stale·partial·unverified
│     ├─ revision/                  # ProjectRevision·WorkspaceSnapshot
│     ├─ scan/                      # full/incremental plan·partition generation
│     ├─ source_graph/              # CanonicalSource·Symbol·Reference
│     └─ cache_key/                 # backend-neutral cache key·reuse eligibility
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
│     ├─ cancellation/              # pause·cancel·interrupt
│     ├─ recipe/                    # M4 Recipe resolve·input·selector binding orchestration
│     ├─ preview/                   # target-effect 없는 materialized/isolated preview와 PatchSet v2
│     ├─ idempotence/               # expected-after replay·already-satisfied 판정
│     ├─ patch_apply/               # permit 소비·operation journal·receipt state machine
│     ├─ patch_recovery/            # actual reconciliation·reverse/discard plan
│     ├─ change_bundle/             # 9단계 participant DAG·state·partial/hold/resume coordinator
│     ├─ merge_queue/               # repository별 serial integration·stale/conflict lifecycle
│     ├─ resource_budget/           # project/worktree/process/check/disk/memory/time reservation
│     ├─ remote_operation/          # approval-bound push·PR·merge·after-snapshot reconciliation
│     ├─ reproduction/              # M7 bounded rerun·reduce·bisect adapter orchestration
│     ├─ dependency_update/         # M7 승인된 isolated package-manager preview·PatchSet orchestration
│     ├─ migration/                 # M8 dry-run·backup·rehearsal·checkpoint·resume·rollback orchestration
│     ├─ measurement/               # M8 explicit workload cohort·warmup·sample orchestration
│     ├─ language_cutover/          # M8 boundary·consumer phase·cutover/rollback orchestration
│     └─ rust_style_process/        # M11 registered cargo/rustfmt/Clippy typed step execution; policy 판단 없음
├─ star-validation/
│  └─ src/
│     ├─ change_set/                # snapshot delta를 TaskSpec·ScopeRevision에 bind한 actual comparison
│     ├─ plan/                      # 검사 계획과 단계
│     ├─ selector/                  # ImpactAnalysis·descriptor 기반 affected 검사 선택
│     ├─ fallback/                  # package→workspace→project full promotion
│     ├─ previous_success/          # 이전 pass와 current dirty delta compatibility
│     ├─ registry/                  # check descriptor 발견
│     ├─ preflight/                 # M2 plan coherence·current Registry·permission 확인
│     ├─ binding/                   # source·plan·config·Catalog·Tool evidence identity와 stale
│     ├─ runner/                    # tool 실행과 timeout
│     ├─ normalize/                 # 공통 diagnostic 변환
│     ├─ claim/                     # 완료 주장과 actual ChangeSet·evidence 대조
│     ├─ aggregate/                 # attempt·completeness·freshness·flaky result
│     ├─ suppression/               # 이유·fingerprint·만료
│     ├─ findings/                  # Rule 결과·Occurrence·Finding projection
│     ├─ baseline/                  # new·existing·worsened·improved 비교
│     ├─ ratchet/                   # raw outcome과 Gate satisfaction 분리
│     ├─ stability/                 # flaky detection, attempt history
│     ├─ disposition/               # local triage와 stale 판정
│     ├─ gate/                      # AUTO_PASS·HUMAN_REVIEW·BLOCK
│     ├─ patch_gate/                # 4단계 patch pre/post binding·invalidation
│     ├─ registry_consistency/      # M5 lifecycle·binding·consumer·M6 drift record
│     ├─ contract_evidence/         # M6 report·docs·doctor exact subject binding·stale 판정
│     ├─ maintenance_evidence/      # M7 failure/security/dependency exact binding·freshness
│     ├─ maintenance_radar/         # 공통 Finding/Suppression/snapshot의 결정적 derived ordering
│     ├─ migration_evidence/        # M8 chain·backup/restore·checkpoint·invariant·partial/rollback 판정
│     ├─ performance_evidence/      # M8 comparability·noise·outlier·metric·trade-off 판정
│     ├─ equivalence_evidence/      # M8 behavior dimension·platform·consumer·cutover 판정
│     ├─ change_bundle_gate/        # 9단계 prepare/goal-exit project binding·partial/remote 집계
│     └─ rust_style/                # M11 Diagnostic·coverage·hunk·side-effect·idempotence·Gate input
├─ star-checks/
│  └─ src/
│     ├─ change_scope/              # B01 실제 ChangeSet과 accepted scope·plan 일치 검사
│     ├─ test_trust/                # B02 테스트 약화·회귀 증거
│     ├─ validator_guard/           # B03 검증기 자기보호
│     ├─ contract_architecture/     # B04 baseline diff·consumer migration·공개 경계·generated drift
│     ├─ security_supply_chain/     # B05 secret·workflow·release·외부 scanner/freshness 정규화
│     ├─ failure_recovery/          # B06 family/occurrence·causality·before/after·재현·복구
│     ├─ docs_environment/          # B07 docs/config/assumption·read-only doctor·clean-room readiness
│     ├─ performance_build/         # B08 성능·자원·build
│     └─ release_deploy/            # B09 CI·release·배포 준비
├─ [infrastructure] star-state/
│  └─ src/
│     ├─ layout/                    # 사용자·프로젝트 상태 위치
│     ├─ repository/                # global/project ManagementRepositorySet과 backend adapter
│     ├─ transaction/               # store-local event·projection·idempotency·revision
│     ├─ coordination/              # cross-store operation·participant receipt·recovery
│     ├─ scan_generation/           # invisible batch와 atomic visible publish
│     ├─ index_cache/               # 재생성 가능한 content-addressed cache adapter
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
│     ├─ reproduction/              # 일반 log와 분리한 curated ReproductionPack manifest
│     ├─ supply_chain/              # redacted scanner·workflow·release·external source evidence
│     ├─ dependencies/              # dependency snapshot·PatchSet·before lockfile evidence
│     ├─ maintenance/               # deterministic Radar snapshot·default-safe render
│     ├─ migrations/                # plan·attempt·checkpoint·restore/invariant evidence export
│     ├─ performance/               # raw sample·profile·build analyzer·comparison export
│     ├─ language_migrations/       # behavior baseline·equivalence·cutover/rollback export
│     ├─ change_bundles/            # participant·worktree·merge/conflict·release handoff export
│     ├─ remote/                    # redacted adapter snapshot·operation receipt export
│     ├─ rust_style/                # M11 toolchain/policy/coverage/step·Diagnostic/diff EvidenceRefSet
│     ├─ costs/                     # 시간·사용량·재작업
│     ├─ risks/                     # 남은 위험과 미확인
│     ├─ bundle/                    # GateDecision을 참조하는 EvidenceBundle 조립·hash
│     ├─ review_pack/               # bundle 기반 구조화 ReviewPack·render
│     ├─ rework/                    # blocking 근거 기반 ReworkDirective
│     ├─ report/                    # 최종 보고
│     ├─ export/                    # 진단 Pack·공개 보고서
│     └─ redaction/                 # 저장·출력 전 가림
├─ star-vcs/
│  └─ src/
│     ├─ baseline/                  # 시작 revision과 dirty state
│     ├─ overlap/                   # file·rename·range·symbol·contract·generated·lockfile 겹침
│     ├─ worktree/                  # M4 decision을 9단계 role·ownership·budget으로 확장
│     ├─ local_review/              # 로컬 검토 요청 정보
│     ├─ merge_queue/               # repository별 직렬 의존 순서와 stale 상태
│     ├─ conflict/                  # 양쪽 intent·contract·resolution PatchSet
│     ├─ remote_state/              # adapter-bound branch·PR·check·release snapshot
│     ├─ remote_operation/          # approval·idempotency·after-snapshot effect record
│     ├─ multi_repo/                # MultiProjectGoal·provider/consumer step DAG
│     └─ release_handoff/           # project별 revision·artifact·Gate 10단계 입력
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
      │  ├─ plan_registry_change.rs # DB 직접 write 없이 ChangePlan·PatchSet 경로만 조정
      │  ├─ inspect_failures.rs     # M7 failure/reproduction/recovery application use case
      │  ├─ inspect_supply_chain.rs # M7 offline/current security·freshness use case
      │  ├─ plan_dependency_update.rs # M7 candidate→isolated PatchSet·approval 대기
      │  ├─ build_maintenance_radar.rs # M7 결정적 derived view
      │  ├─ plan_migration.rs       # M8 version/chain/dry-run plan과 9단계 handoff
      │  ├─ run_migration_phase.rs  # M8 approval-gated backup/rehearsal/execute/resume/rollback
      │  ├─ compare_performance.rs  # M8 explicit workload cohort·comparison
      │  ├─ plan_language_migration.rs # M8 behavior/coexistence/equivalence/cutover plan
      │  ├─ prepare_change_bundle.rs # 9단계 current participant·DAG·overlap·budget plan
      │  ├─ run_change_bundle_participant.rs # project-local apply·validate·merge
      │  ├─ recover_change_bundle.rs # hold·resume·roll-forward·compensation
      │  ├─ operate_remote.rs       # action별 승인된 push·PR·merge와 reconcile
      │  ├─ create_release_handoff.rs # 10단계 project revision·artifact input
      │  ├─ rust_style.rs           # M11 inspect/check/prepare/auto-apply fixed workflow command
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
      ├─ queries/                   # project·scan·finding·change·store·Registry derived snapshot 조회
      ├─ coordinator/               # Package 사이 workflow 조정
      ├─ transaction/               # 상태·event·artifact commit 경계
      └─ service.rs                 # Controller가 호출하는 단일 façade
```

`star-checks` 안의 9개 module은 서로의 내부 구현을 import하지 않는다. 공통 동작은 `star-validation`의 공개 계약을 사용한다. 특정 검사군이 독립 dependency와 별도 release 주기를 가질 정도로 커졌을 때만 별도 Package로 분리한다.

M7은 debugger·scanner·package manager·advisory provider별 Package나 DB를 추가하지 않는다. 등록 도구 실행은 기존 `ToolExecutorPort`와 ToolDescriptor를 사용하고, ecosystem/source별 차이는 `ExternalDataSourceDescriptor`·`PackageManagerAdapterDescriptor`와 normalizer module에 둔다. core에 network client, dependency resolver, debugger protocol과 vulnerability DB를 넣지 않는다.

M8도 migration framework·DB engine·profiler·build analyzer·compiler·language별 Package를 추가하지 않는다. version/chain/state/comparability/equivalence는 기존 bounded Package의 module이 소유하고 실제 target effect·measurement·compiler output은 registered ToolDescriptor와 narrow port adapter로 수집한다. Star-Control 자체 management store migration은 `star-state/migration`에 남고 범용 Project migration은 `star-execution/migration`이 조정한다.

M11도 formatter·Rust parser·AST engine·LSP·Clippy 구현이나 `star-rust-style.exe`를 추가하지 않는다. Cargo discovery는 `star-project`, fixed workflow는 `star-application`, typed child process는 `star-execution`, Diagnostic/coverage/Gate 판정은 `star-validation`, projection/evidence는 기존 `star-state`·`star-evidence` module에 둔다. concrete cargo/rustfmt/Clippy 차이는 registered ToolDescriptor와 bounded adapter에만 있고 DB·Catalog가 Rust source/config 정본을 복제하지 않는다.

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
│     ├─ filesystem/                # Windows path·ACL·per-path atomic replace·exact mutation receipt
│     ├─ watcher/                   # Tool Registry 변화 감지; Project scan watcher는 M1 첫 Slice 제외
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
│     ├─ repository/                # top-level·git-dir·common-dir·object format 관찰
│     ├─ status_diff/               # porcelain v2 -z, staged·unstaged·untracked 관찰
│     ├─ worktree/                  # identity 관찰 + exact-base create/inspect/retain/discard·ownership
│     ├─ branch_commit/             # exact parent·ref precondition과 local receipt
│     ├─ merge_conflict/            # project-local merge/result·conflict observation
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
- P0·1단계의 `star-project`는 read-only project filesystem·Git 관찰과 Catalog·Index 의미 계산, `star-execution`은 exact-hash local patch effect를 소유하는 명시적 boundary Package다. `star-project`는 `source_effect=none`인 port만 사용하며 두 Package 모두 management DB, network, AI client와 사용자 root locator 저장소를 알 수 없다.
- 2단계 `star-planning`은 immutable TaskSpec·ScopeRevision·ChangeSet·Index graph value만 받는 pure engine이다. `star-project`, filesystem·Git·DB·process port를 직접 호출하지 않고 `star-application`이 current query 결과를 주입한다.
- `star-project/revision`은 source-derived Revision·WorkspaceSnapshot과 delta fact만 관찰하고, `star-validation/change_set`이 이를 TaskSpec·ScopeRevision에 bind한다. 어느 쪽도 ImpactEdge·risk 결과를 ChangeSet에 되써서 input/output cycle을 만들지 않는다.
- `star-validation/selector`는 ImpactAnalysis와 CatalogSnapshot을 ValidationPlan으로 바꾸고 실행은 runner에 맡긴다. planning과 selector가 서로의 내부 module을 import하지 않고 `star-contracts` document로 연결된다.
- M3 `star-validation/preflight·binding·aggregate·ratchet·gate`는 immutable contract value만 계산한다. filesystem·Git·process·DB·AI handle을 받지 않고 current probe·Tool 실행·commit 순서는 `star-application`이 port 결과로 주입한다.
- `star-validation/runner`는 M2 CheckGraph·TaskInvocation을 소비할 뿐 Check family·scope를 다시 선택하지 않는다. process I/O는 `ToolExecutorPort`, raw byte는 ArtifactStore를 사용한다.
- `star-checks`는 공통 check input에서 Diagnostic을 생산하고 서로의 내부 module, management repository와 external scanner client를 직접 import하지 않는다. 외부 도구는 CheckDescriptor·ToolDescriptor와 port를 통해서만 연결한다.
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
- `star-project`가 TaskSpec별 impact certainty·risk severity·affected Check를 결정하거나 `star-planning`이 source graph를 직접 재수집하는 의존
- `star-checks/change_scope`가 초기 ImpactAnalysis·ValidationPlan을 생성하거나 automatic planned change scope를 확대하는 동작
- `star-validation/runner`가 M2 selected Check·scope·fallback을 조용히 축소·확대하거나 raw shell command를 합성하는 동작
- changed validator가 current self-test 하나만으로 자기 Rule·severity·allowlist·fixture 변경을 승인하는 동작
- adapter에서 승인·위험·완료 여부 판단
- CLI·MCP handler에서 상태 파일 직접 읽기·쓰기
- CLI·MCP·향후 Codex entry adapter에서 management DB handle·SQL·ArtifactStore 직접 사용
- 한 Package가 다른 Package의 database·폴더 배치를 알고 접근
- `star-contracts` 밖에서 같은 직렬화 type 재정의
- module 사이 순환 의존과 feature flag로 숨긴 순환 구조
- 공용 편의를 이유로 모든 Package가 의존하는 비대한 helper Package 생성

CI는 workspace dependency graph와 금지 import를 검사해 이 규칙을 기계적으로 지킨다.

## 0·1·2·3·4·5단계 소유권과 repository abstraction

[공통 개발 관리 계약](../contracts/development-management.md), [Project Catalog·Code Index](../contracts/project-catalog-and-code-index.md), [변경 계획·영향 분석](../contracts/change-planning-and-impact.md)과 [Managed Registry](../contracts/managed-symbol-registry.md)는 새 storage·runner Package를 만들지 않고 기존 책임에 다음처럼 배치한다.

| 계약·행동 | 의미 소유 | persistence·I/O | 진입·조정 |
|---|---|---|---|
| Project stable identity·Revision·WorkspaceSnapshot·CanonicalSource | `star-contracts`, `star-domain`, `star-project` | `star-project` read-only filesystem·Git observer, project repository | `star-application` |
| ProjectCheckout·ProjectCatalogSnapshot | `star-contracts`, `star-domain`, `star-project` | protected root-binding port, `star-state` global projection | `star-application` |
| ScanRun·CodeIndexSnapshot·partition·freshness | `star-contracts`, `star-project` | `star-state` project scan generation·cache adapter | `star-application` |
| ManagedRegistryManifest·Fragment·Declaration·lifecycle | `star-contracts`; 의미 정본은 Project Git manifest | `star-project` read-only source resolver, M4 SourceMutationPort | `star-application` |
| ManagedRegistrySnapshot·binding·consumer·RegistryConsistencyRecord | `star-project`, `star-validation` | `star-state` derived project projection, `star-evidence` | `star-application` |
| package·module·symbol·contract·dependency graph와 Symbol·Reference | `star-project` | `star-state` project scan generation | `star-application` |
| TaskSpec·ScopeRevision·ImpactAnalysis와 task-specific risk path | `star-contracts`, `star-planning` | `star-state` local operational projection, `star-evidence` trace | `star-application` |
| ChangeSet·affected Check selection·ValidationPlan | `star-validation` | `star-state`, `star-evidence` | `star-application` |
| Rule·Finding·Occurrence·Suppression·Baseline·Disposition | `star-validation` | `star-state` projection, `star-evidence` artifact | `star-application` |
| ChangeRecipe·ChangePlan·PatchSet·RecipeExecution·PatchApplication | `star-contracts`, `star-execution` | `star-state`, `star-evidence`, SourceMutationPort·WorktreePort·ToolExecutorPort adapter | `star-application` |
| ValidationRun·Result·EvidenceSubjectBinding·Claim evaluation | `star-validation` | `star-state`, `star-evidence`, ToolExecutorPort | `star-application` |
| Diagnostic·Baseline/Suppression evaluation·RunSatisfaction·GateDecision | `star-validation` | `star-state`, `star-evidence` | `star-application` |
| ArtifactRef·EvidenceBundle·ReviewPack·ReworkDirective | `star-evidence` | ArtifactStore와 project evidence index | `star-application` |
| global Project directory·cross-project relation·coordination | `star-domain`, `star-application` | `star-state` global repository | `star-application` |
| DB version·migration·backup·integrity·rebuild·retention | `star-state` | global/project private backend adapter | Controller lifecycle·application command |

### 1단계 index adapter 경계

- `star-contracts`는 adapter 이름이 아니라 tier·partition·quality·limitation wire type만 소유한다.
- `star-project`는 내부 `LanguageIndexAdapter`·`BuildMetadataAdapter` trait, capability 선택, input/output fingerprint, batch 정렬, fallback과 nondeterminism 검사를 소유한다.
- 첫 M1 Slice의 concrete language adapter는 `star-project` 안의 private in-process module이다. bounded source byte와 manifest metadata만 받고 filesystem·process·network·DB handle을 받지 않는다.
- adapter descriptor는 stable adapter ID·version, 지원 language/mode·tier, deterministic 여부, 최대 input, 생성 entity/edge 종류와 limitation code set을 선언한다. 이 descriptor fingerprint가 CodeIndexSnapshot input에 들어간다.
- parser library type·AST·error와 library 이름은 `star-project` 밖 public contract, StarConfig, DB row와 CLI/MCP DTO에 노출하지 않는다. typed IndexBatch로 즉시 정규화한다.
- external process, language-server protocol 또는 compiler service adapter는 첫 Slice에 없다. 필요성이 corpus로 확인되고 dependency·lifecycle·license·offline·Windows 검증을 거친 뒤에만 `crates/adapters/`의 새 bounded adapter 분리를 별도 구조 결정으로 검토한다.
- Git·filesystem identity는 기존 `star-adapter-git`·`star-adapter-windows`가 read-only port로 제공하고, cache byte I/O는 `star-state/index_cache`가 맡는다. 어느 adapter도 current·gate·Finding assessment를 결정하지 않는다.

`star-ports::ManagementRepositorySet`은 `GlobalManagementRepository`, ProjectId별 `ProjectManagementRepository`, lifecycle, coordination, artifact와 root-binding port를 조립한다. 각 repository는 store-local transaction, project/checkout/source, scan/index generation, decision, change, event, cursor query와 retention operation만 노출한다. cache port는 snapshot·partition·adapter fingerprint 기반 opaque byte와 hit metadata만 다루며 current 판정을 소유하지 않는다. SQL row·table·connection·pragma·backend 오류는 port 밖으로 나오지 않는다. public input·result는 `star-contracts` type과 stable repository error category만 사용한다.

Controller의 single-store transaction 순서는 `artifact finalize -> repository begin -> expected revision·idempotency 검증 -> event+projection+store revision commit -> evidence export`다. DB commit에 실패한 artifact는 orphan으로 격리하고 성공 evidence로 노출하지 않는다. cross-store 작업은 `global prepared -> project participant transaction+receipt -> global completed` 순서이며 partial 상태를 ACID 성공으로 숨기지 않는다.

CLI-only composition은 read-only filesystem·Git·metadata tool runner, repository·cache·artifact port만 조립한다. 1단계의 discover·scan·index·query command graph에는 `star-execution`, source-write filesystem port, `star-adapter-codex`, App Server, 다른 AI provider와 OpenAI API client를 생성하거나 lazy-load하지 않는다. 향후 Codex 연동은 같은 `ManagementApplicationService` command를 호출하는 별도 entry adapter이며 별도 writer나 별도 engine을 만들지 않는다.

2단계 CLI-only composition은 위 read-only query graph에 `star-planning` pure engine과 `star-validation/selector`만 추가한다. test runner·ToolExecutor, `star-execution`, Codex adapter, source-write port와 cross-repo VCS adapter를 조립하지 않는다. local operational document commit은 Controller application transaction이 수행한다.

3단계 CLI-only composition은 M2 ready ValidationPlan consumer, `star-validation` preflight·runner·normalize·ratchet·gate, `star-checks`와 trusted ToolExecutorPort를 추가한다. 선택된 project Check가 선언한 build output 같은 effect는 PermissionPlan 안에서만 허용하고 source·Git·external effect는 descriptor에 없으면 거부한다. `star-adapter-codex`, App Server, 다른 AI provider와 OpenAI API client는 조립·lazy-load하지 않으며 의미 검토는 GateDecision `human_review`로 남긴다.

4단계 CLI-only composition은 M1 target resolver, M2 plan/reconciliation, M3 pre/post Gate에 `star-execution` Recipe preview·idempotence·PatchApplication, `star-vcs` single-project WorktreeDecision과 RewriteTransformerPort·SourceMutationPort·WorktreePort를 추가한다. external mutating codemod는 ToolExecutorPort를 통해 isolated preview worktree에서만 실행하고 live target write path를 받지 않는다. source mutation은 single-use permit 뒤 internal PatchSet apply만 수행한다. Codex·App Server·다른 AI·OpenAI API dependency는 여전히 0이고 cross-project write·merge port는 조립하지 않는다.

5단계 CLI-only composition은 `star-project/managed_registry·registry_binding`의 read-only resolve와 `star-validation/registry_consistency`를 M1 query에 추가한다. 변경 명령은 별도 DB writer를 만들지 않고 M2 `ChangePlan`, M4 Recipe preview·single-project PatchSet과 M3 pre/post Gate를 그대로 조립한다. source manifest와 DB Index가 다르면 source를 우선하고 projection을 stale로 처리한다. generated output은 declared generator를 통한 PatchSet operation만 허용하고 직접 편집을 거부한다. cross-project consumer는 read-only 영향으로만 반환하며 9단계 전 multi-project SourceMutationPort·merge port를 조립하지 않는다.

## Codex Integration 구조

```text
integrations/
└─ codex-plugin-template/
   └─ marketplace-root/
      └─ .agents/plugins/
         ├─ marketplace.json        # Star-Control 소유 로컬 Marketplace 정본
         └─ plugins/star-control/
            ├─ .codex-plugin/
            │  └─ plugin.json       # 필수 Plugin manifest
            ├─ skills/
            │  └─ star-control-operations/
            │     └─ SKILL.md        # ready action 실행과 native fallback 절차
            ├─ hooks/
            │  └─ hooks.json         # SessionStart와 star hook 명령 연결
            └─ .mcp.json             # 설치 때 실제 star-mcp 절대 경로로 렌더링
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
- 정적 template에는 PC별 절대 경로를 두지 않는다. `star-adapter-codex`가 검증된 installation record에서 실제 경로를 렌더링하고 Codex 공식 Plugin 명령으로 등록한다.
- Codex cache·`config.toml`·Hook trust 저장소는 직접 수정하지 않는다. 등록·수리·제거와 수동 조치 상태는 [Windows 설치와 Codex 연동 계약](../contracts/windows-installation-and-codex-integration.md)을 따른다.

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
│  ├─ ai_development_validation.toml
│  └─ rust_style_auto_fix.toml      # M11 fixed pipeline·exact allowlist·coverage·Gate metadata
├─ tool-packages/
│  ├─ star-control-core.toml        # required core action도 Registry 선언
│  └─ rust-style.toml               # M11 rustfmt/Clippy 4 role Tool·Check binding; raw shell 없음
├─ policies/
│  ├─ actions.toml                  # 행동 분류와 기본 승인 성격
│  ├─ risk-paths.toml               # versioned RiskPathDescriptor required set
│  ├─ rust-style.toml               # M11 built-in floor·empty/default exact fix allowlist·coverage contract
│  ├─ redaction.toml
│  └─ retention.toml
├─ validators/
│  ├─ registry.toml                 # B01~B09 Rule·Check mapping과 Validator Registry
│  ├─ gates.toml                    # GatePolicyDescriptor·protected invariant
│  ├─ fixture-manifests.toml        # Rule별 positive·negative·edge·regression 요구
│  └─ suppressions.example.toml
├─ change-recipes/
│  ├─ README.md                     # ChangeRecipe 작성·fingerprint 경계
│  └─ registry.toml                 # built-in recipe 선언
├─ maintenance/
│  ├─ external-data-sources.toml    # vulnerability/license/version source·freshness descriptor
│  └─ package-managers.toml         # ecosystem manifest·lockfile owner·typed operation
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
- Rule·Diagnostic mapping·GatePolicy 변경은 positive·negative·edge·regression, 보안·validator guard는 adversarial fixture까지 통과한다.
- Rule, ChangeRecipe와 RiskPathDescriptor는 stable ID·version·definition fingerprint를 가지며 raw shell, AI prompt와 DB query를 포함하지 않는다.
- CheckDescriptor는 trusted ToolDescriptor·typed scope binding만 참조하고 external tool command text·scanner DB를 Catalog에 복제하지 않는다.
- ExternalDataSourceDescriptor는 official source·query/schema·coverage·maximum age를 선언하지만 DB payload를 Catalog에 복제하지 않는다.
- PackageManagerAdapterDescriptor는 ToolDescriptor·typed operation·write scope·lockfile ownership을 선언하며 resolver logic이나 credential을 넣지 않는다.
- M7 Profile의 network/download/dependency change·debug attach·민감 dump·PatchSet apply checkpoint는 `personal_auto`도 `auto`로 낮출 수 없다.
- M11 Rust style metadata는 fixed step/Tool role, exact lint ID allowlist, compatible feature/target coverage와 auto ceiling만 선언한다. rustfmt option·Cargo lint level·Clippy parameter를 복제하거나 group/wildcard·raw argv를 넣지 않는다.

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

위 tree는 현재 checked-in v1 생성물의 대표 구조다. M1 구현은 `project-checkout`, `project-catalog-snapshot`, `code-index-snapshot`을 추가한다. M2 구현은 `task-spec`, `scope-revision`, `impact-analysis`, `risk-path-descriptor` Schema, ChangePlan·ChangeSet·ValidationPlan의 versioned target과 nested ResolvedProfileRef·PhaseSubjectExpectation을 추가해야 한다. M3 구현은 Rule·ValidationRun·GateDecision·EvidenceBundle·Diagnostic의 v2, ReviewPack v1, nested EvidenceSubjectBinding·SubjectBindingRecord·CompletionClaim·ClaimEvaluation·DiagnosticEvaluation·RunSatisfaction·EvidenceRefSet과 Baseline·Suppression·Disposition v2를 추가한다. M5 구현은 `managed-registry-manifest`, `managed-registry-fragment`, `managed-registry-snapshot` Schema와 nested ManagedDeclaration·AliasRecord·BindingSpec·ConsumerContract·RegistryConsistencyRecord를 추가한다. M6 구현은 `project-contract-manifest`, `contract-surface-snapshot`, `compatibility-report`, `documentation-snapshot`, `environment-snapshot`, `project-doctor-report`, `clean-room-specification`, `dependency-security-input-manifest` Schema와 nested change·consumer·config trace·assumption type을 추가한다. M7 구현은 `failure-record`, `reproduction-pack`, `regression-record`, `recovery-plan`, `dependency-snapshot`, `supply-chain-snapshot`, `external-data-snapshot`, `dependency-update-plan`, `maintenance-radar-snapshot`과 `external-data-source-descriptor`, `package-manager-adapter-descriptor` Schema를 추가한다. 문서 설계 단계에서는 해당 `.schema.json`, Registry/contract/maintenance manifest를 빈 placeholder로 만들지 않으며 typed contract·migration 구현과 같은 change에서 생성한다.

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
├─ README.md                        # case contract·redaction·기여 절차
├─ manifests/                       # case set version·RuleRef·input/expected hash
├─ change_planning/                 # M2 impact·risk·affected·fallback·no-result
├─ common_gate/                     # binding·claim·ratchet·suppression·decision
├─ change_scope/                    # B01
├─ test_trust/                      # B02
├─ validator_guard/                 # B03
├─ contract_architecture/           # B04
├─ security_supply_chain/           # B05
├─ failure_recovery/                # B06
├─ docs_environment/                # B07
├─ performance_build/               # B08
├─ release_deploy/                  # B09
└─ rust_style/                      # M11 toolchain·style edition·coverage·suggestion·side-effect·idempotence

각 기능군/
├─ manifest.toml                    # case ID·input hash·expected Diagnostic/Gate
├─ positive/                        # 허용해야 하는 사례
├─ negative/                        # 잡아야 하는 사례
├─ edge/                            # empty·limit·encoding·path·partial 경계
├─ regression/                      # 실제로 다시 막아야 하는 결함
└─ adversarial/                     # 검사·suppression·policy 우회 시도

evals/
├─ routing/                         # 모델·생각 깊이·방식 배정 비교
├─ planning/                        # 단계 분해와 재계획 품질
├─ validation/                      # TP·FP·FN과 흔들림
├─ cost_time/                       # 재작업 포함 총효율
├─ end_to_end/                      # 실제 목표 단위 평가
├─ manifests/                       # dataset version·출처·가림 상태
├─ baselines/                       # 승인된 비교 기준
├─ candidates/                      # Rule·Check·Profile·Recipe trial 정의와 fingerprint
├─ policies/                        # sample·threshold·protected metric·stop trigger 정본
└─ decisions/                       # accept/reject/deprecation source-change 근거 ref
```

Corpus는 검증 규칙의 기계적 회귀 자료이고, evals는 배정과 전체 효용을 비교하는 자료다. Corpus는 runtime Baseline·Suppression이나 external scanner database가 아니다.

`evals/` source는 case·baseline·candidate·measurement policy의 정본이고 EvaluationRun 결과는 management repository·`.ai-runs`의 derived evidence다. case result는 `cli_only`와 `codex_integrated` context를 분리하고 Rule·Check·Profile·Recipe별 duration·finding·actual defect·false positive·flaky·suppression·rework·failure와 provider-verified cost를 기록한다. recommendation이 `evals/`, Catalog 또는 config source를 자동으로 수정하지 않는다.

`policies/`는 결과를 보기 전에 sample floor·comparability·false-positive/flaky 상한·protected metric·trial stop trigger를 고정한다. `candidates/`는 effective Catalog가 아니며 shadow/bounded trial definition만 둔다. accept/reject/deprecation decision은 source change·migration·Gate로 반영하고 rejected/retired ID·version tombstone과 historical CatalogSnapshot을 보존한다.

M7 B05 corpus에는 secret/PII redaction, stale/unknown external data, workflow permission·mutable action, manifest/lockfile diff, release digest/manifest와 scanner unmapped output을 넣는다. B06 corpus에는 family/occurrence fingerprint, root/cascade DAG, rerun·reducer·bisect, before/after incompatibility, flaky, external-condition unverified와 sensitive artifact exclusion을 넣는다. dependency update fixture는 `common_gate`와 M4 patch corpus를 함께 사용해 manager-owned lockfile, unapproved network/change, preview replan, previous lockfile와 rollback을 검증한다. 실제 외부 vulnerability DB dump나 secret bytes는 Corpus에 넣지 않는다.

소유권은 다음처럼 고정한다.

| 경로 | 의미 owner | 검증 owner |
|---|---|---|
| `corpus/change_planning` | `star-planning`, M2 selector | planning/selector unit·contract test |
| `corpus/common_gate` | `star-validation` | gate pure engine·evidence binding conformance |
| `corpus/change_scope`~`release_deploy` | 대응 `star-checks` module | 해당 module unit + integration Gate test |
| `corpus/rust_style` | `star-project`, `star-validation/rust_style`, M4 adapter | toolchain/config/coverage/fix/replay/Windows conformance |
| `corpus/manifests` | `star-contracts` case Schema | `xtask corpus` |

각 case manifest는 case ID·version, fixture kind, RuleRef, input Project/Workspace/config/Catalog/Tool fingerprint, expected Diagnostic fingerprint·severity·location key, baseline/suppression relation, ValidationRun outcome/completeness/freshness/stability와 GateDecision을 고정한다. expected file을 현재 validator output으로 자동 갱신하지 않는다.

실제 사용자 작업을 넣을 때는 secret과 개인 경로를 제거하고 출처·동의·보존 정책을 기록한다. secret·사용자 이름·raw 절대 경로·repository 전체 사본과 외부 취약점 DB dump를 Corpus에 넣지 않는다. project-specific fixture는 대상 project source가 소유하고 Star-Control built-in Corpus에 자동 수집하지 않는다.

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
├─ validation/
│  ├─ contracts/                    # Binding·Claim·Diagnostic·Gate Schema golden
│  ├─ preflight/                    # M2 coherence·stale subject·CheckGraph
│  ├─ runner/                       # attempt·timeout·cancel·output limit·side effect
│  ├─ normalizer/                   # external code·path·severity·redaction mapping
│  ├─ baseline-suppression/         # existing/new/worsened·expiry·migration
│  ├─ validator-guard/              # two-snapshot·fixture·policy weakening
│  ├─ patch-gate/                   # pre/post apply binding·invalidation
│  └─ cli-only/                     # AI dependency 0·human_review 경계
├─ rust-style/
│  ├─ discovery/                    # Cargo graph·toolchain·config·style edition
│  ├─ tool-adapter/                 # rustfmt/Clippy typed argv·JSON normalization
│  ├─ coverage/                     # package·target·feature·triple·cfg matrix
│  ├─ preview-patch/                # isolated step/final diff·hunk mapping·side effect·replay
│  ├─ policy-approval/              # safe_default/personal_auto exact PatchSet decision
│  └─ cli-only/                     # inspect/check/prepare/auto-apply, AI/OpenAI dependency 0
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
   ├─ projects/                     # 작고 합성된 여러 언어 프로젝트; rust_style multi-crate/x64/ARM64 fixture 포함
   ├─ app_server/                   # 민감정보 없는 recorded event
   ├─ git_repositories/
   ├─ remote_responses/
   └─ state_versions/
```

실제 유료 호출, 원격 변경과 배포를 기본 CI에서 수행하지 않는다. 해당 검사는 별도 승인된 환경에서만 실행하고 로컬 deterministic fixture와 구분한다.

## 개발 도구·Script·Packaging 구조

```text
tools/
├─ package-release/                  # 설치 stage 생성·release-file manifest·PE/file-set 검증
│  ├─ Cargo.toml
│  └─ src/main.rs
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
├─ release.toml                     # product version source·package set·support/supply-chain policy 목표 정본
└─ windows/
   ├─ README.md                      # build·install·repair·remove 운영 절차
   ├─ star-control.iss               # Inno Setup current-user x64·ARM64 설치·제거 source
   └─ build-installer.ps1            # Cargo cross-build→검증 stage→ISCC wrapper
```

`scripts/`에는 정책 계산, Schema 해석, 상태 migration 같은 제품 logic을 두지 않는다. 복잡한 검사는 typed `xtask` command로 올려 Windows와 CI에서 같은 code를 사용한다.

### Release 산출물

```text
dist/                               # 생성물, Git 제외
├─ stage/<version>/x64/             # release-manifest.json을 포함한 검증된 설치 입력
├─ stage/<version>/arm64/
├─ star-control-windows-x64-<version>-setup.exe
├─ star-control-windows-x64-<version>.zip        # portable 정책 승인 시만
├─ star-control-windows-arm64-<version>-setup.exe
├─ star-control-windows-arm64-<version>.zip      # portable 정책 승인 시만
├─ checksums.sha256
├─ release-manifest.json
├─ sbom.spdx.json                                # applicability=required일 때
├─ provenance.json                               # applicability=required일 때
└─ signatures/                                   # signing policy가 required일 때
```

Codex Plugin과 Windows runtime은 같은 canonical 제품 version과 compatibility manifest를 사용하지만 source folder와 산출물은 분리한다. `packaging/release.toml`은 version 값을 복제하지 않고 root `Cargo.toml [workspace.package].version`을 version source로 가리키며 package role·support target·conditional supply-chain policy를 소유한다. Inno Setup installer가 검증된 Plugin template을 program payload에 포함하고, 설치 후 실제 경로를 가진 current-user 로컬 Marketplace를 렌더링한다.

architecture별 final artifact set은 한 번 build·package해 SHA-256과 set digest로 봉인한다. target/full/release verification과 channel promotion은 같은 byte를 사용한다. recompile·재압축·signing으로 byte가 바뀌면 새 candidate이며 이전 Gate를 상속하지 않는다. conditional 파일이 필요하지 않으면 empty placeholder를 만들지 않고 ReleaseManifest applicability decision을 기록한다.

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
│  ├─ project-catalog-and-code-index.md # 1단계 discovery·index·freshness 정본
│  ├─ change-planning-and-impact.md # 2단계 scope·impact·affected·fallback 정본
│  ├─ safe-patch-and-codemod.md     # 4단계 Recipe·PatchSet·single-project mutation 정본
│  ├─ managed-symbol-registry.md    # 5단계 Git 정본·lifecycle·binding·consumer 정본
│  ├─ contract-compatibility-and-environment.md # 6단계 compatibility·docs/config drift·doctor 정본
│  ├─ failure-security-and-dependency-maintenance.md # 7단계 failure·security·dependency·Radar 정본
│  ├─ migration-performance-and-platform.md # 8단계 migration·performance·language/platform 정본
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
│  ├─ common-validation-gate.md     # M3 공통 실행·ratchet·validator guard·Patch Gate
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
│  ├─ ADR-0008-P0-embedded-relational-backend.md
│  └─ ADR-0009-Git-정본-Managed-Registry와-Patch-Gate-경계.md
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
| `profiles/` | 최종 16개 작업 유형의 목적·적용 경계 | Profile 설명 정본 |
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
├─ tasks.toml                       # project TaskDescriptor·CheckDescriptor
├─ contracts.toml                   # 보호할 공개·지속 계약
├─ maintenance.toml                 # project external-data/package-manager descriptor 보강
├─ risk-paths.toml                  # additive/stronger RiskPathDescriptor
├─ rules.toml                       # project Rule enable·parameter·override
├─ suppressions.toml                # review된 shared Suppression
├─ baselines/                       # versioned shared Baseline 선언
├─ change-recipes/                  # project shared ChangeRecipe
└─ profiles/
   └─ overrides.toml                # 이 repo에 필요한 Profile 보정
```

이 폴더에는 사용자 secret, 사용자 이름, 절대 경로, 개인 비용 한도, external DB payload와 local Disposition을 넣지 않는다. `maintenance.toml`은 source URL·maximum age·typed adapter reference만 선언하며 credential·scanner output·resolved dependency state를 저장하지 않는다. 개인 설정은 `%APPDATA%\Star-Control\config.toml`, local-only decision과 projection은 관리 DB에 둔다.

`contracts.toml`은 API·CLI·Schema·file format·config·error code surface, explicit baseline policy, docs/assumption target와 environment constraint만 선언한다. managed ID·lifecycle은 `managed-registry/`를 참조하고, task/tool/check 실행 metadata는 `tasks.toml`·Catalog를 참조한다. 실제 config override나 environment value를 이 파일에 복사하지 않는다.

## Runtime과 생성 폴더

source tree와 runtime 상태를 섞지 않는다.

### 사용자 전체 상태

```text
%LOCALAPPDATA%\Star-Control\
├─ installation/
│  └─ installation-record.v1.json   # 실제 install root·version·architecture·release hash
├─ integrations/codex/<version>/
│  ├─ marketplace-root/              # 실제 EXE 경로가 렌더링된 Star-Control 소유 source
│  └─ integration-record.v1.json     # render hash·등록 상태·수동 조치
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
├─ root-bindings/                   # current-user protected opaque checkout locator
├─ worktrees/
│  └─ <project-id>\<bundle-or-run-id>\<participant-or-stage-id>\<worktree-id>\
├─ cache/
│  └─ project-index/
│     └─ <project-id>/
│        └─ <adapter-id>/
│           └─ <cache-key>/         # 다시 만들 수 있는 partition intermediate
├─ migration-workspaces/
│  └─ <project-id>/<migration-plan-id>/ # protected target copy/candidate; source·evidence 아님
├─ logs/                            # 보존 정책과 redaction 적용
├─ updates/                         # update staging과 rollback metadata
└─ recovery/                        # DB 밖 제품 lifecycle 복구본
```

`installation/`과 `integrations/`는 installer-owned local fact이며 Controller persisted projection이 아니다. 기본 uninstall은 installation record를 지우고, Codex 등록 해제가 확인될 때만 해당 Marketplace source를 지운다. 나머지 runtime state와 `%APPDATA%\Star-Control` 사용자 설정은 보존한다. 실제 DB filename, extension, connection string과 backend setting은 이 layout의 공개 계약이 아니다. directory name에는 ProjectId·stable adapter ID·content fingerprint 외 project 이름·repository 이름·사용자 이름·source path를 넣지 않는다. 관리 DB에는 `root_binding_id`만 두고 raw project absolute path는 저장하지 않는다. cache는 active generation·backup에 포함되지 않고 삭제 뒤 재scan할 수 있어야 하며 source 전체 byte와 민감 literal을 저장하지 않는다.

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
│     │  ├─ provenance.json
│     │  └─ evidence-bundle.json
│     ├─ review/
│     │  ├─ review-pack.json
│     │  ├─ review-pack.md
│     │  └─ gate-decision.json
│     ├─ merge/
│     │  ├─ plan.json
│     │  ├─ conflicts.json
│     │  └─ result.json
│     └─ reports/
│        ├─ final-report.md
│        └─ handoff.md
└─ management/
   ├─ scans/<scan-run-id>/           # catalog/index refs·coverage·freshness evidence
   ├─ patches/<patch-set-id>/
   ├─ validations/<validation-result-id>/
   ├─ migrations/<migration-plan-id>/ # attempt·checkpoint·restore/invariant manifest refs
   ├─ performance/<comparison-id>/    # raw cohort·profile/build report refs
   ├─ language-migrations/<plan-id>/  # behavior·equivalence·cutover/rollback refs
   └─ change-bundles/<bundle-id>/     # 이 Project participant·worktree·merge·remote evidence
```

`.ai-runs/`는 기본적으로 Git에서 제외한다. 사용자가 명시적으로 내보낸 redacted report만 별도 경로에서 commit할 수 있다.

### Source·생성물·로컬 자료 분류

| 분류 | 위치 | 처리 |
|---|---|---|
| 사람이 편집하는 source | `apps/`, `crates/`, `catalog/`, `docs/`의 비생성 문서와 Project `.star-control/contracts.toml`·`managed-registry/` | review와 CI 대상; 해당 Git manifest가 정본 |
| 생성 후 commit하는 계약 | `specs/schemas/`, `docs/generated/`, Registry가 선언한 language binding output | 선언된 generator로만 갱신, 직접 편집 금지·drift 검사 |
| test·평가 source | `tests/`, `corpus/`, `evals/` | version·출처·기대 결과 관리 |
| build·release 생성물 | `target/`, `dist/`, coverage | Git 제외, 다시 생성 |
| runtime 상태 | `%LOCALAPPDATA%`의 backend-neutral 관리 DB·root binding·재생성 index cache, 대상 repo `.ai-runs/` | source release와 분리; cache는 정본·backup 아님 |
| local-only 과거 자료 | `legacy/` | 읽기 전용, 현재 설계와 package 입력 제외 |

## 23개 구현 기능의 소유 Package

각 기능은 기본 소유 Package를 하나만 가진다. 보조 Package는 외부 연결이나 공통 계약만 제공한다.

| ID | 기본 소유 위치 | 주요 보조 위치 |
|---|---|---|
| A01 목표·작업 계약 | `star-planning/task_contract·scope_revision` | `star-contracts`, `star-application` |
| A02 단계 계획·재계획 | `star-planning/decompose·stage_graph·replan·scope_revision` | `star-domain`, `star-application` |
| A03 프로젝트 이해·Context Pack | `star-project`의 discovery·identity·inventory·classification·index tier·graph·freshness | `star-contracts`, `star-ports`, Git·Windows adapter, `catalog/profiles` |
| A04 변경 영향·위험 | `star-planning/seed·impact·risk_paths`와 `star-validation/selector·fallback·previous_success` | `star-project` typed graph query, `star-checks/change_scope`, `star-application` |
| A05 Codex 단계별 배정 | `star-routing` | `star-adapter-codex/capability`, `catalog/routing` |
| A06 Codex 실행·터미널 제어 | `star-application`과 `star-execution` | `star-config/registry`, `apps/`, `star-ipc`, Codex·Windows adapter |
| A07 상태·Checkpoint·자체 복구 | `star-state`의 management repository·migration·recovery | `star-execution/checkpoint·recovery`, `star-evidence` |
| A08 권한·승인·격리·secret | `star-policy` | `star-adapter-windows`, `star-evidence/redaction` |
| A09 Worktree·병렬·병합 | `star-vcs`의 baseline·overlap·worktree·merge_queue·conflict | `star-execution/parallel·change_bundle·resource_budget`, `star-adapter-git`, `star-validation/change_bundle_gate` |
| A10 Task·Tool·Validation·Profile Registry와 Managed Registry | 실행 Catalog는 `star-config/registry`·`catalog/`·`tools.d`; 계약 값은 `star-project/managed_registry` | `star-mcp`, `star-validation/registry·registry_consistency`, `star-planning`, `star-execution`, `star-ports` |
| B01 diff·범위·주장·증거 | `star-validation/findings·claim·gate` | `star-planning` scope·impact, `star-checks/change_scope`, `star-evidence/review_pack` |
| B02 테스트 신뢰성 | `star-checks/test_trust` | `star-validation`, `corpus/test_trust` |
| B03 검증기 보호·Corpus | `star-checks/validator_guard` | `corpus/validator_guard`, `star-evaluation` |
| B04 계약·구조·설정·migration | `star-checks/contract_architecture`의 comparator·consumer/companion/migration invariant evaluator | `star-project/contract_surface·managed_registry`, `star-validation/contract_evidence·migration_evidence`, `star-execution/migration`, `specs/`, `star-config` |
| B05 보안·dependency·공급망 | `star-checks/security_supply_chain` | `star-project/dependency_inventory`, `star-validation/maintenance_evidence`, `star-policy`, `star-evidence/redaction·supply_chain·dependencies`, M6 `DependencySecurityInputManifest` |
| B06 실패 재현·대상 복구 | `star-checks/failure_recovery` | `star-execution/reproduction·migration`, `star-validation/maintenance_evidence·migration_evidence`, `star-evidence/reproduction·migrations`, registered tool port |
| B07 문서·설정·개발 환경 | `star-checks/docs_environment`의 docs/config/assumption evaluator·doctor/readiness | `star-project/documentation·environment·toolchain`, read-only environment/tool port |
| B08 성능·자원·build | `star-checks/performance_build` | `star-execution/measurement`, `star-validation/performance_evidence`, `star-evidence/performance·costs`, `star-evaluation`, registered profiler/build adapter |
| B09 CI·release·배포 준비 | `star-checks/release_deploy`·`star-validation/release_gate` | `star-application/release`, `star-execution/release`, CI/build/installer/signer/remote adapter, `packaging/`, `star-evidence/release` |
| C01 최종 16개 개발 작업 Profile | `catalog/profiles` | `star-config`, `docs/features/profiles.md`; M11은 `rust_style_auto_fix` |
| D01 여러 project·원격 Git·조사 | `star-vcs/multi_repo·remote_state·remote_operation·release_handoff` | `star-application`, `star-execution/change_bundle`, Codex·remote Git adapter |
| D02 비용·평가·규칙 개선 | `star-evaluation`의 cohort·metric·comparison·lifecycle recommendation | `evals/`, `star-validation/validator_guard`, `star-evidence/evaluation`, `star-routing/shadow` |
| D03 Windows 배포·제품 수명주기 | `packaging/windows`와 `star-application/release` | `star-execution/release`, `integrations/codex-plugin`, Windows/installer/signer/remote adapter, state·config migration |

이 표에 없는 Package가 새 기능의 판단을 소유하면 구조 위반이다. 기능 소유권이 바뀌면 이 표와 관련 ADR을 함께 갱신한다.

## 주요 정본과 단일 Writer

| 정보 | 정본 | Writer |
|---|---|---|
| 직렬화 계약 | `star-contracts` | contract 변경 작업 |
| Project stable identity·shared Rule·Recipe·Suppression·Baseline | Git의 `.star-control`·Catalog | review된 source 변경 작업 |
| ManagedDeclaration·namespace·alias·lifecycle·consumer 계약 | Project Git `.star-control/managed-registry/manifest.toml`과 명시 fragment | 승인된 M4 single-project PatchApplication만; DB/UI 직접 writer 없음 |
| Registry language별 generated output | manifest BindingSpec가 선언한 Project source path | pinned generator가 만든 operation을 포함한 승인된 M4 PatchApplication만 |
| public surface·baseline·docs·environment constraint 선언 | Project Git `.star-control/contracts.toml` | review된 contract 변경 작업과 승인된 M4 PatchApplication만; doctor/DB writer 없음 |
| external source·package-manager project 선언 | Project Git `.star-control/maintenance.toml` | review된 source 변경; payload·credential·상태 writer 없음 |
| Failure/Dependency/SupplyChain/Update/Radar document | ProjectId별 management repository | Controller application transaction; scanner/debugger/package manager 직접 writer 없음 |
| ProjectMigrationManifest·PerformanceWorkloadSpec | 대상 Project Git `.star-control/migrations.toml`·`performance.toml` | review된 source 변경과 승인된 M4 PatchApplication만 |
| Rust formatting/lint/toolchain source | 대상 Project Git의 `rustfmt.toml`/`.rustfmt.toml`, Cargo `[lints]`/`[workspace.lints]`, source attribute, `clippy.toml`/`.clippy.toml`, `rust-toolchain.toml` | Project owner의 review된 source 변경과 별도 ChangePlan/Profile; M11은 `.rs`만 수정 |
| Rust fix allowlist·coverage·auto grant | versioned built-in/user/project Catalog Profile metadata와 user StarConfig grant | review된 Catalog/config source 변경; DB·Diagnostic·EvaluationRun 역쓰기 금지 |
| Rust style run·Patch·Evidence | existing RecipeExecution·PatchSet·PatchApplication·ValidationRun·EvidenceBundle projection | Controller application/evidence transaction; cargo/rustfmt/Clippy 직접 writer 없음 |
| MigrationPlan·Checkpoint·Attempt·Validation·Restore record | ProjectId별 management repository와 `.ai-runs` evidence | Controller M8 application/evidence transaction; migration tool 직접 writer 없음 |
| PerformanceRun·Comparison, LanguageMigrationPlan·EquivalenceReport | ProjectId별 management repository와 `.ai-runs` evidence | Controller M8 application/evidence transaction; profiler/compiler 직접 writer 없음 |
| CrossProjectMigrationHandoff | global summary ref + project participant document refs | Controller read-only handoff builder; 9단계 ChangeBundle writer 아님 |
| MultiProjectGoal·CrossRepoChangeBundle | global store participant ref·step graph·state projection | Controller 9단계 application transaction; project source detail writer 아님 |
| ChangeBundleParticipant·Worktree·MergeQueue/Conflict/Result | ProjectId별 management repository + project `.ai-runs` ArtifactRef | Controller application/evidence transaction과 Git adapter receipt; global row 직접 writer 없음 |
| RemoteStateSnapshot·RemoteOperationRecord | ProjectId별 management repository + global summary ref | remote adapter는 observation/receipt만 반환, Controller transaction만 state writer |
| ChangeBundleReleaseHandoff | global small document + project release input refs | Controller handoff builder; release/publish writer 아님 |
| product version·release package/support policy | root `Cargo.toml [workspace.package].version`, `packaging/release.toml`, `CHANGELOG.md`, `LICENSE` | review된 release source 변경; generated package metadata 직접 편집 금지 |
| ReleaseManifest·artifact set·release Gate/status | global management document + project refs + release ArtifactRef | Controller 10단계 application/evidence transaction; CI/build/installer/signer/remote adapter 직접 writer 없음 |
| published·deployed remote proof | exact provider after RemoteStateSnapshot | Controller가 verified observation으로 projection; adapter receipt·local tag 직접 writer 아님 |
| dependency manifest·lockfile source | 대상 Project Git source | 승인된 M4 PatchApplication 안의 registered package manager output만; DB/core 직접 writer 없음 |
| 생성 Schema | `specs/schemas` | `xtask schema`만 |
| 제품 기본 설정 | `catalog/defaults` | 설정 변경 작업 |
| 최종 16개 Profile | `catalog/profiles` | Profile 변경 작업 |
| release Tool package 선언 | `catalog/tool-packages` | Registry 계약 변경 작업 |
| 사용자·프로젝트 외부 tool | 각 `tools.d/*.toml` | 사용자와 trusted project 설정 |
| 실행 시 live tool 목록 | ToolRegistrySnapshot | Controller `registry_runtime` 한 process만 |
| 정책·검사 metadata | `catalog/policies`, `catalog/validators` | 해당 정책·검사 변경 작업 |
| 실행 시 Validator Registry·Gate policy 목록 | ValidatorRegistrySnapshot | Controller가 검증한 Catalog snapshot publish |
| 실행 중 Goal·Stage 상태 | Controller user-data state | `star-controller` 한 process만 |
| TaskSpec·ScopeRevision·ImpactAnalysis summary·ValidationPlan | global planning coordinator와 project participant DocumentRef | Controller application transaction만 |
| project별 ChangeSet·ImpactEdge·ChangePlan | ProjectId별 management repository | Controller application transaction만 |
| RecipeExecution·PatchSet v2·PatchApplication·recovery state | ProjectId별 management repository + `.ai-runs` forward/reverse/tool artifact | Controller application/evidence transaction만 |
| Project directory·ProjectCheckout·ProjectCatalogSnapshot·cross-project coordination | global management repository | Controller application transaction만 |
| ProjectRevision·WorkspaceSnapshot·CodeIndexSnapshot·Symbol·Finding projection | ProjectId별 management repository | `star-controller`가 주입한 `star-state` adapter 한 process만 |
| ManagedRegistrySnapshot·binding/consumer observation·RegistryConsistencyRecord | ProjectId별 management repository와 evidence | `star-controller`가 주입한 read-only scan/validation transaction 한 process만; source writer 권한 없음 |
| ContractSurface·Documentation·Environment snapshot, Compatibility·Doctor report | ProjectId별 management repository와 `.ai-runs` evidence | Controller의 read-only scan/validation transaction만; target·system source writer 권한 없음 |
| index intermediate cache | `%LOCALAPPDATA%\Star-Control\cache\project-index` | `star-state` cache adapter; semantic current 판단 권한 없음 |
| local Suppression·Disposition·ChangePlan v1/v2 | ProjectId별 management repository | Controller application transaction만 |
| ValidationRun·Result·raw Diagnostic·DiagnosticEvaluation·RunSatisfaction·GateDecision | ProjectId별 repository + global coordinator ref | Controller application transaction만 |
| DB backend·table·migration 구현 | `star-state` private adapter | 승인된 persistence 변경 작업 |
| 프로젝트 실행 증거·EvidenceBundle·ReviewPack | `.ai-runs/star-control/<run-id>` | Controller의 state·evidence transaction |
| 배정·검증 평가 source | `evals/`의 manifest·baseline·candidate·policy | 승인된 평가 source 변경 작업 |
| EvaluationRun·case result·recommendation | management repository + `.ai-runs` evaluation artifact | Controller evaluation transaction; Catalog/config 자동 writer 없음 |
| Rule·Check·Profile·Recipe lifecycle | Catalog source + replacement/migration/tombstone | review된 Catalog 변경 작업; Radar/EvaluationRun 직접 writer 없음 |
| 검증기 회귀 사례 | `corpus` | 규칙 변경 작업 |
| 사람이 읽는 현재 설계 | `docs`의 비생성 정본 | 설계 변경 작업 |
| CLI·MCP·Schema reference | `docs/generated` | `xtask docs`만 |

### 6단계 docs·config·generated·doctor 소유권

| 대상 | source owner | observer/evaluator | write 경계 |
|---|---|---|---|
| 사람이 쓰는 문서 | `docs/`의 비생성 Markdown | `star-project/documentation` → `star-checks/docs_environment` | 설계/문서 변경 작업만; doctor 수정 금지 |
| config key identity·lifecycle | `.star-control/managed-registry/` | `star-project/managed_registry·registry_binding` | M4 PatchApplication만 |
| config Schema·default·override | `star-contracts`, `catalog/defaults`, StarConfig source | `star-config`·`ConfigKeyTrace` evaluator | 각 정본 writer만; 실제 값 evidence 복사 금지 |
| generated Schema/reference | typed contract·Catalog·generator input | generator provenance와 `docs_environment` drift check | `xtask schema\|docs` operation을 포함한 승인 PatchSet만; 직접 편집 금지 |
| public compatibility 선언 | `.star-control/contracts.toml` | `contract_architecture` pure comparator | review된 contract 변경만; derived report 역쓰기 금지 |
| doctor·environment | `contracts.toml` constraint와 registered ToolDescriptor | `star-project/environment` read-only probe + `docs_environment` evaluator | target/system write 없음; local derived evidence만 Controller writer |
| clean-room | 승인된 `CleanRoomSpecification` | readiness evaluator와 별도 disposable M3 Check | environment 생성·install은 6단계 writer 책임이 아님 |

doctor는 새 독립 Package나 privileged service가 아니다. `star-application` command가 read-only port를 조립하고 `star-checks/docs_environment`가 결과를 pure evaluation한다. source mutation, package manager install, network와 Windows setting adapter 의존을 이 경로에 넣으면 구조 위반이다.

### 8단계 migration·performance·language/platform 소유권

| 대상 | source/contract owner | orchestrator/evaluator | write 경계 |
|---|---|---|---|
| Project target·version·chain·invariant | `.star-control/migrations.toml`, `star-contracts/migration` | `star-execution/migration`·`star-validation/migration_evidence` | source는 M4, target effect는 approved adapter |
| Star-Control management DB migration | `specs/compatibility.toml`, `star-state/migration` private source | `star-state` lifecycle·M3 Gate | 범용 manifest/tool path로 우회 금지 |
| backup/restore/candidate target | target owner·MigrationTargetPort | `star-execution/migration`·`star-checks/failure_recovery` | opaque protected target, DB/evidence에 raw data copy 금지 |
| performance workload | `.star-control/performance.toml`·Catalog | `star-execution/measurement`·`star-validation/performance_evidence` | explicit run만, profiler/analyzer는 adapter |
| behavior·consumer·platform plan | M6 contract source와 `LanguageMigrationPlan` | `star-execution/language_cutover`·`star-validation/equivalence_evidence` | source/codegen/codemod는 M4, cutover는 approval |
| 여러 Project migration | project별 plan·PatchSet·Gate·Recovery | M8 read-only handoff | M8에는 cross-project apply가 없고 9단계 ChangeBundle만 project-local apply를 조정 |

M8 state projection과 Gate evaluator는 target DB handle, compiler AST, profiler protocol과 build cache API를 직접 알지 않는다. adapter가 반환한 typed observation·receipt를 공통 계약으로 검증한다.

### 9단계 ChangeBundle·Git·remote 소유권

| 대상 | contract/domain owner | orchestrator/adapter | write 경계 |
|---|---|---|---|
| MultiProjectGoal·BundleStep DAG·compatibility window | `star-contracts/change_bundle`, `star-vcs/multi_repo` | `star-application/prepare_change_bundle` | global plan/event만, source write 없음 |
| participant·worktree·resource state | `star-contracts/change_bundle`, `star-vcs/worktree` | `star-execution/change_bundle·resource_budget`, Git/Windows adapter | owning Project·owned root만 |
| overlap·MergePlan/Queue/Conflict/Result | `star-contracts/merge`, `star-vcs/overlap·merge_queue·conflict` | `star-execution/merge_queue`, Git adapter | repository별 serial effect·M3 merge Gate |
| remote snapshot·operation | `star-contracts/remote`, `star-vcs/remote_state·remote_operation` | `star-application/operate_remote`, remote Git adapter | action별 사용자 승인·before/after snapshot |
| bundle prepare/Goal Gate | `star-validation/change_bundle_gate` | M3 runner·global coordinator | project Gate를 대체하지 않는 pure aggregation |
| 10단계 release handoff | `star-contracts/change_bundle`, `star-vcs/release_handoff` | `star-application/create_release_handoff` | source/artifact ref만, publish 권한 없음 |

`star-adapter-codex`는 이 표의 owner가 아니다. 선택적인 Stage/Conflict proposal consumer이며 CLI-only dependency graph에는 들어오지 않는다. Git/remote adapter도 dependency order·permission·Gate·완료 상태를 판단하지 않는다.

### 10단계 Release·Evaluation 소유권

| 대상 | source/contract owner | orchestrator/evaluator | write 경계 |
|---|---|---|---|
| version·package set·support/supply-chain policy | root version source, `packaging/release.toml`, changelog/license | `star-application/release` preflight, `star-checks/release_deploy` | review된 source change만; ReleaseManifest 역쓰기 금지 |
| local_quick·target·full·release tier | ProfileDescriptor·ValidationPlan v5 | M2 selector, M3 runner·`star-validation/release_gate` | runner 재선택·완화 금지 |
| build/package candidate·artifact set | typed invocation과 final byte | `star-execution/release`, CI/build/package adapter | adapter는 byte·observation만, Controller가 manifest/status writer |
| included file·metadata·license·supply-chain evidence | final artifact set과 package policy | `star-checks/release_deploy`, `star-evidence/release` | source 추측·placeholder 금지 |
| install/update/rollback/uninstall | installer ownership source·state compatibility | `star-execution/release`, Windows/installer adapter | owned disposable target만; user data purge 별도 action |
| ready·approved·published·rollback status | ReleaseManifest v2·ApprovalRequest·RemoteStateSnapshot | Controller release state reducer | CI/provider/CLI/MCP/Codex 직접 writer 없음 |
| evaluation corpus·baseline·candidate·policy | `evals/` source | `star-evaluation` pure comparator | result가 source/Catalog 자동 수정 금지 |
| EvaluationRun·Radar·lifecycle recommendation | EvaluationRun v2·Catalog lifecycle contract | Controller evaluation transaction·validator guard | accept도 review된 Catalog migration 입력일 뿐 |

CI runner, compiler, package manager, installer, signer, artifact registry와 deploy provider는 port adapter다. `star-execution/release`가 이 기능을 재구현하거나 provider SDK type을 domain에 노출하면 구조 위반이다.

`star-adapter-codex`는 evaluation `codex_integrated` context의 선택 소비자다. `cli_only` context와 core release Gate는 Codex 없이 완전해야 하며 두 context의 metric·cost를 합치지 않는다.

### 11단계 Rust style Profile 소유권

| 대상 | source/contract owner | orchestrator/evaluator | write 경계 |
|---|---|---|---|
| Cargo workspace/package/target/feature와 config/toolchain 후보 | Project Git source, `star-project/rust_workspace·rust_style_config` | read-only project adapter | source write·install·network 없음 |
| RustToolchainBinding·RustStylePolicySnapshot·RustStyleCoverageMatrix·RustStyleStepExecution | `star-contracts/rust_style` nested type | `star-application/rust_style`, `star-validation/rust_style` | existing Recipe/Patch/Validation/Evidence record에 ref; 별도 run truth 없음 |
| `rust_style_v1` Profile·Tool·Check·policy | `catalog/profiles/rust_style_auto_fix.toml`, `tool-packages/rust-style.toml`, `policies/rust-style.toml` | `star-config` Catalog resolution | raw shell·rustfmt/lint/config 값 복제 금지 |
| cargo/rustfmt/Clippy process | Tool Registry·resolved executable identity | `star-execution/rust_style_process`, ToolExecutorPort | trusted isolated cwd, owned target dir; live source write·download 금지 |
| Diagnostic·coverage·hunk·side effect·replay | raw artifact + common Diagnostic contract | `star-validation/rust_style`, `star-evidence/rust_style` | pure 판정·derived evidence; lint suppression writer 아님 |
| preview·PatchSet·apply·recovery | M4 RecipeExecution/PatchSet/PatchApplication | existing preview/SourceMutationPort/recovery | preview external write만, target은 immutable `.rs` Patch operation만 |
| `personal_auto` exact approval | user StarConfig standing grant + ApprovalRequest contract | `star-policy/approval`, `star-application/rust_style` | exact candidate 평가 뒤 policy resolution; pre/post Gate·permit 우회 없음 |
| CLI | `star style rust inspect|check|prepare|auto-apply` command contract | `star-cli` parse/render → Controller IPC | cargo argv·DB·filesystem·policy 직접 처리 없음 |

M11은 위 기존 Package 안의 bounded module만 추가한다. `star-rust-style.exe`, Rust-specific DB, formatter/parser/AST/LSP engine와 scheduler/watcher를 추가하면 구조 위반이다. Profile 구현은 [M11 정본](../features/rust-code-style-auto-fix.md)의 최소 Corpus를 `corpus/rust_style`, integration/E2E를 `tests/rust-style`에 둔다.

## 확장 절차

### 새 Rule 또는 ChangeRecipe 추가

1. stable ID, version, definition fingerprint와 소유 Catalog·project 선언 위치를 정한다.
2. Rule은 identity anchor·redaction parameter·Occurrence contract를 선언한다. Recipe는 target language, `text|syntax|symbol-aware|codegen` assurance, typed selector, input Schema, pre/postcondition, transformer binding, path·dirty scope, replay idempotency, rollback·permission·validation을 선언한다.
3. `star-contracts` valid·invalid·fingerprint golden과 해당 `corpus/` 사례를 추가한다.
4. source-derived 결과는 `star-project`·`star-validation` public contract로 만들고 DB query·table을 analyzer나 Recipe에 노출하지 않는다.
5. 큰 scan output과 patch는 ArtifactRef로 만들고 DB에는 요약·관계만 저장한다.
6. existing Finding identity, suppression·baseline stale 판정과 GateDecision 회귀를 검사한다.
7. Recipe는 raw literal-only global replacement, raw shell·script·AI prompt와 DB query를 포함하지 않는지 확인한다.
8. M4 corpus에서 selector ambiguity·dirty overlap·idempotence·scope 밖 output·partial apply·reverse recovery를 검사한다.

Rule·Recipe 추가만으로 CLI·MCP handler, management repository port와 DB backend를 바꾸지 않는다. 새 persisted 의미가 필요할 때만 계약·migration 변경 절차로 승격한다.

### 새 ManagedDeclaration 추가 또는 변경

1. scanner candidate인지, 기존 managed declaration인지, Registry가 소유하지 않는 local implementation constant인지 먼저 분류한다.
2. managed 대상이면 stable declaration ID, namespace claim·owner·type·value role·description, public value와 uniqueness scope를 정한다.
3. source definition, language symbol, Schema·documentation·generated output BindingSpec와 consumer 최소 지원 version·accepted version을 선언한다.
4. lifecycle `reserved|active|deprecated|removed`, replacement와 bounded alias를 검토한다. removed/reserved ID·public value와 tombstone은 재사용하지 않는다.
5. `ManagedDeclarationChangeIntent`를 M2 영향 분석에 넣고 downstream Project는 read-only로 계산한다.
6. M4가 한 Project에서 manifest와 필요한 binding을 dry-run해 immutable PatchSet을 만들게 한다. generated output을 직접 편집하거나 DB row를 source로 쓰지 않는다.
7. duplicate·namespace·alias·consumer migration·generated provenance를 M3 pre Gate에서 검사하고 exact PatchSet 승인을 받는다.
8. 적용 뒤 source를 다시 scan해 actual ManagedRegistrySnapshot과 RegistryConsistencyRecord를 만들고 post Gate·EvidenceBundle을 통과한다.

first Slice는 error code·Diagnostic ID만 사용한다. 이 절차는 Registry 전용 Package, DB source writer와 cross-repo apply를 추가하지 않는다. exact contract는 [Managed Registry 정본](../contracts/managed-symbol-registry.md)이 소유한다.

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
2. Rule ID·version·definition fingerprint·fingerprint contract와 owner module을 정한다.
3. `star-checks`의 해당 module에 공통 Diagnostic producer를 구현한다.
4. `catalog/validators/registry.toml`에 Rule·Check·Tool mapping, severity/confidence floor, ratchet eligibility와 protected invariant를 등록한다.
5. positive, negative, edge, regression fixture를 `corpus/`에 추가하고 validator/security guard면 adversarial case도 추가한다.
6. external output이면 unmapped code·truncation·path redaction·tool version fixture를 추가한다.
7. baseline existing/new/worsened, suppression active/expired/stale, flaky와 GateDecision 동작을 검사한다.
8. 기능 문서와 generated reference를 갱신한다.

검사는 raw shell을 직접 실행하지 않고 `ToolExecutorPort`와 registered ToolDescriptor를 사용한다. changed validator의 current self-test만으로 Rule·severity·allowlist·fixture 변경을 승인하지 않는다.

### 새 프로젝트 도구 연결

1. 기존 구조화 command와 result parser 등록으로 충분한지 확인한다.
2. 충분하면 대상 프로젝트 `.star-control/tasks.toml` 또는 catalog metadata만 추가한다.
3. 별도 인증, protocol 또는 lifecycle이 있을 때만 새 port 능력을 검토한다.
4. 새 adapter가 필요하면 conformance fixture와 capability discovery를 함께 구현한다.
5. adapter에는 승인과 완료 판단을 넣지 않는다.

Compiler, LSP, test runner, scanner, debugger, profiler와 CI 도구마다 별도 Package를 만들지 않는다.

### 새 M7 scanner·debugger·package manager·외부 자료 adapter 연결

1. 기존 ToolDescriptor·CheckDescriptor·`argv_v1|star_json_stdio_v1`로 표현 가능한지 먼저 확인한다.
2. external data면 ExternalDataSourceDescriptor에 official source·query/schema·coverage·time field·maximum age와 Tool ref를 등록한다.
3. package manager면 PackageManagerAdapterDescriptor에 manifest/lockfile kind, typed operation, offline/network/cache effect, expected write scope와 rollback verification을 등록한다.
4. scanner/debugger output은 common RuleRef·Diagnostic mapping, raw ArtifactRef, redaction·retention과 unmapped/truncated fixture를 가진다.
5. network/download/dependency change·process attach·민감 dump action을 숨기지 않고 `personal_auto`에서도 prompt floor를 검증한다.
6. fake adapter conformance 뒤에만 실제 executable을 연결하고, adapter success가 M3 GateDecision을 만들지 못하는 E2E를 추가한다.
7. package manager preview는 isolated worktree에서 actual diff를 만들고 M2 replan·immutable PatchSet·previous lockfile 뒤 `awaiting_apply_approval`에서 멈춘다.

Star-Control core에 vulnerability/license/package DB, resolver, debugger protocol, scanner engine, PKI나 general network client를 추가하지 않는다. 새 protocol/lifecycle이 generic Tool executor로 안전하게 표현되지 않을 때만 narrow port를 검토하며, 그 경우에도 permission·Gate 의미는 core 계약을 재사용한다.

### 새 M8 migration·measurement·language/platform adapter 연결

1. migration tool은 version probe, dry-run/no-live-write, target effect, receipt·checkpoint, backup/restore와 invariant capability를 각각 선언한다.
2. target locator는 ProjectPathRef 또는 opaque binding이며 SQL·connection string·credential·raw 개인 path를 core 계약에 넣지 않는다.
3. backup adapter는 consistency point와 set manifest를, restore adapter는 실제 restore target·integrity·behavior result를 별도 output으로 반환한다.
4. benchmark/build tool은 workload/input/cache mode, numeric unit·collector, warmup/measured attempt와 environment probe를 표현한다.
5. profiler/build analyzer output은 ArtifactRef와 candidate cause Diagnostic으로 정규화하며 GateDecision을 만들지 않는다.
6. compiler/test/codegen/codemod는 기존 Task·Check·M4 transformer descriptor를 사용하고 compile result와 equivalence dimension을 분리한다.
7. fake adapter에서 timeout·cancel·crash·partial receipt·outcome unknown·unmapped output·redaction conformance를 먼저 통과한다.
8. local Windows, remote CI, cross-compile, emulator/simulator와 native evidence kind를 구분한다.
9. destructive/live write, profiler attach, remote execution, cutover와 rollback permission을 하나의 action으로 묶지 않는다.
10. multi-project target을 받는 adapter는 M8에서 거부하고 `CrossProjectMigrationHandoff`만 생성한다.

core에 DB engine, migration framework, profiler, build analyzer, compiler, transpiler, package manager 또는 target OS runtime을 추가하지 않는다. generic ToolDescriptor로 안전하게 표현할 수 없는 atomic target activation/restore만 narrow `MigrationTargetPort` capability로 검토한다.

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
- 범용 Project migration은 product version과 별도 target별 `MigrationVersionVector`를 사용하고 Star-Control 자체 `management_store_version`과 합치지 않는다.
- migration은 dry-run, consistent backup, backup integrity, restore rehearsal, migration rehearsal, execute/resume, 검증, activation과 rollback 단계로 나눈다.
- backup byte 존재와 restore 후 structural·behavior 검증을 별도 상태로 기록한다.
- partial/outcome unknown은 success가 아니며 actual checkpoint before/expected-after를 reconcile하기 전 재실행하지 않는다.
- destructive migration은 승인 없이 실행하지 않는다.
- source/config/migration script 변경은 M4 PatchSet, live target data effect는 별도 M8 attempt·permission·Gate가 소유한다.
- 여러 Project migration은 9단계 ChangeBundle 전에는 project별 plan·handoff만 만든다.

### 의존성 관리

- workspace dependency version은 root `Cargo.toml`에서 한 번 관리한다.
- runtime dependency 추가는 목적, 대안, license, 보안과 binary 크기 영향을 기록한다.
- adapter 전용 dependency를 core Package로 끌어오지 않는다.
- `Cargo.lock`은 Windows release와 CI에서 고정한다.
- repository scan은 manifest·lockfile과 direct/transitive/internal package relation을 관찰하지만 DB를 version 정본으로 만들지 않는다.
- update 후보는 patch/minor/major/security/internal과 affected Project·freshness를 가진 `DependencyUpdatePlan`으로 만들고, 등록 package manager가 isolated worktree에서 lockfile을 생성한다.
- core·codemod가 lockfile을 직접 편집하거나 dependency closure를 역산하지 않는다.
- network read/download, dependency add/change와 PatchSet apply는 exact 사용자 승인 전 실행하지 않으며 기본 종료점은 `awaiting_apply_approval`이다.
- previous manifest·lockfile, reverse PatchSet과 rollback validation이 없으면 dependency PatchSet apply를 허용하지 않는다.

### CI 단계

```text
quick
  -> same Task/source/config/Profile
  -> format + compile + changed-package unit + contract drift

target
  -> affected package + integration + relevant corpus

full
  -> clean Windows workspace lint + all tests + corpus + resilience + docs links

release
  -> build/package once + immutable artifact set digest
  -> clean Windows x64 Stable native E2E + ARM64 Preview cross-build/simulation
  -> x64 install/safe_default/update/failure rollback/repair/uninstall + ARM64 fake lifecycle
  -> package file manifest + checksum + metadata/license
  -> applicable SBOM/provenance/signing
  -> ready; explicit approval 뒤 publish, after snapshot 뒤 published
```

`.github/workflows/`에는 `quick.yml`, `full.yml`, `docs.yml`, `security.yml`, `release.yml`만 두고 실제 command 선택은 `scripts/`와 `xtask`가 소유한다. CI YAML에 제품 logic을 복사하지 않는다. 각 workflow는 provider run ID가 아니라 Task ID·source revision·config·Catalog·Tool·Profile fingerprint를 evidence에 전달한다. release workflow는 full/release 검증을 위해 candidate byte를 내려받을 뿐 다시 build하지 않는다.

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
| M1 | P0 Project v1→checkout-aware schema migration, read-only discovery·inventory·text index, 첫 syntax adapter, graph·hardcoding candidate, full/incremental generation·freshness와 CLI query Slice |
| M2 | TaskSpec·ScopeRevision·ImpactAnalysis와 ChangePlan v2 target, `star-planning` pure impact Slice, `star-validation/selector` affected·fallback Slice, CLI-only source read-only planning E2E |
| P1 | foundation 4개 Package, rmcp 2.2 고정 Gateway, authenticated IPC, live Registry·Win32 runtime, Schema·fixture와 MCP matrix 수직 slice |
| P2 | Controller·CLI app skeleton, core Tool package, `integrations/codex-plugin` |
| P3 | M2 `star-planning`을 StageGraph·재계획·완료 판단으로 확장하고 `star-routing`, `star-policy` 생성 |
| P4 | P0 `star-application`·`star-state`·`star-execution`을 Goal/Codex lifecycle로 확장하고 `star-evidence`, Codex adapter 생성 |
| P5 (M3) | M2 ready plan을 소비하는 `star-validation` preflight·binding·runner·normalizer·ratchet·gate, B01~B07 `star-checks`, Baseline/Suppression v2, `corpus/`·Patch pre/post Gate와 CLI-only tests 생성 |
| M4 | ChangeRecipe v2·TargetSelector·RecipeExecution·PatchSet v2·PatchApplication, dry-run preview·idempotence·single-project worktree·internal apply/recovery와 CLI-only E2E. M1→M2→M3 제품 gate 뒤 시작 |
| M5 | Managed Registry type·Schema·Git manifest loader·derived snapshot, error-code first Slice, binding/consumer consistency와 M2→M4→M3 CLI-only E2E. M1→M4 제품 gate 뒤 시작 |
| M6 | Contract/environment 8개 type·Schema, explicit baseline snapshot/comparator, docs/config trace, read-only Windows doctor·clean-room readiness와 7단계 input handoff. M1·M3·M5 current evidence gate 뒤 시작 |
| M7 | Failure/Reproduction/Regression/Recovery, Dependency/SupplyChain/ExternalData/Update/Radar type·Schema와 pure fingerprint·freshness·ordering, read-only CLI, adapter normalization, 승인-gated isolated dependency PatchSet. M1·M3·M6 current evidence gate 뒤 시작 |
| M8 | Migration/Checkpoint/Attempt/Validation/Restore, Performance workload/run/comparison, Language plan/equivalence/handoff type·Schema, pure chain/state/comparability/equivalence, fake adapter, read-only CLI, isolated rehearsal 뒤 approval-gated single-Project execute. M3·M4·M6·M7 current evidence gate 뒤 시작 |
| P6 | `star-vcs` local baseline·overlap·WorktreeRecord·MergePlan v2·queue/conflict/result, Git fake/adapter와 CLI-only local integration tests |
| P7 | MultiProjectGoal·CrossRepoChangeBundle·participant state/Gate/recovery, RemoteStateSnapshot v2·explicit-approval remote adapter, release handoff와 multi-project integration tests |
| P8 | EvaluationRun v2, `star-evaluation` pure cohort·metric·comparison, `evals/` policy/candidate, shadow·trial, Radar·lifecycle migration와 CLI/Codex context 분리 |
| M11 | read-only Cargo/toolchain/config discovery → Rust nested contract/coverage → rustfmt/Clippy check → isolated rustfmt PatchSet → exact allowlisted Clippy hunk → convergence/replay → candidate/post Gate → `personal_auto` exact approval → Windows x64 CLI-only·ARM64 target/cfg simulation Corpus. M1→M2→M3→M4 제품 Gate 뒤 mutation을 시작하고 P9 전에 conformance를 완료 |
| P9 | ReleaseManifest v2·evidence v6, final 16 Profile·M11 conformance, release tier/Gate, build-once artifact set, `packaging/windows`, clean x64 Stable install lifecycle·ARM64 Preview simulation, approval·GitHub publish after-state와 최종 운영 문서 |

M1·M2·M3·M4·M5·M6·M7·M8과 사용자 9·10·11단계는 이 문서에서 지정한 관리 확장을 뜻하며 기존 제품 로드맵의 P1·P2·P3·P7 번호와 다르다. M3 제품 구현은 로드맵 P5 첫 수직 Slice이고 M4는 그 Gate를 소비한 뒤 P6 병렬·merge보다 좁은 single-project worktree capability부터 사용한다. M5~M8은 새 기능별 Package 없이 read-only Index·planning·Patch·Gate·check·evidence 경계에 Registry, compatibility/environment, maintenance와 migration/performance/equivalence 계약을 조립한다. 9단계도 새 Package를 만들지 않고 P6 `star-vcs` local integration과 P7 `star-execution/star-application` coordination·remote adapter를 조립한다. 10단계는 P8 `star-evaluation`과 P9 release/packaging을 같은 Gate·evidence 위에 조립한다. M11은 기존 Project/Tool/Patch/Gate/Evidence module에 Rust bounded adapter를 조립하고 P9 공개 배포 conformance 앞에 둔다. 현재 M1~11단계의 관리·release/evaluation 기능은 구조·계약 문서만 확정했고 module·migration·validator·runner·rewrite·worktree·merge queue·ChangeBundle·remote operation·release/evaluation/Rust style engine·Registry/contract/maintenance/migration·codemod·doctor·clean-room runner·debugger·scanner·dependency updater·benchmark·profiler·compiler·CI/signer/deploy adapter·network client·Corpus를 구현하지 않았다. 예외로 P-0026은 M10 전체와 분리된 installation transport만 구현해 technical release-file manifest·installation record·Inno Setup installer·Codex Plugin 렌더링을 제공한다. P0의 backend·dependency는 별도 승인 뒤에만 추가한다. 이미 구현된 P1 MCP와 P-0026 수직 Slice는 그 역사와 검증 상태를 유지하지만 M1~11단계 관리 계약이나 M10 release `ready`가 구현됐다는 근거가 되지 않는다. 각 단계는 다음 단계가 실제로 필요로 하는 공개 계약까지만 먼저 만들고 미래 Package의 빈 폴더와 사용되지 않는 추상화를 만들지 않는다.

## 구조 검증 항목

다음 검사는 repository 정책으로 자동화한다.

- Cargo workspace의 Package가 이 문서의 허용 계층을 거스르지 않는지
- engine이 filesystem·process·network·Git을 직접 호출하지 않는지
- CLI와 MCP가 Controller 상태를 직접 쓰지 않는지
- CLI·MCP·Codex entry adapter가 management DB·ArtifactStore를 직접 열지 않는지
- CLI-only dependency graph와 E2E가 Codex·App Server·다른 AI·OpenAI API를 호출하지 않는지
- 모든 project-scoped DB relation이 ProjectId partition과 project-relative path를 사용하는지
- Project와 checkout identity가 분리되고 linked worktree가 common repository identity를 ProjectId로 오인하지 않는지
- discover·scan·index·query dependency graph에 source-write port, scheduler, watcher, AI client가 들어오지 않는지
- CodeIndexSnapshot의 current 판정이 revision·workspace·config·classification·adapter fingerprint를 모두 대조하는지
- syntax·semantic 미지원·parse 실패·no-result가 text fallback과 limitation을 보존하는지
- management store migration·backup·corruption·read-only recovery·rebuild fixture가 있는지
- DB backend 이름·SQL type이 public contract와 StarConfig에 노출되지 않는지
- `star-contracts` 밖에 중복 wire type이 없는지
- 23개 기능 ID와 최종 16개 Profile에 소유 위치가 있는지
- management DB·Project Catalog·Code Index·Finding·Managed Registry·ChangeRecipe·ChangeBundle·ReleaseManifest·EvaluationRun의 source/derived/Writer가 하나씩인지
- Catalog 항목과 Profile 문서·generated reference가 일치하는지
- Schema와 generated 문서를 다시 생성했을 때 diff가 없는지
- 모든 migration version에 old-version fixture가 있는지
- 각 built-in check에 필요한 Corpus 종류가 있는지
- M3 runner가 M2 selected Check·scope·fallback을 재선택하지 않고 registered ToolDescriptor만 실행하는지
- required evidence의 subject revision·WorkspaceSnapshot·plan·config·Catalog·Rule·Tool fingerprint가 current인지
- test·architecture·hardcoding·docs·security 결과가 같은 RuleRef·Diagnostic·Gate 계약을 사용하는지
- Baseline existing/new/worsened와 Suppression active/expired/stale가 raw outcome을 바꾸지 않는지
- validator·policy·test harness 변경을 pre-change/current two-snapshot guard와 positive·negative·edge·regression Corpus가 검사하는지
- local_quick·target·full·release가 같은 Task/source/config/Catalog/Tool/Profile identity를 유지하는지
- final artifact가 source revision에 결합되고 build-once byte와 promotion digest가 같은지
- `ready`, `approved`, `published`, `publish_outcome_unknown`을 같은 상태로 축약하지 않는지
- publish·deploy가 action별 approval과 exact remote after snapshot 없이 완료되지 않는지
- Rule·Check·Profile·Recipe evaluation이 actual defect·FP·flaky·suppression·rework·duration과 provider-verified cost를 보존하는지
- evaluation candidate가 validator·required Check·Corpus·ratchet·freshness를 약화하지 않는지
- CLI-only와 Codex-integrated evaluation result·dependency graph가 분리되는지
- release/evaluation Package가 compiler·scanner·profiler·package manager·CI·installer·signer·deploy service를 재구현하지 않는지
- CLI-only validation dependency graph에 Codex·App Server·다른 AI·OpenAI API가 없고 의미 검토가 `human_review`인지
- Patch engine이 stale pre-apply decision을 거부하고 post-apply after binding에서 새 Gate를 만드는지
- M4 prepare가 live target source를 바꾸지 않고 PatchSet·diff·impact를 먼저 만들며 prepare command에 apply path가 없는지
- raw literal-only global replacement와 cross-project PatchSet을 Schema·domain에서 거부하는지
- external mutating codemod가 Tool Registry·typed args·isolated worktree를 사용하고 live target path를 받지 않는지
- Recipe ID/version·transformer/Tool version/hash·input/output·idempotence·operation receipt가 evidence에 남는지
- dirty overlap·partial apply·outcome unknown을 성공으로 만들지 않고 reverse PatchSet 또는 owned worktree discard로 복구하는지
- `rust_style_auto_fix`가 기존 3 executable과 M1/M2/M3/M4 application path만 사용하고 AI/OpenAI/browser/scheduler·별도 Rust engine이 없는지
- Rust toolchain/config/style edition·exact allowlist·coverage/step fingerprint가 PatchSet/Gate/Evidence에 bind되고 DB가 source truth가 아닌지
- rustfmt/Clippy fix가 live checkout에서 실행되지 않고 final complete diff가 handwritten in-scope `.rs` modify만 포함하는지
- Clippy fix가 exact lint ID·`MachineApplicable`·actual hunk 대응을 요구하고 group/wildcard·suppression·`cargo fix`를 사용하지 않는지
- package/target/feature/triple/cfg coverage가 `--all-features` 없이 declared matrix를 실행하고 partial/unavailable/conflict를 `AUTO_PASS`로 만들지 않는지
- `personal_auto`가 exact PatchSet policy ApprovalDecision, pre/post Gate와 single-use permit을 우회하지 않고 partial/post failure를 recovery로 남기는지
- M6 baseline이 immutable approval에 결합되고 current/DB latest를 baseline으로 자동 채택하지 않는지
- contract change가 consumer·Schema·generated reference·docs·migration guide와 같은 ChangePlan lineage인지
- config key lifecycle과 documented/read/overridden 관찰이 분리되고 secret·environment value가 evidence에 없는지
- docs command·doctor probe가 exact registered read-only descriptor만 사용하고 raw shell·install·network·system mutation 경로가 없는지
- EnvironmentSnapshot이 Windows path·case·encoding·line-ending·path-length를 관찰하면서 username·raw path·secret을 fingerprint에서 제외하는지
- clean-room readiness와 실제 disposable Check가 분리되고 누락 prerequisite를 자동 설치하지 않는지
- DependencySecurityInputManifest의 manifest·lockfile·toolchain·environment provenance·coverage·freshness가 후속 B05에서 검증되는지
- FailureRecord의 family/occurrence fingerprint가 revision 재발과 exact attempt를 구분하고 root/cascade DAG cycle을 거부하는지
- ReproductionPack이 일반 log와 role을 구분하고 `quarantined|unknown` artifact를 default report에서 제외하는지
- scanner·debugger·package manager가 common Diagnostic·ArtifactRef adapter이고 별도 DB·GateDecision writer가 아닌지
- ExternalDataSnapshot의 source/query/schema·coverage·freshness·valid_until이 stale/unknown clean pass를 막는지
- dependency preview가 registered package manager·isolated worktree·actual diff replan을 사용하고 승인 대기 PatchSet에서 멈추는지
- lockfile을 core/codemod가 직접 편집·역산하지 않고 previous bytes·rollback Gate를 보존하는지
- network/download/dependency change·debug attach·민감 dump·PatchSet apply가 `personal_auto`에서도 exact prompt인지
- Maintenance Radar가 공통 Finding/Suppression/snapshot ref의 rebuildable projection이고 AI 없이 deterministic한지
- 범용 Project migration과 Star-Control 자체 management store migration이 manifest·adapter·writer에서 분리되는지
- migration chain이 explicit version source와 연속 edge를 사용하고 gap·ambiguity·cycle을 거부하는지
- backup created/integrity verified/restore rehearsed/restore validated가 분리되고 destructive execute 전에 required 수준을 만족하는지
- partial/outcome unknown이 success pointer를 바꾸지 않고 checkpoint before/expected-after reconciliation 뒤에만 resume하는지
- performance Profile이 explicit workload만 실행하고 workload·input·tool·environment·mode·cohort revision 비교 가능성을 확인하는지
- numeric value·unit·collector가 없는 metric을 0·추정치로 채우지 않고 warmup·raw outlier·noise를 보존하는지
- profiler·build analyzer·compiler·migration tool이 adapter이고 M3 Gate writer가 아닌지
- language migration의 compile/build result와 기능 equivalence dimension이 분리되고 reader-first·consumer window·rollback을 지키는지
- 실행하지 않은 OS·architecture evidence가 `not_run|unverified`이며 cross-compile을 runtime pass로 만들지 않는지
- CrossProjectMigrationHandoff가 read-only이고 9단계 ChangeBundle 전 cross-project apply·merge·remote write가 없는지
- MultiProjectGoal source effect Stage가 한 Project만 소유하고 provider open→consumer→provider close DAG·window가 cycle 없이 표현되는지
- ChangeBundle이 project별 base·dirty·PatchSet·Gate·evidence를 유지하고 management coordination을 Git transaction success로 쓰지 않는지
- worktree ownership·user dirty 보존, file/symbol/contract/generated/lockfile overlap과 stale Patch/MergePlan corpus가 있는지
- repository별 merge queue가 직렬이고 conflict가 양쪽 intent·contract·resolution PatchSet·재검사를 보존하는지
- local commit/branch update와 remote push·PR/check/merge 상태가 분리되는지
- remote upload·PR·merge·publish가 action별 명시적 승인과 current before/after snapshot 없이는 실행·성공 처리되지 않는지
- partial·rollback required·held·outcome unknown participant가 Goal 완료를 막고 resume/compensation이 새 effect인지
- CLI-only ChangeBundle graph에 Codex·App Server·AI client가 없고 Codex가 선택 소비자인지
- ChangeBundleReleaseHandoff가 ProjectId별 immutable commit·artifact hash·Gate를 10단계에 연결하는지
- Plugin root 구조와 manifest 경로가 공식 형식에 맞는지
- release staging에 `legacy/`, `.ai-runs/`, secret, 사용자 절대 경로가 없는지
- 모든 Markdown 내부 연결과 package README 링크가 실제 파일을 가리키는지

## 최종 구조 완료 조건

1. `star`, Controller와 MCP가 같은 application use case와 상태를 사용한다.
2. 23개 기능마다 기본 소유 위치가 하나 있고 중복 engine이 없다.
3. 최종 16개 작업 유형이 Catalog Profile만으로 공통 engine을 조합한다.
4. 외부 도구와 Codex 변경이 adapter 밖의 Package에 직접 번지지 않는다.
5. Controller가 상태의 단일 writer이며 crash 뒤 복구 가능한 transaction 경계를 가진다.
6. Git 정본, local management projection·operation state와 `.ai-runs` evidence의 경계가 구현과 문서에서 일치한다.
7. 계약·설정·Profile·정책·증거와 생성 문서의 정본이 각각 하나다.
8. built-in 검사군은 공통 diagnostic·gate·Review Pack 계약을 사용한다.
9. Package 의존 graph에 cycle과 모호한 공용 Package가 없다.
10. clean Windows에서 설치, 첫 실행, update, rollback, uninstall을 검사할 수 있다.
11. 현재 문서만으로 source, runtime 상태, 생성물과 local-only legacy 경계를 이해할 수 있다.

이 구조는 모든 최종 기능을 담는 목표 구조다. 구현 과정에서 세부 기술을 바꿀 수는 있지만 Package 책임, 정본 위치, 단일 Writer, adapter 경계와 기능 소유권을 바꾸려면 먼저 ADR과 이 문서를 갱신해야 한다.
