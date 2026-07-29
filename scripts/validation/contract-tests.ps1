[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
. (Join-Path $PSScriptRoot "common.ps1")
. (Join-Path $PSScriptRoot "project.ps1")

function Assert-ValidationContract {
    param(
        [Parameter(Mandatory)][bool]$Condition,
        [Parameter(Mandatory)][string]$Message
    )
    if (-not $Condition) {
        throw "validation contract assertion failed: $Message"
    }
}

$config = New-ProjectValidationConfig -Root $repositoryRoot
$entryTokens = $null
$entryErrors = $null
$entryAst = [Management.Automation.Language.Parser]::ParseFile(
    (Join-Path $repositoryRoot "scripts/validate.ps1"),
    [ref]$entryTokens,
    [ref]$entryErrors
)
Assert-ValidationContract -Condition (@($entryErrors).Count -eq 0) -Message "entrypoint syntax"
$entrySource = Get-Content -LiteralPath (Join-Path $repositoryRoot "scripts/validate.ps1") -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($entrySource.Contains('[Console]::OutputEncoding = $utf8NoBom')) -Message "entrypoint stdout UTF-8 contract"
Assert-ValidationContract -Condition ($entrySource.Contains('$OutputEncoding = $utf8NoBom')) -Message "entrypoint native output UTF-8 contract"
$entryParameters = @($entryAst.ParamBlock.Parameters | ForEach-Object {
        $_.Name.VariablePath.UserPath
    })
Assert-ValidationContract -Condition (($entryParameters -join ",") -eq "Profile,Unit,BaseRef,OutputFormat") -Message "public parameter set"
foreach ($example in $config.ContractExamples) {
    $actual = & $config.ClassifyPath $example.Path
    Assert-ValidationContract -Condition ($actual -eq $example.Profile) -Message "impact $($example.Path): expected $($example.Profile), got $actual"
    $unit = & $config.ResolveUnit $example.Path
    Assert-ValidationContract -Condition (-not [string]::IsNullOrWhiteSpace($unit)) -Message "unit resolution $($example.Path)"
}
foreach ($example in $config.ContractUnitExamples) {
    $actual = & $config.ClassifyUnit $example.Unit
    Assert-ValidationContract -Condition ($actual -eq $example.Profile) -Message "unit impact $($example.Unit): expected $($example.Profile), got $actual"
}
Assert-ValidationContract -Condition (-not (& $config.ValidateUnit "__invalid_validation_unit__")) -Message "invalid unit rejection"

$quickContext = [pscustomobject]@{
    Root = $repositoryRoot
    Profile = "quick"
    RequestedProfile = "target"
    RequiredProfile = "quick"
    Unit = $null
    BaseRef = $null
    ChangedPaths = @("README.md")
    ValidationFiles = @("README.md")
    PathsFile = $schemaPath = Join-Path $PSScriptRoot "project-validation-report.schema.json"
    WholeProject = $false
    NoImpact = $false
    AffectedUnits = @("docs")
}
$quickSpecs = @(& $config.BuildChecks $quickContext)
Assert-ValidationContract -Condition (-not @($quickSpecs | Where-Object { $_.Executable -eq "cargo" }).Count) -Message "QUICK must not execute Cargo"

$targetExample = @($config.ContractExamples | Where-Object { $_.Profile -eq "target" } | Select-Object -First 1)
if ($targetExample.Count -eq 1) {
    $targetUnit = & $config.ResolveUnit $targetExample[0].Path
    $targetContext = [pscustomobject]@{
        Root = $repositoryRoot
        Profile = "target"
        RequestedProfile = "target"
        RequiredProfile = "target"
        Unit = $null
        BaseRef = $null
        ChangedPaths = @($targetExample[0].Path)
        ValidationFiles = @($targetExample[0].Path)
        PathsFile = $schemaPath
        WholeProject = $false
        NoImpact = $false
        AffectedUnits = @($targetUnit)
    }
    $targetSpecs = @(& $config.BuildChecks $targetContext)
    $targetCargo = @($targetSpecs | Where-Object { $_.Executable -eq "cargo" })
    Assert-ValidationContract -Condition ($targetCargo.Count -gt 0) -Message "TARGET must execute Cargo for Rust code"
    Assert-ValidationContract -Condition (-not @($targetCargo | Where-Object { "-p" -notin $_.Arguments }).Count) -Message "TARGET Cargo must select affected packages"
}

$cleanTargetContext = [pscustomobject]@{
    Root = $repositoryRoot
    Profile = "target"
    RequestedProfile = "target"
    RequiredProfile = "target"
    Unit = $null
    BaseRef = $null
    ChangedPaths = @()
    ValidationFiles = @()
    PathsFile = $schemaPath
    WholeProject = $true
    NoImpact = $false
    AffectedUnits = @()
}
$cleanTargetSpecs = @(& $config.BuildChecks $cleanTargetContext)
$cleanTargetCargo = @($cleanTargetSpecs | Where-Object { $_.Executable -eq "cargo" })
Assert-ValidationContract -Condition ($cleanTargetCargo.Count -eq 4) -Message "clean TARGET must execute the workspace Cargo gate"
Assert-ValidationContract -Condition (-not @($cleanTargetCargo | Where-Object { $_.Unit -ne "workspace" }).Count) -Message "clean TARGET Cargo unit must be workspace"
Assert-ValidationContract -Condition (-not @($cleanTargetCargo | Where-Object { "-p" -in $_.Arguments }).Count) -Message "clean TARGET must not emit an empty package selector"

Assert-ValidationContract -Condition ((Resolve-ValidationProfile -RequestedProfile "target" -RequiredProfile "quick") -eq "quick") -Message "target must adapt down to quick"
Assert-ValidationContract -Condition ((Resolve-ValidationProfile -RequestedProfile "quick" -RequiredProfile "target") -eq "target") -Message "quick must promote to target"
Assert-ValidationContract -Condition ((Resolve-ValidationProfile -RequestedProfile "target" -RequiredProfile "full") -eq "full") -Message "target must promote to full"
Assert-ValidationContract -Condition ((Resolve-ValidationProfile -RequestedProfile "full" -RequiredProfile "quick") -eq "full") -Message "full must not downgrade"
Assert-ValidationContract -Condition ((Resolve-ValidationProfile -RequestedProfile "release" -RequiredProfile "quick") -eq "release") -Message "release must not downgrade"

$entryError = New-ValidationEntryError -Kind "invocation" -Status "fail" -Message "invalid unit" -ExitCode 2
Assert-ValidationContract -Condition ($entryError.schema_id -eq "star.project-validation-entry-error") -Message "entry error schema id"
Assert-ValidationContract -Condition ($entryError.status -eq "fail" -and $entryError.exit_code -eq 2) -Message "entry error status and exit code"
$artifactExample = Join-Path $repositoryRoot "target/validation/example-run"
$rootToken = ConvertTo-ValidationFingerprintToken -Value (Join-Path $repositoryRoot "scripts/validate.ps1") -Root $repositoryRoot -ArtifactDirectory $artifactExample
$artifactToken = ConvertTo-ValidationFingerprintToken -Value (Join-Path $artifactExample "paths.json") -Root $repositoryRoot -ArtifactDirectory $artifactExample
Assert-ValidationContract -Condition ($rootToken -eq "<root>/scripts/validate.ps1") -Message "fingerprint root normalization"
Assert-ValidationContract -Condition ($artifactToken -eq "<artifact>/paths.json") -Message "fingerprint artifact normalization"

$passCheck = [ordered]@{ status = "pass" }
$failCheck = [ordered]@{ status = "fail" }
$unverifiedCheck = [ordered]@{ status = "unverified" }
$flakyCheck = [ordered]@{ status = "flaky" }
$aggregate = Get-ValidationAggregate -Checks @()
Assert-ValidationContract -Condition ($aggregate.status -eq "not_run") -Message "not_run aggregation"
$aggregate = Get-ValidationAggregate -Checks @($passCheck)
Assert-ValidationContract -Condition ($aggregate.status -eq "pass") -Message "pass aggregation"
$aggregate = Get-ValidationAggregate -Checks @($passCheck, $unverifiedCheck)
Assert-ValidationContract -Condition ($aggregate.status -eq "unverified") -Message "unverified aggregation"
$aggregate = Get-ValidationAggregate -Checks @($passCheck) -ScopePartial
Assert-ValidationContract -Condition ($aggregate.status -eq "partial") -Message "partial aggregation"
$aggregate = Get-ValidationAggregate -Checks @($passCheck, $flakyCheck)
Assert-ValidationContract -Condition ($aggregate.status -eq "flaky") -Message "flaky aggregation"
$aggregate = Get-ValidationAggregate -Checks @($passCheck, $failCheck) -ScopePartial
Assert-ValidationContract -Condition ($aggregate.status -eq "fail") -Message "failure precedence"

$schema = Get-Content -LiteralPath $schemaPath -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-ValidationContract -Condition ($schema.properties.schema_id.const -eq "star.project-validation-report") -Message "schema id"
Assert-ValidationContract -Condition ($schema.properties.schema_version.const -eq 1) -Message "schema version"
$statusValues = @($schema.'$defs'.status.enum)
foreach ($status in @("pass", "fail", "not_run", "partial", "unverified", "flaky")) {
    Assert-ValidationContract -Condition ($status -in $statusValues) -Message "status enum $status"
}
foreach ($field in @("revision", "branch", "rust", "cargo", "python", "pyyaml", "git", "powershell", "platform")) {
    Assert-ValidationContract -Condition ($field -in @($schema.properties.environment.required)) -Message "environment field $field"
}

$requirements = Get-Content -LiteralPath (Join-Path $PSScriptRoot "requirements-validation.txt") -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($requirements -match "PyYAML==6\.0\.3") -Message "PyYAML pin"
Assert-ValidationContract -Condition (([regex]::Matches($requirements, "sha256:[0-9a-f]{64}")).Count -ge 2) -Message "PyYAML hashes"

$workflow = Get-Content -LiteralPath (Join-Path $repositoryRoot ".github/workflows/full.yml") -Raw -Encoding UTF8
$rustToolchain = Get-Content -LiteralPath (Join-Path $repositoryRoot "rust-toolchain.toml") -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($workflow.Contains("./scripts/validate.ps1 @parameters")) -Message "native validator is the CI gate"
Assert-ValidationContract -Condition (([regex]::Matches($workflow, "\./scripts/validate\.ps1")).Count -eq 1) -Message "native validator runs once"
Assert-ValidationContract -Condition (-not $workflow.Contains("continue-on-error: true")) -Message "validation gate must be authoritative"
Assert-ValidationContract -Condition ($workflow.Contains("uses: dtolnay/rust-toolchain@1.96.0")) -Message "CI uses the exact Rust toolchain pin"
Assert-ValidationContract -Condition ($workflow.Contains("components: rustfmt, clippy, rust-analyzer, rust-src")) -Message "CI installs every required Rust component"
Assert-ValidationContract -Condition ($rustToolchain.Contains('channel = "1.96.0"')) -Message "repository Rust toolchain pin"
foreach ($component in @("rustfmt", "clippy", "rust-analyzer", "rust-src")) {
    Assert-ValidationContract -Condition ($rustToolchain.Contains("`"$component`"")) -Message "repository Rust component $component"
}
foreach ($removedDuplicate in @(
        "cargo fmt --all -- --check",
        "cargo check --workspace --all-targets --locked",
        "cargo test --workspace --locked",
        "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings",
        "cargo run --locked -p star-schema-gen -- --check",
        "cargo run --locked -p star-matrix-check",
        "Observe validate.ps1 shadow"
    )) {
    Assert-ValidationContract -Condition (-not $workflow.Contains($removedDuplicate)) -Message "duplicate CI command removed: $removedDuplicate"
}

$commonSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot "common.ps1") -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($commonSource.Contains('$startInfo.CreateNoWindow = $true')) -Message "validator child processes must not allocate console windows"
Assert-ValidationContract -Condition ($commonSource.Contains('$startInfo.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden')) -Message "validator child windows must stay hidden"
Assert-ValidationContract -Condition ($commonSource.Contains('$process.Dispose()')) -Message "validator process handles must be disposed"
Assert-ValidationContract -Condition ($commonSource.Contains('$streams += "[stderr]') -and $commonSource.Contains('$streams += "[stdout]')) -Message "validation failure summary preserves both process streams"

$projectSource = Get-Content -LiteralPath (Join-Path $repositoryRoot "crates/control/star-project/src/lib.rs") -Raw -Encoding UTF8
$projectCatalogSource = Get-Content -LiteralPath (Join-Path $repositoryRoot "crates/control/star-project/src/catalog.rs") -Raw -Encoding UTF8
$planningSource = Get-Content -LiteralPath (Join-Path $repositoryRoot "apps/star-controller/src/validation_planning.rs") -Raw -Encoding UTF8
$cacheSource = Get-Content -LiteralPath (Join-Path $repositoryRoot "apps/star-controller/src/validation_cache.rs") -Raw -Encoding UTF8
$runOutputSchema = Get-Content -LiteralPath (Join-Path $repositoryRoot "catalog/tool-packages/schemas/validation-run-output.schema.json") -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-ValidationContract -Condition ($projectSource.Contains('command.creation_flags(0x0800_0000)')) -Message "project Git observation must hide child consoles"
Assert-ValidationContract -Condition (([regex]::Matches($projectSource, 'Command::new')).Count -eq 1) -Message "project commands must use the hidden command factory"
Assert-ValidationContract -Condition (([regex]::Matches($projectCatalogSource, 'Command::new')).Count -eq 0) -Message "catalog Git commands must use the shared hidden command factory"
Assert-ValidationContract -Condition ($planningSource.Contains('command.creation_flags(0x0800_0000)')) -Message "validation planning observations must hide child consoles"
Assert-ValidationContract -Condition (([regex]::Matches($planningSource, 'Command::new')).Count -eq 1) -Message "validation planning commands must use the hidden command factory"
Assert-ValidationContract -Condition ($cacheSource.Contains('target/validation/star-control-cache')) -Message "cache stays under ignored validation artifacts"
Assert-ValidationContract -Condition ($cacheSource.Contains('artifact_hashes')) -Message "cache binds every native artifact hash"
Assert-ValidationContract -Condition ($cacheSource.Contains('ValidationOutcome::Pass')) -Message "cache requires a pass ValidationRun"
Assert-ValidationContract -Condition ('cache' -in @($runOutputSchema.required)) -Message "validation run output reports cache disposition"
Assert-ValidationContract -Condition ($runOutputSchema.properties.cache.properties.hit.type -eq 'boolean') -Message "cache hit is machine readable"

$packagingSource = Get-Content -LiteralPath (Join-Path $repositoryRoot "packaging/windows/build-installer.ps1") -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($packagingSource.Contains('$startInfo.CreateNoWindow = $true')) -Message "packaging child processes must not allocate console windows"
Assert-ValidationContract -Condition ($packagingSource.Contains('$startInfo.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden')) -Message "packaging child windows must stay hidden"
Assert-ValidationContract -Condition (-not [regex]::IsMatch($packagingSource, '(?m)^\s*&\s+(cargo|git|\$IsccPath)\b')) -Message "packaging must not bypass the hidden process runner"
Assert-ValidationContract -Condition ($packagingSource.Contains("'-p', 'star-updater', '--bin', 'star-updater'")) -Message "installer generation rebuilds star-updater.exe"

$packageReleaseSource = Get-Content -LiteralPath (Join-Path $repositoryRoot "tools/package-release/src/main.rs") -Raw -Encoding UTF8
foreach ($requiredCodexAsset in @(
    'marketplace-root/.agents/plugins/marketplace.json',
    'plugins/star-control/.codex-plugin/plugin.json',
    'plugins/star-control/.mcp.json',
    'plugins/star-control/hooks/hooks.json',
    'skills/star-control-operations/SKILL.md',
    'skills/star-control-operations/agents/openai.yaml',
    'skills/star-control-operations/references/routing-matrix.md',
    'skills/orchestrate-parallel-implementation/SKILL.md',
    'skills/orchestrate-parallel-implementation/agents/openai.yaml',
    'skills/orchestrate-parallel-implementation/references/decomposition.md',
    'skills/orchestrate-parallel-implementation/references/scheduling-and-lifecycle.md',
    'skills/orchestrate-parallel-implementation/references/workspace-and-integration.md',
    'skills/orchestrate-parallel-implementation/references/safety-and-validation.md',
    'skills/orchestrate-parallel-implementation/assets/worker-context-pack.md',
    'skills/orchestrate-parallel-implementation/assets/worker-report.md',
    'skills/orchestrate-parallel-implementation/assets/controller-report.md'
)) {
    Assert-ValidationContract -Condition ($packageReleaseSource.Contains($requiredCodexAsset)) -Message "release package requires Codex asset: $requiredCodexAsset"
}

$codexHookTemplate = Get-Content -LiteralPath (Join-Path $repositoryRoot 'integrations/codex-plugin-template/marketplace-root/plugins/star-control/hooks/hooks.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$codexHookNames = @($codexHookTemplate.hooks.PSObject.Properties.Name)
Assert-ValidationContract -Condition ('SessionEnd' -in $codexHookNames) -Message 'Codex integration observes the main session end lifecycle'
Assert-ValidationContract -Condition ($codexHookTemplate.hooks.SessionEnd[0].hooks[0].command -eq 'star hook session-end') -Message 'SessionEnd uses the installed Hook bridge'
Assert-ValidationContract -Condition ($codexHookTemplate.hooks.SessionEnd[0].hooks[0].timeout -eq 3) -Message 'SessionEnd stays within the Codex three-second host limit'
foreach ($unboundHook in @('PermissionRequest', 'PreCompact', 'PostCompact')) {
    Assert-ValidationContract -Condition ($unboundHook -notin $codexHookNames) -Message "Codex integration must not overstate an unbound Hook contract: $unboundHook"
}
$windowsInstallerSource = Get-Content -LiteralPath (Join-Path $repositoryRoot 'packaging/windows/star-control.iss') -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($windowsInstallerSource.Contains('including SessionStart and SessionEnd')) -Message 'Windows installer explains the complete trusted Hook set'
Assert-ValidationContract -Condition ($windowsInstallerSource.Contains('SessionStart와 SessionEnd를 포함한')) -Message 'Windows installer Korean notice explains the complete trusted Hook set'
Assert-ValidationContract -Condition ($windowsInstallerSource.Contains('Codex CLI /hooks browser')) -Message 'Windows installer names the actual Hook review surface'

$codexOperationsSkill = Get-Content -LiteralPath (Join-Path $repositoryRoot 'integrations/codex-plugin-template/marketplace-root/plugins/star-control/skills/star-control-operations/SKILL.md') -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($codexOperationsSkill.Contains('## 4. Code Health')) -Message 'Codex operations Skill routes current Code Health features'
Assert-ValidationContract -Condition ($codexOperationsSkill.Contains('## 5. Runtime and Codex integration updates')) -Message 'Codex operations Skill distinguishes Runtime and integration update lifecycles'
$codexPluginManifest = Get-Content -LiteralPath (Join-Path $repositoryRoot 'integrations/codex-plugin-template/marketplace-root/plugins/star-control/.codex-plugin/plugin.json') -Raw -Encoding UTF8 | ConvertFrom-Json
Assert-ValidationContract -Condition ($codexPluginManifest.description.Contains('code-health')) -Message 'Codex Plugin metadata exposes the current Code Health route'
$codexSkillAgent = Get-Content -LiteralPath (Join-Path $repositoryRoot 'integrations/codex-plugin-template/marketplace-root/plugins/star-control/skills/star-control-operations/agents/openai.yaml') -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($codexSkillAgent.Contains('code health')) -Message 'Codex Skill UI metadata exposes the current Code Health route'
$parallelSkillRoot = Join-Path $repositoryRoot 'integrations/codex-plugin-template/marketplace-root/plugins/star-control/skills/orchestrate-parallel-implementation'
$parallelSkill = Get-Content -LiteralPath (Join-Path $parallelSkillRoot 'SKILL.md') -Raw -Encoding UTF8
foreach ($requiredParallelContract in @(
    '중앙 작업 자체를 `create_goal`로 등록하지 않는다',
    'list_projects({})',
    'list_threads({limit: ...})',
    'model: "gpt-5.6-sol"',
    'thinking: "max"',
    'model: "gpt-5.6-terra"',
    'thinking: "high"',
    'BOOTSTRAP_ONLY bundle_id=<unique bundle_id>',
    'this is not a Bundle assignment',
    'complete Context Pack은 post-create identity',
    'target: {',
    'projectId: <list_projects projectId>',
    'environment: { type: "worktree" }',
    'goal_pursuit: required',
    'create_thread({',
    'wait_threads({ targets: [{ threadId, hostId, afterCursor }]',
    'read_thread({ threadId, hostId })',
    'send_message_to_thread({ threadId, prompt:',
    'clientThreadId',
    'THREAD_IDENTITY_CONFIRMED',
    'ACTIVATE_BUNDLE',
    'activation ACK와 Goal active',
    '0건 timeout 또는 복수 match',
    'SOL_REVIEW_PENDING',
    'EXISTING_GOAL_RESUMED',
    'awaiting_external_sol_review',
    'Sol 승인 전 `update_goal',
    '`VERIFIED`'
)) {
    Assert-ValidationContract -Condition ($parallelSkill.Contains($requiredParallelContract)) -Message "parallel implementation Skill contract: $requiredParallelContract"
}

$ParallelForwardScenarios = @(
    '일반 구현 요청은 새 Codex App thread 0건이며 current-task single-agent로 수행한다.',
    '명시 승인 create_thread bootstrap은 unique bundle_id, BOOTSTRAP_ONLY, prompt, target:{type:"project", projectId, environment:{type:"worktree"}}를 사용한다.',
    'direct threadId/hostId도 project/worktree identity를 확인한 뒤 같은 ACTIVATE_BUNDLE protocol로 activation한다.',
    'clientThreadId only는 bounded list_threads unique bundle_id + expected projectId + worktree/project identity exactly one resolve이며 timeout/복수 match는 controller BLOCKED다.',
    'activation 전에는 Bundle assignment가 아니며 create_goal/commentary/source mutation/test/commit 0건이고 activation ACK/Goal active 뒤에만 진행한다.',
    'same file/contract ownership은 한 Bundle로 묶고 shared contract conflict는 mutation 없이 controller에 보고한다.',
    'preexisting dirty paths와 owned worktree baseline/head/fingerprint를 보존하고 reset·clean·restore하지 않는다.',
    'Terra는 WORKER_COMPLETE 한 번 뒤 Sol review를 polling하지 않고 controller만 wait_threads/read_thread로 관찰한다.',
    '자동 Goal turn 3회 뒤 blocked는 bundle_state=WORKER_COMPLETE, review_state=pending, blocked_reason=awaiting_external_sol_review이며 실패·거절이 아니다.',
    'blocked 뒤 correction/approval은 same threadId의 send_message_to_thread로 existing Goal을 EXISTING_GOAL_RESUMED하며 새 create_goal을 만들지 않는다.',
    'exact baseline_sha/head_sha/diff_fingerprint Sol 승인 뒤에만 same Goal complete와 INTEGRATED를 허용한다.',
    '승인 없는 dependency 설치·삭제·push와 Sol combined review/final validation 전 VERIFIED 선언을 하지 않는다.'
)

function Test-ParallelActualThreadAliasGuard {
    param([Parameter(Mandatory)][string]$Content)
    return (-not (@('message', 'project') | Where-Object { $Content.Contains("create_thread({`n  $($_):") }))
}

function Test-ParallelForwardScenarios {
    param([Parameter(Mandatory)][string]$Content)
    $section = $Content.Split('## 필수 forward scenario', 2)[1].Split('## 완료 증거', 2)[0]
    $matches = @([regex]::Matches($section, '(?m)^(\d+)\. (.+)$'))
    if ($matches.Count -ne $ParallelForwardScenarios.Count) { return $false }
    for ($index = 0; $index -lt $ParallelForwardScenarios.Count; $index++) {
        if ([int]$matches[$index].Groups[1].Value -ne ($index + 1) -or $matches[$index].Groups[2].Value -ne $ParallelForwardScenarios[$index]) {
            return $false
        }
    }
    return $true
}
foreach ($forbiddenParallelApi in @('spawn_agent', 'followup_task', 'wait_agent', 'interrupt_agent')) {
    Assert-ValidationContract -Condition (-not $parallelSkill.Contains($forbiddenParallelApi)) -Message "parallel implementation Skill removes obsolete collaboration API: $forbiddenParallelApi"
}
$invalidThreadCallPatterns = @("create_thread({`n  message:", "create_thread({`n  project:")
$parallelComponents = @(Get-ChildItem -LiteralPath $parallelSkillRoot -File -Recurse | Where-Object { $_.FullName -notmatch '\\.git\\' })
Assert-ValidationContract -Condition ($parallelComponents.Count -eq 9) -Message 'parallel implementation Skill has exactly nine rendered components for actual-call validation'
foreach ($component in $parallelComponents) {
    $componentText = Get-Content -LiteralPath $component.FullName -Raw -Encoding UTF8
    Assert-ValidationContract -Condition (Test-ParallelActualThreadAliasGuard $componentText) -Message "parallel implementation rendered component accepts only schema create_thread fields: $($component.Name)"
    foreach ($invalidThreadCall in $invalidThreadCallPatterns) {
        $alias = if ($invalidThreadCall.Contains('message:')) { 'message' } else { 'project' }
        $negativeCandidate = "$componentText`ncreate_thread({`n  $alias`: <different invalid value for $($component.Name)>`n})"
        Assert-ValidationContract -Condition (-not (Test-ParallelActualThreadAliasGuard $negativeCandidate)) -Message "parallel implementation negative append is rejected by common alias guard: $($component.Name) $invalidThreadCall"
    }
}
$parallelAgent = Get-Content -LiteralPath (Join-Path $parallelSkillRoot 'agents/openai.yaml') -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($parallelAgent.Contains('allow_implicit_invocation: true')) -Message 'parallel implementation Skill is available by default'
Assert-ValidationContract -Condition ($parallelAgent.Contains('Use $orchestrate-parallel-implementation')) -Message 'parallel implementation Skill has an explicit invocation prompt'
Assert-ValidationContract -Condition (-not $parallelAgent.Contains('dependencies:')) -Message 'parallel implementation Skill does not invent a direct tool dependency'
$parallelLifecycle = Get-Content -LiteralPath (Join-Path $parallelSkillRoot 'references/scheduling-and-lifecycle.md') -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($parallelLifecycle.Contains('EXISTING_GOAL_RESUMED')) -Message 'Terra correction resumes the same existing goal'
Assert-ValidationContract -Condition ($parallelLifecycle.Contains('SOL_REVIEW_PENDING')) -Message 'worker complete has a non-failure Sol review pending state'
Assert-ValidationContract -Condition ($parallelLifecycle.Contains('awaiting_external_sol_review')) -Message 'automatic blocked state preserves pending external Sol review'
foreach ($requiredThreadLifecycle in @('list_projects', 'list_threads', 'create_thread', 'wait_threads', 'read_thread', 'send_message_to_thread', 'clientThreadId', 'threadId', 'hostId', 'afterCursor')) {
    Assert-ValidationContract -Condition ($parallelLifecycle.Contains($requiredThreadLifecycle)) -Message "parallel implementation lifecycle uses confirmed Codex App threads: $requiredThreadLifecycle"
}
foreach ($requiredBootstrapState in @('BOOTSTRAP_ONLY', 'THREAD_IDENTITY_CONFIRMED', 'ACTIVATE_BUNDLE', 'GOAL_ACTIVE', 'unique bundle_id + expected projectId + expected worktree/project identity', '0건 timeout 또는 복수 match는 controller `BLOCKED`')) {
    Assert-ValidationContract -Condition ($parallelLifecycle.Contains($requiredBootstrapState)) -Message "parallel implementation lifecycle preserves bootstrap activation: $requiredBootstrapState"
}
$parallelSafety = Get-Content -LiteralPath (Join-Path $parallelSkillRoot 'references/safety-and-validation.md') -Raw -Encoding UTF8
Assert-ValidationContract -Condition (Test-ParallelForwardScenarios $parallelSafety) -Message 'parallel implementation Skill keeps the exact numbered 1..12 forward scenario mapping'
for ($index = 0; $index -lt $ParallelForwardScenarios.Count; $index++) {
    $number = $index + 1
    $expected = $ParallelForwardScenarios[$index]
    $negativeCandidate = $parallelSafety.Replace("$number. $expected", "$number. tampered scenario semantic") + "`n$expected"
    Assert-ValidationContract -Condition (-not (Test-ParallelForwardScenarios $negativeCandidate)) -Message "parallel implementation scenario $number semantic mutation is rejected"
}

$controllerStartupSource = Get-Content -LiteralPath (Join-Path $repositoryRoot 'apps/star-controller/src/main.rs') -Raw -Encoding UTF8
$managementSpawnIndex = $controllerStartupSource.IndexOf('let management_runtime = spawn_management_runtime')
$pipeStartIndex = $controllerStartupSource.IndexOf('PipeAcceptPool::start(pipe.clone())')
Assert-ValidationContract -Condition ($managementSpawnIndex -ge 0 -and $pipeStartIndex -gt $managementSpawnIndex) -Message 'Controller schedules management recovery and retention before opening the IPC pool without waiting for it'
Assert-ValidationContract -Condition ($controllerStartupSource.Contains('service.recover_incomplete_registrations()')) -Message 'background management startup preserves incomplete registration recovery'
Assert-ValidationContract -Condition ($controllerStartupSource.Contains('service.apply_retention(')) -Message 'background management startup preserves startup retention application'
Assert-ValidationContract -Condition ($controllerStartupSource.Contains('"MANAGEMENT_STORE_BUSY"') -and $controllerStartupSource.Contains('MANAGEMENT_INITIALIZING_MESSAGE')) -Message 'management requests receive a typed busy state during startup'
Assert-ValidationContract -Condition ($controllerStartupSource.Contains('matches!(code, "TOOL_PROCESS_RETRYABLE" | "MANAGEMENT_STORE_BUSY")')) -Message 'management startup busy state is retryable across direct and Tool action lanes'

$updaterRestartSource = Get-Content -LiteralPath (Join-Path $repositoryRoot "crates/control/star-updater-core/src/integration_restart.rs") -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($updaterRestartSource.Contains('const FORCED_CLOSE_TIMEOUT: Duration = Duration::from_secs(12);')) -Message "forced Codex termination has a bounded exit-observation window"
Assert-ValidationContract -Condition ($updaterRestartSource.Contains('terminate_until_process_exit(')) -Message "forced Codex termination repeats exact bounded passes"
Assert-ValidationContract -Condition ($updaterRestartSource.Contains('terminate_verified_tree_best_effort_excluding(')) -Message "one inaccessible helper cannot prevent later exact-root termination"
Assert-ValidationContract -Condition ($updaterRestartSource.Contains('|| Ok(exact_image_instances(&snapshot()?, desktop).is_empty())')) -Message "forced Codex termination uses a fresh exact-image census as completion proof"
Assert-ValidationContract -Condition (([regex]::Matches($updaterRestartSource, 'close_codex_desktop\(&desktop\)\.await')).Count -eq 2) -Message "offline installer and integration restart share the exact close contract"
Assert-ValidationContract -Condition ($updaterRestartSource.Contains('abort_and_relaunch(&mut transaction, &receipt_request, &desktop);')) -Message "offline close failure relaunches the same Desktop"

$installerSource = Get-Content -LiteralPath (Join-Path $repositoryRoot "packaging/windows/star-control.iss") -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($installerSource.Contains('CloseApplications=no')) -Message "installer must not terminate active Codex"
Assert-ValidationContract -Condition (-not $installerSource.Contains('CloseApplications=force')) -Message "forced application termination must stay disabled"
Assert-ValidationContract -Condition ($installerSource.Contains('function PrepareToInstall(var NeedsRestart: Boolean): String;')) -Message "installer must run an offline preflight before copying files"
foreach ($offlineProcess in @('ChatGPT.exe', 'star-controller.exe', 'star-mcp.exe')) {
    Assert-ValidationContract -Condition ($installerSource.Contains($offlineProcess)) -Message "installer offline preflight process: $offlineProcess"
}

Write-Output "validation contract tests passed for $($config.Id)"
