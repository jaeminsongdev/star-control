Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$script:StarPackageMap = $null

function Get-StarPackageMap {
    param([Parameter(Mandatory)][string]$Root)
    if ($null -ne $script:StarPackageMap) {
        return $script:StarPackageMap
    }
    $map = [ordered]@{}
    $manifestResult = Invoke-ValidationGit -Root $Root -Arguments @("ls-files", "*Cargo.toml")
    foreach ($manifest in (Split-ValidationLines $manifestResult.Stdout)) {
        $absolute = Join-Path $Root $manifest
        $content = Get-Content -LiteralPath $absolute -Raw -Encoding UTF8
        if ($content -match '(?ms)^\[package\].*?^name\s*=\s*"([^"]+)"') {
            $directory = ConvertTo-ValidationPath ([IO.Path]::GetDirectoryName($manifest))
            $map[$directory] = $Matches[1]
        }
    }
    $script:StarPackageMap = $map
    return $map
}

function Get-StarValidationImpact {
    param([Parameter(Mandatory)][string]$Path)
    $normalized = ConvertTo-ValidationPath $Path
    if (
        $normalized -in @("Cargo.toml", "Cargo.lock", "rust-toolchain.toml") -or
        $normalized.StartsWith(".star-control/", [StringComparison]::Ordinal) -or
        $normalized.StartsWith(".github/", [StringComparison]::Ordinal) -or
        $normalized.StartsWith("scripts/validation/", [StringComparison]::Ordinal) -or
        $normalized -eq "scripts/validate.ps1" -or
        $normalized.StartsWith("catalog/", [StringComparison]::Ordinal) -or
        $normalized.StartsWith("schemas/", [StringComparison]::Ordinal) -or
        $normalized.StartsWith("specs/", [StringComparison]::Ordinal) -or
        $normalized.StartsWith("docs/contracts/", [StringComparison]::Ordinal) -or
        $normalized.StartsWith("docs/features/", [StringComparison]::Ordinal) -or
        $normalized.StartsWith("crates/foundation/star-contracts/", [StringComparison]::Ordinal) -or
        $normalized.StartsWith("crates/foundation/star-ports/", [StringComparison]::Ordinal) -or
        $normalized.StartsWith("integrations/", [StringComparison]::Ordinal)
    ) {
        return "full"
    }
    if ($normalized.EndsWith(".rs", [StringComparison]::OrdinalIgnoreCase) -or
        $normalized.EndsWith("build.rs", [StringComparison]::OrdinalIgnoreCase)) {
        return "target"
    }
    if ([IO.Path]::GetExtension($normalized).ToLowerInvariant() -in @(".md", ".toml", ".json", ".jsonl", ".yaml", ".yml", ".lock")) {
        return "quick"
    }
    return "target"
}

function Get-StarValidationUnit {
    param(
        [Parameter(Mandatory)]$PackageMap,
        [Parameter(Mandatory)][string]$Path
    )
    $normalized = $Path.Replace([IO.Path]::DirectorySeparatorChar, "/").Replace("\", "/")
    while ($normalized.StartsWith("./", [StringComparison]::Ordinal)) {
        $normalized = $normalized.Substring(2)
    }
    foreach ($directory in @($PackageMap.Keys | Sort-Object Length -Descending)) {
        if ($normalized -eq "$directory/Cargo.toml" -or $normalized.StartsWith("$directory/", [StringComparison]::Ordinal)) {
            return $PackageMap[$directory]
        }
    }
    if ($normalized.StartsWith("docs/", [StringComparison]::Ordinal) -or
        [IO.Path]::GetExtension($normalized).ToLowerInvariant() -in @(".md", ".json", ".jsonl", ".toml", ".yaml", ".yml")) {
        return "docs"
    }
    return "workspace"
}

function Add-StarDiffChecks {
    param(
        [Parameter(Mandatory)][Collections.Generic.List[object]]$Checks,
        [Parameter(Mandatory)]$Context
    )
    [void]$Checks.Add((New-ValidationCheckSpec -Id "diff-worktree" -Unit "project" -Executable "git" -Arguments @("diff", "--check") -WorkingDirectory $Context.Root))
    [void]$Checks.Add((New-ValidationCheckSpec -Id "diff-staged" -Unit "project" -Executable "git" -Arguments @("diff", "--cached", "--check") -WorkingDirectory $Context.Root))
    if (-not [string]::IsNullOrWhiteSpace($Context.BaseRef)) {
        [void]$Checks.Add((New-ValidationCheckSpec -Id "diff-base" -Unit "project" -Executable "git" -Arguments @("diff", "--check", "$($Context.BaseRef)...HEAD") -WorkingDirectory $Context.Root))
    }
}

function Add-StarTargetCargoChecks {
    param(
        [Parameter(Mandatory)][Collections.Generic.List[object]]$Checks,
        [Parameter(Mandatory)]$Context,
        [string[]]$Packages = @()
    )
    $packageArguments = [Collections.Generic.List[string]]::new()
    foreach ($package in @($Packages | Sort-Object -Unique)) {
        $packageArguments.Add("-p")
        $packageArguments.Add($package)
    }
    $unit = if ($Packages.Count -eq 1) { $Packages[0] } elseif ($Packages.Count -gt 1) { "affected-packages" } else { "workspace" }
    $fmtArguments = @("fmt") + @($packageArguments) + @("--", "--check")
    $checkArguments = @("check") + @($packageArguments) + @("--all-targets", "--locked")
    $testArguments = @("test") + @($packageArguments) + @("--locked")
    $clippyArguments = @("clippy") + @($packageArguments) + @("--all-targets", "--locked", "--", "-D", "warnings")
    [void]$Checks.Add((New-ValidationCheckSpec -Id "cargo-fmt" -Unit $unit -Executable "cargo" -Arguments $fmtArguments -WorkingDirectory $Context.Root))
    [void]$Checks.Add((New-ValidationCheckSpec -Id "cargo-check" -Unit $unit -Executable "cargo" -Arguments $checkArguments -WorkingDirectory $Context.Root))
    [void]$Checks.Add((New-ValidationCheckSpec -Id "cargo-test" -Unit $unit -Executable "cargo" -Arguments $testArguments -WorkingDirectory $Context.Root))
    [void]$Checks.Add((New-ValidationCheckSpec -Id "cargo-clippy" -Unit $unit -Executable "cargo" -Arguments $clippyArguments -WorkingDirectory $Context.Root))
}

function Get-StarValidationChecks {
    param([Parameter(Mandatory)]$Context)
    $checks = [Collections.Generic.List[object]]::new()
    $checker = Join-Path $Context.Root "scripts/validation/check_files.py"
    [void]$checks.Add((New-ValidationCheckSpec -Id "static-files" -Unit "project" -Executable "python" -Arguments @("-X", "utf8", $checker, "--root", $Context.Root, "--paths-file", $Context.PathsFile) -WorkingDirectory $Context.Root -UnverifiedExitCodes @(3)))
    Add-StarDiffChecks -Checks $checks -Context $Context
    if ($Context.Profile -eq "quick") {
        return @($checks)
    }
    if ($Context.Unit -eq "docs") {
        [void]$checks.Add((New-UnverifiedValidationCheckSpec -Id "promoted-docs-scope" -Unit "docs" -Reason "the selected docs unit cannot satisfy the promoted code or contract profile" -WorkingDirectory $Context.Root))
        return @($checks)
    }

    $explicitPackage = -not [string]::IsNullOrWhiteSpace($Context.Unit) -and $Context.Unit -notin @("docs", "workspace")
    if ($Context.Profile -in @("full", "release") -and -not $explicitPackage) {
        [void]$checks.Add((New-ValidationCheckSpec -Id "cargo-fmt" -Unit "workspace" -Executable "cargo" -Arguments @("fmt", "--all", "--", "--check") -WorkingDirectory $Context.Root))
        [void]$checks.Add((New-ValidationCheckSpec -Id "cargo-check" -Unit "workspace" -Executable "cargo" -Arguments @("check", "--workspace", "--all-targets", "--locked") -WorkingDirectory $Context.Root))
        [void]$checks.Add((New-ValidationCheckSpec -Id "cargo-test" -Unit "workspace" -Executable "cargo" -Arguments @("test", "--workspace", "--locked") -WorkingDirectory $Context.Root))
        [void]$checks.Add((New-ValidationCheckSpec -Id "cargo-clippy" -Unit "workspace" -Executable "cargo" -Arguments @("clippy", "--workspace", "--all-targets", "--all-features", "--locked", "--", "-D", "warnings") -WorkingDirectory $Context.Root))
        [void]$checks.Add((New-ValidationCheckSpec -Id "schema-check" -Unit "contracts" -Executable "cargo" -Arguments @("run", "--locked", "-p", "star-schema-gen", "--", "--check") -WorkingDirectory $Context.Root))
        [void]$checks.Add((New-ValidationCheckSpec -Id "mcp-matrix" -Unit "contracts" -Executable "cargo" -Arguments @("run", "--locked", "-p", "star-matrix-check") -WorkingDirectory $Context.Root))
    } else {
        $packages = if ($explicitPackage) {
            @($Context.Unit)
        } else {
            @($Context.AffectedUnits | Where-Object { $_ -notin @("docs", "workspace") })
        }
        Add-StarTargetCargoChecks -Checks $checks -Context $Context -Packages $packages
        if ($Context.Profile -in @("full", "release")) {
            [void]$checks.Add((New-UnverifiedValidationCheckSpec -Id "full-unit-consumers" -Unit $Context.Unit -Reason "FULL was requested for one unit; reverse consumers and workspace conformance were not run" -WorkingDirectory $Context.Root))
        }
    }
    if ($Context.Profile -eq "release") {
        [void]$checks.Add((New-ValidationCheckSpec -Id "cargo-release-build" -Unit "workspace" -Executable "cargo" -Arguments @("build", "--workspace", "--release", "--locked") -WorkingDirectory $Context.Root))
        [void]$checks.Add((New-UnverifiedValidationCheckSpec -Id "release-platform-security-recovery" -Unit "release" -Reason "cross-platform installer, security, recovery, and signed artifact gates are not implemented in this project runner" -WorkingDirectory $Context.Root))
    }
    return @($checks)
}

function New-ProjectValidationConfig {
    param([Parameter(Mandatory)][string]$Root)
    $resolvedRoot = [IO.Path]::GetFullPath($Root)
    $packageMap = Get-StarPackageMap -Root $resolvedRoot
    $validUnits = @("docs", "workspace") + @($packageMap.Values)
    $resolveUnit = ${function:Get-StarValidationUnit}
    return @{
        Id = "star-control"
        Root = $resolvedRoot
        ArtifactBase = "target/validation"
        IgnoredPathPrefixes = @("target", "dist", "legacy", "--check")
        FingerprintFiles = @(
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "scripts/validate.ps1",
            "scripts/validation/common.ps1",
            "scripts/validation/project.ps1",
            "scripts/validation/project-validation-report.schema.json",
            "scripts/validation/requirements-validation.txt",
            "scripts/validation/invoke-shadow-validation.ps1",
            "scripts/validation/shadow_compare.py",
            "scripts/validation/shadow-contract.json",
            ".github/workflows/full.yml"
        )
        ClassifyPath = { param($Path) Get-StarValidationImpact -Path $Path }
        ClassifyUnit = { param($Unit) if ($Unit -eq "docs") { "quick" } else { "target" } }
        ResolveUnit = ({ param($Path) & $resolveUnit -PackageMap $packageMap -Path $Path }).GetNewClosure()
        ValidateUnit = ({ param($Unit) $Unit -in $validUnits }).GetNewClosure()
        BuildChecks = { param($Context) Get-StarValidationChecks -Context $Context }
        ContractExamples = @(
            @{ Path = "docs/README.md"; Profile = "quick" },
            @{ Path = "apps/star-cli/src/main.rs"; Profile = "target" },
            @{ Path = "crates/foundation/star-contracts/src/lib.rs"; Profile = "full" },
            @{ Path = "catalog/tool-packages/star-control-core.toml"; Profile = "full" },
            @{ Path = "Cargo.lock"; Profile = "full" }
        )
        ContractUnitExamples = @(
            @{ Unit = "docs"; Profile = "quick" },
            @{ Unit = "star-cli"; Profile = "target" }
        )
    }
}
