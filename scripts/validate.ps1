[CmdletBinding()]
param(
    [ValidateSet("quick", "target", "full", "release")]
    [string]$Profile = "target",

    [string]$Unit,

    [string]$BaseRef,

    [ValidateSet("text", "json")]
    [string]$OutputFormat = "text"
)

$ErrorActionPreference = "Stop"
$utf8NoBom = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8NoBom
$OutputEncoding = $utf8NoBom
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
. (Join-Path $PSScriptRoot "validation/common.ps1")
. (Join-Path $PSScriptRoot "validation/project.ps1")

try {
    $config = New-ProjectValidationConfig -Root $repositoryRoot
    $exitCode = Invoke-ProjectValidation -Config $config -Profile $Profile -Unit $Unit -BaseRef $BaseRef -OutputFormat $OutputFormat
    exit $exitCode
} catch [ArgumentException] {
    $errorRecord = New-ValidationEntryError -Kind "invocation" -Status "fail" -Message $_.Exception.Message -ExitCode 2
    if ($OutputFormat -eq "json") {
        [Console]::Out.WriteLine(($errorRecord | ConvertTo-Json -Compress))
    } else {
        [Console]::Error.WriteLine("validation invocation error: $($_.Exception.Message)")
    }
    exit 2
} catch {
    $errorRecord = New-ValidationEntryError -Kind "runner" -Status "unverified" -Message $_.Exception.Message -ExitCode 4
    if ($OutputFormat -eq "json") {
        [Console]::Out.WriteLine(($errorRecord | ConvertTo-Json -Compress))
    } else {
        [Console]::Error.WriteLine("validation runner error: $($_.Exception.Message)")
    }
    exit 4
}
