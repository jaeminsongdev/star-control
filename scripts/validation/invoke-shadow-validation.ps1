[CmdletBinding()]
param(
    [ValidateSet("quick", "target", "full", "release")]
    [string]$Profile = "target",
    [string]$Unit,
    [string]$BaseRef,
    [ValidateSet("pass", "fail", "cancelled", "skipped")]
    [string]$LegacyResult = "pass",
    [string]$ContractPath,
    [switch]$FailOnMismatch
)

$ErrorActionPreference = "Stop"
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
if ([string]::IsNullOrWhiteSpace($ContractPath)) {
    $ContractPath = Join-Path $PSScriptRoot "shadow-contract.json"
} elseif (-not [IO.Path]::IsPathRooted($ContractPath)) {
    $ContractPath = Join-Path $repositoryRoot $ContractPath
}

$runId = "{0}-{1}" -f [DateTime]::UtcNow.ToString("yyyyMMddTHHmmssfffZ"), $PID
$artifactDirectory = Join-Path $repositoryRoot "target/validation-shadow/$runId"
$candidatePath = Join-Path $artifactDirectory "candidate-report.json"
$candidateErrorPath = Join-Path $artifactDirectory "candidate-stderr.log"
$comparisonPath = Join-Path $artifactDirectory "comparison.json"
$utf8 = [Text.UTF8Encoding]::new($false)
[void](New-Item -ItemType Directory -Path $artifactDirectory -Force)

$validationArguments = @(
    "-NoProfile",
    "-File", (Join-Path $repositoryRoot "scripts/validate.ps1"),
    "-Profile", $Profile,
    "-OutputFormat", "json"
)
if (-not [string]::IsNullOrWhiteSpace($Unit)) {
    $validationArguments += @("-Unit", $Unit)
}
if (-not [string]::IsNullOrWhiteSpace($BaseRef)) {
    $validationArguments += @("-BaseRef", $BaseRef)
}

$candidateOutput = @(& pwsh @validationArguments 2> $candidateErrorPath)
$candidateExitCode = $LASTEXITCODE
[IO.File]::WriteAllLines($candidatePath, [string[]]$candidateOutput, $utf8)

$comparisonArguments = @(
    "-X", "utf8", (Join-Path $PSScriptRoot "shadow_compare.py"),
    "--contract", $ContractPath,
    "--candidate", $candidatePath,
    "--legacy-result", $LegacyResult,
    "--output", $comparisonPath
)
$comparisonOutput = @(& python @comparisonArguments 2>&1)
$comparisonExitCode = $LASTEXITCODE
foreach ($line in $comparisonOutput) {
    Write-Output ([string]$line)
}

$comparisonStatus = "unverified"
if (Test-Path -LiteralPath $comparisonPath) {
    $comparison = Get-Content -LiteralPath $comparisonPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $comparisonStatus = [string]$comparison.comparison_status
    foreach ($observation in @($comparison.observations)) {
        $command = if ($null -eq $observation.candidate_command) {
            "not_run"
        } else {
            $executable = [string]$observation.candidate_command.executable
            $arguments = @($observation.candidate_command.arguments) -join " "
            "$executable $arguments".Trim()
        }
        Write-Output (
            "shadow check: legacy={0} candidate={1} unit={2} selection={3} command_relation={4} result_relation={5} exit={6} duration_ms={7} log={8} command={9}" -f
            $observation.legacy_id,
            $observation.candidate_id,
            $observation.candidate_unit,
            $observation.selection_relation,
            $observation.command_relation,
            $observation.result_relation,
            $observation.candidate_exit_code,
            $observation.candidate_duration_ms,
            $observation.candidate_log_ref,
            $command
        )
    }
}
$summary = "shadow observation: authority=$LegacyResult candidate_exit=$candidateExitCode comparison=$comparisonStatus artifact=$artifactDirectory"
Write-Output $summary
if (-not [string]::IsNullOrWhiteSpace($env:GITHUB_STEP_SUMMARY)) {
    Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value "- $summary" -Encoding UTF8
}

$shadowFailed = $candidateExitCode -ne 0 -or $comparisonExitCode -ne 0 -or $comparisonStatus -ne "pass"
if ($shadowFailed) {
    Write-Warning "shadow validation differs from the authority path; the authority result is unchanged"
}
if ($FailOnMismatch -and $shadowFailed) {
    exit 1
}
exit 0
