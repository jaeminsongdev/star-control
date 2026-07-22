[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('x64', 'arm64')]
    [string]$Architecture,

    [string]$SourceRevision,
    [string]$IsccPath,
    [switch]$PortableZip,
    [switch]$ReplaceStage,
    [switch]$UseExistingSignedStage
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-PackagingProcessCapture {
    param(
        [Parameter(Mandatory)][string]$Executable,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory)][string]$WorkingDirectory,
        [switch]$EchoOutput
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
        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        if ($EchoOutput) {
            if (-not [string]::IsNullOrEmpty($stdout)) {
                [Console]::Out.Write($stdout)
            }
            if (-not [string]::IsNullOrEmpty($stderr)) {
                [Console]::Error.Write($stderr)
            }
        }
        return [pscustomobject]@{
            ExitCode = $process.ExitCode
            Stdout = $stdout
            Stderr = $stderr
        }
    }
    finally {
        $process.Dispose()
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$cargoToml = Get-Content -Raw -LiteralPath (Join-Path $repoRoot 'Cargo.toml')
$versionMatch = [regex]::Match($cargoToml, '(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success) {
    throw 'Cargo.toml workspace package version을 찾을 수 없습니다.'
}
$version = $versionMatch.Groups[1].Value

if ([string]::IsNullOrWhiteSpace($SourceRevision)) {
    $revisionResult = Invoke-PackagingProcessCapture -Executable 'git' -Arguments @('-C', $repoRoot, 'rev-parse', 'HEAD') -WorkingDirectory $repoRoot
    $commit = $revisionResult.Stdout.Trim()
    if ($revisionResult.ExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($commit)) {
        throw 'SourceRevision을 지정하거나 Git commit을 확인할 수 있어야 합니다.'
    }
    $statusResult = Invoke-PackagingProcessCapture -Executable 'git' -Arguments @('-C', $repoRoot, 'status', '--porcelain', '--untracked-files=no') -WorkingDirectory $repoRoot
    if ($statusResult.ExitCode -ne 0) {
        throw 'Git worktree 상태를 확인할 수 없습니다.'
    }
    $dirty = $statusResult.Stdout.Trim()
    $SourceRevision = if ($dirty) { "dirty:$commit" } else { $commit }
}

$target = if ($Architecture -eq 'x64') {
    'x86_64-pc-windows-msvc'
} else {
    'aarch64-pc-windows-msvc'
}
$binaryDir = Join-Path $repoRoot "target\$target\release"
$stageDir = Join-Path $repoRoot "dist\stage\$version\$Architecture"
$outputDir = Join-Path $repoRoot 'dist'

if ($UseExistingSignedStage -and ($ReplaceStage -or $PortableZip)) {
    throw '-UseExistingSignedStage cannot be combined with -ReplaceStage or -PortableZip.'
}

if (Test-Path -LiteralPath $stageDir) {
    $existing = Get-ChildItem -LiteralPath $stageDir -Force -ErrorAction Stop | Select-Object -First 1
    if ($existing) {
        if ($UseExistingSignedStage) {
            # Public flow consumes this exact signed stage without rebuilding it.
        } elseif (-not $ReplaceStage) {
            throw "기존 stage를 덮어쓰지 않습니다. 다시 만들려면 -ReplaceStage를 명시하세요: $stageDir"
        }
        if (-not $UseExistingSignedStage) {
            $resolvedStage = [System.IO.Path]::GetFullPath($stageDir)
            $allowedRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot 'dist\stage')) + [System.IO.Path]::DirectorySeparatorChar
            if (-not $resolvedStage.StartsWith($allowedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
                throw "stage 삭제 경계 밖입니다: $resolvedStage"
            }
            Remove-Item -LiteralPath $resolvedStage -Recurse -Force
        }
    }
} elseif ($UseExistingSignedStage) {
    throw "signed stage가 없습니다: $stageDir"
}

Push-Location $repoRoot
try {
    $stageSigning = 'unsigned_local'
    if ($UseExistingSignedStage) {
        $verifyArgs = @(
            'run', '--locked', '-p', 'star-package-release', '--', 'verify',
            '--architecture', $Architecture,
            '--stage', $stageDir
        )
        $result = Invoke-PackagingProcessCapture -Executable 'cargo' -Arguments $verifyArgs -WorkingDirectory $repoRoot -EchoOutput
        if ($result.ExitCode -ne 0) { throw 'signed release stage verification failed' }
        $stageManifest = Get-Content -Raw -LiteralPath (Join-Path $stageDir 'release-manifest.json') | ConvertFrom-Json
        if ($stageManifest.signing -ne 'signed' -or $stageManifest.source_revision -ne $SourceRevision) {
            throw 'signed stage signing state or source revision does not match the exact build request'
        }
        $stageSigning = 'signed'
    } else {
        $result = Invoke-PackagingProcessCapture -Executable 'cargo' -Arguments @('build', '--locked', '--release', '--target', $target, '-p', 'star-cli', '--bin', 'star') -WorkingDirectory $repoRoot -EchoOutput
        if ($result.ExitCode -ne 0) { throw 'star.exe build failed' }
        $result = Invoke-PackagingProcessCapture -Executable 'cargo' -Arguments @('build', '--locked', '--release', '--target', $target, '-p', 'star-controller', '--bin', 'star-controller') -WorkingDirectory $repoRoot -EchoOutput
        if ($result.ExitCode -ne 0) { throw 'star-controller.exe build failed' }
        $result = Invoke-PackagingProcessCapture -Executable 'cargo' -Arguments @('build', '--locked', '--release', '--target', $target, '-p', 'star-mcp', '--bin', 'star-mcp') -WorkingDirectory $repoRoot -EchoOutput
        if ($result.ExitCode -ne 0) { throw 'star-mcp.exe build failed' }
        $result = Invoke-PackagingProcessCapture -Executable 'cargo' -Arguments @('build', '--locked', '--release', '--target', $target, '-p', 'star-updater', '--bin', 'star-updater') -WorkingDirectory $repoRoot -EchoOutput
        if ($result.ExitCode -ne 0) { throw 'star-updater.exe build failed' }

        $stageArgs = @(
            'run', '--locked', '-p', 'star-package-release', '--', 'stage',
            '--architecture', $Architecture,
            '--binary-dir', $binaryDir,
            '--output', $stageDir,
            '--source-revision', $SourceRevision
        )
        $result = Invoke-PackagingProcessCapture -Executable 'cargo' -Arguments $stageArgs -WorkingDirectory $repoRoot -EchoOutput
        if ($result.ExitCode -ne 0) { throw 'release stage generation failed' }
    }

    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
    if ($PortableZip) {
        $zip = Join-Path $outputDir "star-control-windows-$Architecture-$version-portable.zip"
        if (Test-Path -LiteralPath $zip) {
            throw "기존 portable ZIP을 덮어쓰지 않습니다: $zip"
        }
        Compress-Archive -Path (Join-Path $stageDir '*') -DestinationPath $zip -CompressionLevel Optimal
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash.ToLowerInvariant()
        [pscustomobject]@{
            portable_zip = $zip
            sha256 = "sha256:$hash"
            architecture = $Architecture
            version = $version
            signing_state = $stageSigning
        } | ConvertTo-Json
        return
    }

    if ([string]::IsNullOrWhiteSpace($IsccPath)) {
        $candidates = @(
            (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
            (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe'),
            (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
        )
        $IsccPath = $candidates | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -First 1
    }
    if ([string]::IsNullOrWhiteSpace($IsccPath) -or -not (Test-Path -LiteralPath $IsccPath)) {
        throw 'ISCC.exe를 찾지 못했습니다. Inno Setup 6 설치 후 -IsccPath를 지정하세요.'
    }

    $installer = Join-Path $outputDir "star-control-windows-$Architecture-$version-setup.exe"
    if (Test-Path -LiteralPath $installer) {
        throw "기존 installer를 덮어쓰지 않습니다: $installer"
    }
    $iss = Join-Path $PSScriptRoot 'star-control.iss'
    $isccArgs = @(
        "/DAppVersion=$version",
        "/DTargetArch=$Architecture",
        "/DStageDir=$stageDir",
        "/DOutputDir=$outputDir",
        $iss
    )
    $result = Invoke-PackagingProcessCapture -Executable $IsccPath -Arguments $isccArgs -WorkingDirectory $repoRoot -EchoOutput
    if ($result.ExitCode -ne 0) { throw 'Inno Setup compilation failed' }

    if (-not (Test-Path -LiteralPath $installer)) {
        throw "예상 installer가 없습니다: $installer"
    }
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installer).Hash.ToLowerInvariant()
    [pscustomobject]@{
        installer = $installer
        sha256 = "sha256:$hash"
        architecture = $Architecture
        version = $version
        runtime_stage_signing = $stageSigning
        installer_signing = 'unsigned_local'
    } | ConvertTo-Json
}
finally {
    Pop-Location
}
