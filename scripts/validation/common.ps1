Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$script:ValidationProfiles = @("quick", "target", "full", "release")

function New-ValidationEntryError {
    param(
        [Parameter(Mandatory)][ValidateSet("invocation", "runner")][string]$Kind,
        [Parameter(Mandatory)][ValidateSet("fail", "unverified")][string]$Status,
        [Parameter(Mandatory)][string]$Message,
        [Parameter(Mandatory)][int]$ExitCode
    )
    return [ordered]@{
        schema_id = "star.project-validation-entry-error"
        schema_version = 1
        status = $Status
        error_kind = $Kind
        message = $Message
        exit_code = $ExitCode
    }
}

function ConvertTo-ValidationPath {
    param([Parameter(Mandatory)][string]$Path)
    $normalized = $Path.Replace([IO.Path]::DirectorySeparatorChar, "/").Replace("\", "/")
    while ($normalized.StartsWith("./", [StringComparison]::Ordinal)) {
        $normalized = $normalized.Substring(2)
    }
    return $normalized
}

function Get-ValidationSha256 {
    param([Parameter(Mandatory)][string]$Text)
    $bytes = [Text.Encoding]::UTF8.GetBytes($Text)
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
}

function Get-ValidationProfileRank {
    param([Parameter(Mandatory)][string]$Profile)
    $rank = [Array]::IndexOf($script:ValidationProfiles, $Profile.ToLowerInvariant())
    if ($rank -lt 0) {
        throw [ArgumentException]::new("unknown validation profile: $Profile")
    }
    return $rank
}

function Get-MaxValidationProfile {
    param([Parameter(Mandatory)][string[]]$Profiles)
    if ($Profiles.Count -eq 0) {
        return "quick"
    }
    return $Profiles | Sort-Object { Get-ValidationProfileRank $_ } | Select-Object -Last 1
}

function Resolve-ValidationProfile {
    param(
        [Parameter(Mandatory)][string]$RequestedProfile,
        [Parameter(Mandatory)][string]$RequiredProfile
    )
    if ($RequestedProfile -eq "release") {
        return "release"
    }
    if ($RequestedProfile -eq "full") {
        return "full"
    }
    # quick과 target은 영향도 판정 결과를 따른다. full과 release만 명시적 하향 금지다.
    return $RequiredProfile
}

function Invoke-ValidationProcessCapture {
    param(
        [Parameter(Mandatory)][string]$Executable,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory)][string]$WorkingDirectory
    )
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $Executable
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $startInfo.StandardErrorEncoding = [Text.UTF8Encoding]::new($false)
    foreach ($argument in $Arguments) {
        [void]$startInfo.ArgumentList.Add([string]$argument)
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    try {
        [void]$process.Start()
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        return [pscustomobject]@{
            ExitCode = $process.ExitCode
            Stdout = $stdoutTask.GetAwaiter().GetResult()
            Stderr = $stderrTask.GetAwaiter().GetResult()
        }
    }
    finally {
        $process.Dispose()
    }
}

function Invoke-ValidationGit {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string[]]$Arguments,
        [switch]$AllowFailure
    )
    $allArguments = @("-c", "core.quotepath=false", "-C", $Root) + $Arguments
    $result = Invoke-ValidationProcessCapture -Executable "git" -Arguments $allArguments -WorkingDirectory $Root
    if ($result.ExitCode -ne 0 -and -not $AllowFailure) {
        $message = $result.Stderr.Trim()
        if ([string]::IsNullOrWhiteSpace($message)) {
            $message = "git exited with code $($result.ExitCode)"
        }
        throw [InvalidOperationException]::new($message)
    }
    return $result
}

function Split-ValidationLines {
    param([AllowEmptyString()][string]$Text)
    if ([string]::IsNullOrWhiteSpace($Text)) {
        return @()
    }
    return @([regex]::Split($Text.TrimEnd(), "\r?\n") | Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        })
}

function Test-ValidationIgnoredPath {
    param(
        [Parameter(Mandatory)][string]$Path,
        [string[]]$IgnoredPrefixes = @()
    )
    $normalized = ConvertTo-ValidationPath $Path
    foreach ($prefix in $IgnoredPrefixes) {
        $normalizedPrefix = (ConvertTo-ValidationPath $prefix).TrimEnd("/")
        if ($normalized -eq $normalizedPrefix -or $normalized.StartsWith("$normalizedPrefix/", [StringComparison]::Ordinal)) {
            return $true
        }
    }
    return $false
}

function Get-ValidationChangedPaths {
    param(
        [Parameter(Mandatory)][string]$Root,
        [string]$BaseRef,
        [string[]]$IgnoredPrefixes = @()
    )
    $set = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    if (-not [string]::IsNullOrWhiteSpace($BaseRef)) {
        $verified = Invoke-ValidationGit -Root $Root -Arguments @("rev-parse", "--verify", "$BaseRef^{commit}") -AllowFailure
        if ($verified.ExitCode -ne 0) {
            throw [ArgumentException]::new("BaseRef is not a local commit: $BaseRef")
        }
        $baseDiff = Invoke-ValidationGit -Root $Root -Arguments @("diff", "--name-only", "--diff-filter=ACDMRTUXB", "$BaseRef...HEAD")
        foreach ($path in (Split-ValidationLines $baseDiff.Stdout)) {
            [void]$set.Add((ConvertTo-ValidationPath $path))
        }
    }
    foreach ($arguments in @(
            @("diff", "--name-only", "--diff-filter=ACDMRTUXB"),
            @("diff", "--cached", "--name-only", "--diff-filter=ACDMRTUXB"),
            @("ls-files", "--others", "--exclude-standard")
        )) {
        $result = Invoke-ValidationGit -Root $Root -Arguments $arguments
        foreach ($path in (Split-ValidationLines $result.Stdout)) {
            [void]$set.Add((ConvertTo-ValidationPath $path))
        }
    }
    return @($set | Where-Object {
            -not (Test-ValidationIgnoredPath -Path $_ -IgnoredPrefixes $IgnoredPrefixes)
        } | Sort-Object)
}

function Get-AllValidationFiles {
    param(
        [Parameter(Mandatory)][string]$Root,
        [string[]]$IgnoredPrefixes = @()
    )
    $set = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($arguments in @(@("ls-files"), @("ls-files", "--others", "--exclude-standard"))) {
        $result = Invoke-ValidationGit -Root $Root -Arguments $arguments
        foreach ($path in (Split-ValidationLines $result.Stdout)) {
            [void]$set.Add((ConvertTo-ValidationPath $path))
        }
    }
    return @($set | Where-Object {
            -not (Test-ValidationIgnoredPath -Path $_ -IgnoredPrefixes $IgnoredPrefixes)
        } | Sort-Object)
}

function New-ValidationCheckSpec {
    param(
        [Parameter(Mandatory)][string]$Id,
        [Parameter(Mandatory)][string]$Unit,
        [Parameter(Mandatory)][string]$Executable,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory)][string]$WorkingDirectory,
        [int[]]$UnverifiedExitCodes = @(),
        [switch]$ExpectNoOutput,
        [bool]$Required = $true
    )
    return [pscustomobject]@{
        Id = $Id
        Unit = $Unit
        Executable = $Executable
        Arguments = @($Arguments)
        WorkingDirectory = $WorkingDirectory
        UnverifiedExitCodes = @($UnverifiedExitCodes)
        ExpectNoOutput = [bool]$ExpectNoOutput
        Required = $Required
        Disposition = "run"
        Reason = $null
    }
}

function New-UnverifiedValidationCheckSpec {
    param(
        [Parameter(Mandatory)][string]$Id,
        [Parameter(Mandatory)][string]$Unit,
        [Parameter(Mandatory)][string]$Reason,
        [Parameter(Mandatory)][string]$WorkingDirectory
    )
    return [pscustomobject]@{
        Id = $Id
        Unit = $Unit
        Executable = $null
        Arguments = @()
        WorkingDirectory = $WorkingDirectory
        UnverifiedExitCodes = @()
        ExpectNoOutput = $false
        Required = $true
        Disposition = "unverified"
        Reason = $Reason
    }
}

function Get-ValidationFailureSummary {
    param(
        [AllowEmptyString()][string]$Stdout,
        [AllowEmptyString()][string]$Stderr,
        [Parameter(Mandatory)][string]$Fallback
    )
    $streams = @()
    if (-not [string]::IsNullOrWhiteSpace($Stderr)) {
        $streams += "[stderr]$([Environment]::NewLine)$Stderr"
    }
    if (-not [string]::IsNullOrWhiteSpace($Stdout)) {
        $streams += "[stdout]$([Environment]::NewLine)$Stdout"
    }
    $text = $streams -join [Environment]::NewLine
    if ([string]::IsNullOrWhiteSpace($text)) {
        return $Fallback
    }
    $tailLines = @([regex]::Split($text.Trim(), "\r?\n") | Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        } | Select-Object -Last 16)
    $oneLine = [regex]::Replace(($tailLines -join " "), "\s+", " ")
    if ($oneLine.Length -gt 600) {
        return $oneLine.Substring($oneLine.Length - 600)
    }
    return $oneLine
}

function Invoke-ValidationCheck {
    param(
        [Parameter(Mandatory)]$Spec,
        [Parameter(Mandatory)][string]$ArtifactDirectory,
        [Parameter(Mandatory)][string]$ProjectRoot
    )
    $startedAt = [DateTimeOffset]::UtcNow
    $safeId = [regex]::Replace($Spec.Id, "[^A-Za-z0-9._-]", "_")
    $logPath = Join-Path $ArtifactDirectory "$safeId.log"
    $relativeLog = ConvertTo-ValidationPath ([IO.Path]::GetRelativePath($ProjectRoot, $logPath))
    if ($Spec.Disposition -eq "unverified") {
        [IO.File]::WriteAllText($logPath, "$($Spec.Reason)$([Environment]::NewLine)", [Text.UTF8Encoding]::new($false))
        return [ordered]@{
            id = $Spec.Id
            unit = $Spec.Unit
            required = [bool]$Spec.Required
            status = "unverified"
            outcome = "not_run"
            completeness = "unverified"
            stability = "not_evaluated"
            command = $null
            exit_code = $null
            started_at = $startedAt.ToString("O")
            finished_at = $startedAt.ToString("O")
            duration_ms = 0
            failure_summary = $Spec.Reason
            log_ref = $relativeLog
        }
    }

    $stopwatch = [Diagnostics.Stopwatch]::StartNew()
    $stdout = ""
    $stderr = ""
    $exitCode = $null
    $status = "fail"
    $outcome = "error"
    $completeness = "complete"
    $stability = "not_evaluated"
    $failureSummary = $null
    try {
        $result = Invoke-ValidationProcessCapture -Executable $Spec.Executable -Arguments $Spec.Arguments -WorkingDirectory $Spec.WorkingDirectory
        $stdout = $result.Stdout
        $stderr = $result.Stderr
        $exitCode = $result.ExitCode
        if ($exitCode -eq 0 -and $Spec.ExpectNoOutput -and -not [string]::IsNullOrWhiteSpace($stdout)) {
            $status = "fail"
            $outcome = "fail"
            $stability = "stable"
            $failureSummary = Get-ValidationFailureSummary -Stdout $stdout -Stderr $stderr -Fallback "command produced output but none was expected"
        } elseif ($exitCode -eq 0) {
            $status = "pass"
            $outcome = "pass"
            $stability = "stable"
        } elseif ($Spec.UnverifiedExitCodes -contains $exitCode) {
            $status = "unverified"
            $outcome = "not_run"
            $completeness = "unverified"
            $failureSummary = Get-ValidationFailureSummary -Stdout $stdout -Stderr $stderr -Fallback "required environment is unavailable"
        } else {
            $status = "fail"
            $outcome = "fail"
            $stability = "stable"
            $failureSummary = Get-ValidationFailureSummary -Stdout $stdout -Stderr $stderr -Fallback "command exited with code $exitCode"
        }
    } catch {
        $status = "unverified"
        $outcome = "not_run"
        $completeness = "unverified"
        $failureSummary = $_.Exception.Message
        $stderr = $_ | Out-String
    } finally {
        $stopwatch.Stop()
    }

    $commandLine = @($Spec.Executable) + @($Spec.Arguments)
    $logHeader = @(
        "command: $($commandLine -join ' ')",
        "cwd: $($Spec.WorkingDirectory)",
        "exit_code: $(if ($null -eq $exitCode) { 'null' } else { $exitCode })",
        "duration_ms: $($stopwatch.ElapsedMilliseconds)",
        "",
        "[stdout]",
        $stdout,
        "[stderr]",
        $stderr
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($logPath, $logHeader, [Text.UTF8Encoding]::new($false))
    $finishedAt = [DateTimeOffset]::UtcNow
    return [ordered]@{
        id = $Spec.Id
        unit = $Spec.Unit
        required = [bool]$Spec.Required
        status = $status
        outcome = $outcome
        completeness = $completeness
        stability = $stability
        command = [ordered]@{
            executable = $Spec.Executable
            arguments = @($Spec.Arguments)
            cwd = ConvertTo-ValidationPath ([IO.Path]::GetRelativePath($ProjectRoot, $Spec.WorkingDirectory))
        }
        exit_code = $exitCode
        started_at = $startedAt.ToString("O")
        finished_at = $finishedAt.ToString("O")
        duration_ms = $stopwatch.ElapsedMilliseconds
        failure_summary = $failureSummary
        log_ref = $relativeLog
    }
}

function Get-ValidationAggregate {
    param(
        [object[]]$Checks = @(),
        [switch]$ScopePartial
    )
    if ($Checks.Count -eq 0) {
        return [ordered]@{
            status = "not_run"
            outcome = "not_run"
            completeness = "unverified"
            stability = "not_evaluated"
        }
    }
    $statuses = @($Checks | ForEach-Object { $_.status })
    $hasFailure = $statuses -contains "fail"
    $hasFlaky = $statuses -contains "flaky"
    $hasUnverified = $statuses -contains "unverified"
    $hasPass = $statuses -contains "pass"
    $outcome = if ($hasFailure) { "fail" } elseif ($hasPass -or $hasFlaky) { "pass" } else { "not_run" }
    $completeness = if ($ScopePartial) { "partial" } elseif ($hasUnverified) { "unverified" } else { "complete" }
    $stability = if ($hasFlaky) { "flaky" } elseif ($hasPass -or $hasFailure) { "stable" } else { "not_evaluated" }
    $status = if ($hasFailure) {
        "fail"
    } elseif ($hasFlaky) {
        "flaky"
    } elseif ($ScopePartial) {
        "partial"
    } elseif ($hasUnverified) {
        "unverified"
    } elseif (-not $hasPass) {
        "not_run"
    } else {
        "pass"
    }
    return [ordered]@{
        status = $status
        outcome = $outcome
        completeness = $completeness
        stability = $stability
    }
}

function Get-ValidationToolVersion {
    param(
        [Parameter(Mandatory)][string]$Executable,
        [string[]]$Arguments = @("--version"),
        [Parameter(Mandatory)][string]$Root
    )
    try {
        $result = Invoke-ValidationProcessCapture -Executable $Executable -Arguments $Arguments -WorkingDirectory $Root
        if ($result.ExitCode -eq 0) {
            return $result.Stdout.Trim()
        }
    } catch {
    }
    return $null
}

function Get-ValidationPythonPackageVersion {
    param(
        [Parameter(Mandatory)][string]$Package,
        [Parameter(Mandatory)][string]$Root
    )
    $program = "import importlib.metadata; print(importlib.metadata.version('$Package'))"
    return Get-ValidationToolVersion -Executable "python" -Arguments @("-c", $program) -Root $Root
}

function ConvertTo-ValidationFingerprintToken {
    param(
        [AllowNull()]$Value,
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$ArtifactDirectory
    )
    if ($null -eq $Value) {
        return "<null>"
    }
    $normalized = ([string]$Value).Replace("\", "/")
    $rootPrefix = ([IO.Path]::GetFullPath($Root)).Replace("\", "/").TrimEnd("/")
    $artifactPrefix = ([IO.Path]::GetFullPath($ArtifactDirectory)).Replace("\", "/").TrimEnd("/")
    foreach ($mapping in @(
            @{ Prefix = $artifactPrefix; Token = "<artifact>" },
            @{ Prefix = $rootPrefix; Token = "<root>" }
        )) {
        if ($normalized -eq $mapping.Prefix) {
            return $mapping.Token
        }
        if ($normalized.StartsWith("$($mapping.Prefix)/", [StringComparison]::Ordinal)) {
            return $mapping.Token + $normalized.Substring($mapping.Prefix.Length)
        }
    }
    return $normalized
}

function Get-ValidationInputFingerprint {
    param(
        [Parameter(Mandatory)]$Config,
        [Parameter(Mandatory)][object[]]$Checks,
        [Parameter(Mandatory)][string]$Head,
        [AllowNull()][string]$RustVersion,
        [AllowNull()][string]$CargoVersion,
        [AllowNull()][string]$PythonVersion,
        [AllowNull()][string]$PyYamlVersion,
        [AllowNull()][string]$GitVersion,
        [Parameter(Mandatory)][string]$PowerShellVersion,
        [Parameter(Mandatory)][string]$Platform,
        [Parameter(Mandatory)][string]$RequestedProfile,
        [Parameter(Mandatory)][string]$RequiredProfile,
        [Parameter(Mandatory)][string]$EffectiveProfile,
        [AllowNull()][string]$Unit,
        [AllowNull()][string]$BaseRef,
        [string[]]$ChangedPaths = @(),
        [string[]]$ValidationFiles = @(),
        [Parameter(Mandatory)][string]$ArtifactDirectory
    )
    $root = $Config.Root
    $dirty = Invoke-ValidationGit -Root $root -Arguments @("diff", "--binary", "HEAD")
    $untracked = Invoke-ValidationGit -Root $root -Arguments @("ls-files", "--others", "--exclude-standard")
    $parts = [Collections.Generic.List[string]]::new()
    $parts.Add("project=$($Config.Id)")
    $parts.Add("revision=$Head")
    $parts.Add("dirty=$($dirty.Stdout)")
    $parts.Add("rust=$RustVersion")
    $parts.Add("cargo=$CargoVersion")
    $parts.Add("python=$PythonVersion")
    $parts.Add("pyyaml=$PyYamlVersion")
    $parts.Add("git=$GitVersion")
    $parts.Add("powershell=$PowerShellVersion")
    $parts.Add("platform=$Platform")
    $parts.Add("requested_profile=$RequestedProfile")
    $parts.Add("required_profile=$RequiredProfile")
    $parts.Add("effective_profile=$EffectiveProfile")
    $parts.Add("unit=$(ConvertTo-ValidationFingerprintToken -Value $Unit -Root $root -ArtifactDirectory $ArtifactDirectory)")
    $parts.Add("base_ref=$(ConvertTo-ValidationFingerprintToken -Value $BaseRef -Root $root -ArtifactDirectory $ArtifactDirectory)")
    foreach ($path in @($ChangedPaths | Sort-Object -Unique)) {
        $parts.Add("changed:$path")
    }
    foreach ($path in @($ValidationFiles | Sort-Object -Unique)) {
        $parts.Add("validation_file:$path")
    }
    foreach ($path in (Split-ValidationLines $untracked.Stdout | Sort-Object)) {
        if (Test-ValidationIgnoredPath -Path $path -IgnoredPrefixes $Config.IgnoredPathPrefixes) {
            continue
        }
        $absolutePath = Join-Path $root $path
        if (Test-Path -LiteralPath $absolutePath -PathType Leaf) {
            $parts.Add("untracked:$path=$((Get-FileHash -LiteralPath $absolutePath -Algorithm SHA256).Hash)")
        }
    }
    foreach ($path in @($Config.FingerprintFiles | Sort-Object -Unique)) {
        $absolutePath = Join-Path $root $path
        $hash = if (Test-Path -LiteralPath $absolutePath -PathType Leaf) {
            (Get-FileHash -LiteralPath $absolutePath -Algorithm SHA256).Hash
        } else {
            "missing"
        }
        $parts.Add("config:$path=$hash")
    }
    foreach ($check in $Checks) {
        $executable = ConvertTo-ValidationFingerprintToken -Value $check.Executable -Root $root -ArtifactDirectory $ArtifactDirectory
        $arguments = @($check.Arguments | ForEach-Object {
                ConvertTo-ValidationFingerprintToken -Value $_ -Root $root -ArtifactDirectory $ArtifactDirectory
            })
        $workingDirectory = ConvertTo-ValidationFingerprintToken -Value $check.WorkingDirectory -Root $root -ArtifactDirectory $ArtifactDirectory
        $reason = ConvertTo-ValidationFingerprintToken -Value $check.Reason -Root $root -ArtifactDirectory $ArtifactDirectory
        $parts.Add("check:$($check.Id)=unit:$($check.Unit)|required:$($check.Required)|disposition:$($check.Disposition)|reason:$reason|executable:$executable|arguments:$($arguments -join [char]31)|cwd:$workingDirectory")
    }
    return Get-ValidationSha256 ($parts -join [char]30)
}

function Invoke-ProjectValidation {
    param(
        [Parameter(Mandatory)]$Config,
        [Parameter(Mandatory)][string]$Profile,
        [string]$Unit,
        [string]$BaseRef,
        [Parameter(Mandatory)][string]$OutputFormat
    )
    $root = [IO.Path]::GetFullPath($Config.Root)
    $startedAt = [DateTimeOffset]::UtcNow
    $stopwatch = [Diagnostics.Stopwatch]::StartNew()
    $inside = Invoke-ValidationGit -Root $root -Arguments @("rev-parse", "--is-inside-work-tree")
    if ($inside.Stdout.Trim() -ne "true") {
        throw [ArgumentException]::new("validation requires a Git worktree")
    }
    if (-not [string]::IsNullOrWhiteSpace($Unit)) {
        $valid = & $Config.ValidateUnit $Unit
        if (-not $valid) {
            throw [ArgumentException]::new("unknown unit: $Unit")
        }
    }

    $changedPaths = @(Get-ValidationChangedPaths -Root $root -BaseRef $BaseRef -IgnoredPrefixes $Config.IgnoredPathPrefixes)
    $noImpact = -not [string]::IsNullOrWhiteSpace($BaseRef) -and $changedPaths.Count -eq 0
    $wholeProject = [string]::IsNullOrWhiteSpace($BaseRef) -and $changedPaths.Count -eq 0
    $requiredProfiles = [Collections.Generic.List[string]]::new()
    $affectedUnits = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($path in $changedPaths) {
        $requiredProfiles.Add((& $Config.ClassifyPath $path))
        $resolvedUnit = & $Config.ResolveUnit $path
        if (-not [string]::IsNullOrWhiteSpace($resolvedUnit)) {
            [void]$affectedUnits.Add($resolvedUnit)
        }
    }
    $requiredProfile = if ($noImpact) {
        $Profile
    } elseif ($wholeProject -and -not [string]::IsNullOrWhiteSpace($Unit)) {
        & $Config.ClassifyUnit $Unit
    } elseif ($wholeProject) {
        $Profile
    } else {
        Get-MaxValidationProfile @($requiredProfiles)
    }
    $effectiveProfile = Resolve-ValidationProfile -RequestedProfile $Profile -RequiredProfile $requiredProfile

    $scopePartial = $false
    if (-not [string]::IsNullOrWhiteSpace($Unit) -and $changedPaths.Count -gt 0) {
        foreach ($path in $changedPaths) {
            $pathUnit = & $Config.ResolveUnit $path
            if ($pathUnit -ne $Unit) {
                $scopePartial = $true
                break
            }
        }
    }

    $allFiles = if (($effectiveProfile -in @("full", "release")) -and [string]::IsNullOrWhiteSpace($Unit)) {
        @(Get-AllValidationFiles -Root $root -IgnoredPrefixes $Config.IgnoredPathPrefixes)
    } elseif (-not [string]::IsNullOrWhiteSpace($Unit)) {
        $unitCandidates = if ($wholeProject) {
            @(Get-AllValidationFiles -Root $root -IgnoredPrefixes $Config.IgnoredPathPrefixes)
        } else {
            @($changedPaths)
        }
        @($unitCandidates | Where-Object { (& $Config.ResolveUnit $_) -eq $Unit })
    } elseif ($wholeProject) {
        @(Get-AllValidationFiles -Root $root -IgnoredPrefixes $Config.IgnoredPathPrefixes)
    } else {
        @($changedPaths)
    }

    $runId = "{0}-{1}" -f ([DateTimeOffset]::UtcNow.ToString("yyyyMMddTHHmmssfffZ")), $PID
    $artifactDirectory = Join-Path (Join-Path $root $Config.ArtifactBase) $runId
    [void](New-Item -ItemType Directory -Path $artifactDirectory -Force)
    $pathsFile = Join-Path $artifactDirectory "paths.json"
    [IO.File]::WriteAllText($pathsFile, (ConvertTo-Json -InputObject @($allFiles) -Depth 3), [Text.UTF8Encoding]::new($false))
    $context = [pscustomobject]@{
        Root = $root
        Profile = $effectiveProfile
        RequestedProfile = $Profile
        RequiredProfile = $requiredProfile
        Unit = $Unit
        BaseRef = $BaseRef
        ChangedPaths = @($changedPaths)
        ValidationFiles = @($allFiles)
        PathsFile = $pathsFile
        WholeProject = $wholeProject
        NoImpact = $noImpact
        AffectedUnits = @($affectedUnits | Sort-Object)
    }
    $specs = if ($noImpact) {
        @()
    } else {
        $contractSpec = New-ValidationCheckSpec -Id "validation-contract" -Unit "validation" -Executable "pwsh" -Arguments @("-NoProfile", "-File", (Join-Path $root "scripts/validation/contract-tests.ps1")) -WorkingDirectory $root
        @($contractSpec) + @(& $Config.BuildChecks $context)
    }
    if ($effectiveProfile -eq "release") {
        $specs += New-ValidationCheckSpec -Id "release-clean-worktree" -Unit "project" -Executable "git" -Arguments @("-c", "core.quotepath=false", "status", "--porcelain=v1", "--untracked-files=all") -WorkingDirectory $root -ExpectNoOutput
    }

    $head = (Invoke-ValidationGit -Root $root -Arguments @("rev-parse", "HEAD")).Stdout.Trim()
    $branch = (Invoke-ValidationGit -Root $root -Arguments @("branch", "--show-current")).Stdout.Trim()
    $rustVersion = Get-ValidationToolVersion -Executable "rustc" -Root $root
    $cargoVersion = Get-ValidationToolVersion -Executable "cargo" -Root $root
    $pythonVersion = Get-ValidationToolVersion -Executable "python" -Root $root
    $pyYamlVersion = Get-ValidationPythonPackageVersion -Package "PyYAML" -Root $root
    $gitVersion = Get-ValidationToolVersion -Executable "git" -Root $root
    $powerShellVersion = $PSVersionTable.PSVersion.ToString()
    $platform = "{0}|{1}" -f [Runtime.InteropServices.RuntimeInformation]::OSDescription, [Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    $fingerprint = Get-ValidationInputFingerprint -Config $Config -Checks $specs -Head $head -RustVersion $rustVersion -CargoVersion $cargoVersion -PythonVersion $pythonVersion -PyYamlVersion $pyYamlVersion -GitVersion $gitVersion -PowerShellVersion $powerShellVersion -Platform $platform -RequestedProfile $Profile -RequiredProfile $requiredProfile -EffectiveProfile $effectiveProfile -Unit $Unit -BaseRef $BaseRef -ChangedPaths $changedPaths -ValidationFiles $allFiles -ArtifactDirectory $artifactDirectory

    $checkResults = [Collections.Generic.List[object]]::new()
    foreach ($spec in $specs) {
        $checkResults.Add((Invoke-ValidationCheck -Spec $spec -ArtifactDirectory $artifactDirectory -ProjectRoot $root))
    }
    $aggregate = Get-ValidationAggregate -Checks @($checkResults) -ScopePartial:$scopePartial
    $stopwatch.Stop()
    $finishedAt = [DateTimeOffset]::UtcNow
    $failureSummary = @($checkResults | Where-Object { $_.status -ne "pass" -and $_.failure_summary } | ForEach-Object { "$($_.id): $($_.failure_summary)" } | Select-Object -First 5)
    $relativeArtifact = ConvertTo-ValidationPath ([IO.Path]::GetRelativePath($root, $artifactDirectory))
    $report = [ordered]@{
        schema_id = "star.project-validation-report"
        schema_version = 1
        project_id = $Config.Id
        requested_profile = $Profile
        required_profile = $requiredProfile
        effective_profile = $effectiveProfile
        unit = if ([string]::IsNullOrWhiteSpace($Unit)) { $null } else { $Unit }
        base_ref = if ([string]::IsNullOrWhiteSpace($BaseRef)) { $null } else { $BaseRef }
        status = $aggregate.status
        outcome = $aggregate.outcome
        completeness = $aggregate.completeness
        stability = $aggregate.stability
        started_at = $startedAt.ToString("O")
        finished_at = $finishedAt.ToString("O")
        duration_ms = $stopwatch.ElapsedMilliseconds
        input_fingerprint = $fingerprint
        environment = [ordered]@{
            revision = $head
            branch = $branch
            rust = $rustVersion
            cargo = $cargoVersion
            python = $pythonVersion
            pyyaml = $pyYamlVersion
            git = $gitVersion
            powershell = $powerShellVersion
            platform = $platform
        }
        impact = [ordered]@{
            changed_paths = @($changedPaths)
            affected_units = @($affectedUnits | Sort-Object)
            whole_project = $wholeProject
            scope_partial = $scopePartial
        }
        summary = [ordered]@{
            total = $checkResults.Count
            passed = @($checkResults | Where-Object { $_.status -eq "pass" }).Count
            failed = @($checkResults | Where-Object { $_.status -eq "fail" }).Count
            not_run = @($checkResults | Where-Object { $_.status -eq "not_run" }).Count
            partial = @($checkResults | Where-Object { $_.status -eq "partial" }).Count
            unverified = @($checkResults | Where-Object { $_.status -eq "unverified" }).Count
            flaky = @($checkResults | Where-Object { $_.status -eq "flaky" }).Count
            failure_summary = $failureSummary
        }
        checks = @($checkResults)
        artifact_refs = @("$relativeArtifact/report.json", "$relativeArtifact/paths.json") + @($checkResults | ForEach-Object { $_.log_ref })
    }
    $json = $report | ConvertTo-Json -Depth 12
    [IO.File]::WriteAllText((Join-Path $artifactDirectory "report.json"), $json + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    if ($OutputFormat -eq "json") {
        [Console]::Out.WriteLine($json)
    } else {
        $summaryLine = "validation {0}: requested={1} required={2} effective={3}; checks={4}; duration_ms={5}" -f $report.status, $Profile, $requiredProfile, $effectiveProfile, $checkResults.Count, $report.duration_ms
        [Console]::Out.WriteLine($summaryLine)
        [Console]::Out.WriteLine("evidence: $relativeArtifact/report.json")
        foreach ($failure in $failureSummary) {
            [Console]::Out.WriteLine("failure: $failure")
        }
    }
    switch ($report.status) {
        "pass" { return 0 }
        "fail" { return 1 }
        default { return 3 }
    }
}
