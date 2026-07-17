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
Assert-ValidationContract -Condition ($workflow.Contains("./scripts/validate.ps1 @arguments")) -Message "native validator is the CI gate"
Assert-ValidationContract -Condition (([regex]::Matches($workflow, "\./scripts/validate\.ps1")).Count -eq 1) -Message "native validator runs once"
Assert-ValidationContract -Condition (-not $workflow.Contains("continue-on-error: true")) -Message "validation gate must be authoritative"
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

$installerSource = Get-Content -LiteralPath (Join-Path $repositoryRoot "packaging/windows/star-control.iss") -Raw -Encoding UTF8
Assert-ValidationContract -Condition ($installerSource.Contains('CloseApplications=no')) -Message "installer must not terminate active Codex"
Assert-ValidationContract -Condition (-not $installerSource.Contains('CloseApplications=force')) -Message "forced application termination must stay disabled"
Assert-ValidationContract -Condition ($installerSource.Contains('function PrepareToInstall(var NeedsRestart: Boolean): String;')) -Message "installer must run an offline preflight before copying files"
foreach ($offlineProcess in @('ChatGPT.exe', 'star-controller.exe', 'star-mcp.exe')) {
    Assert-ValidationContract -Condition ($installerSource.Contains($offlineProcess)) -Message "installer offline preflight process: $offlineProcess"
}

Write-Output "validation contract tests passed for $($config.Id)"
