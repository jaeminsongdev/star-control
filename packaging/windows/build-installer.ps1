[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('x64', 'arm64')]
    [string]$Architecture,

    [string]$SourceRevision,
    [string]$IsccPath,
    [switch]$PortableZip,
    [switch]$ReplaceStage
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$cargoToml = Get-Content -Raw -LiteralPath (Join-Path $repoRoot 'Cargo.toml')
$versionMatch = [regex]::Match($cargoToml, '(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success) {
    throw 'Cargo.toml workspace package version을 찾을 수 없습니다.'
}
$version = $versionMatch.Groups[1].Value

if ([string]::IsNullOrWhiteSpace($SourceRevision)) {
    $commit = (& git -C $repoRoot rev-parse HEAD 2>$null)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($commit)) {
        throw 'SourceRevision을 지정하거나 Git commit을 확인할 수 있어야 합니다.'
    }
    $dirty = & git -C $repoRoot status --porcelain --untracked-files=no
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

if (Test-Path -LiteralPath $stageDir) {
    $existing = Get-ChildItem -LiteralPath $stageDir -Force -ErrorAction Stop | Select-Object -First 1
    if ($existing) {
        if (-not $ReplaceStage) {
            throw "기존 stage를 덮어쓰지 않습니다. 다시 만들려면 -ReplaceStage를 명시하세요: $stageDir"
        }
        $resolvedStage = [System.IO.Path]::GetFullPath($stageDir)
        $allowedRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot 'dist\stage')) + [System.IO.Path]::DirectorySeparatorChar
        if (-not $resolvedStage.StartsWith($allowedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "stage 삭제 경계 밖입니다: $resolvedStage"
        }
        Remove-Item -LiteralPath $resolvedStage -Recurse -Force
    }
}

Push-Location $repoRoot
try {
    & cargo build --locked --release --target $target -p star-cli --bin star
    if ($LASTEXITCODE -ne 0) { throw 'star.exe build failed' }
    & cargo build --locked --release --target $target -p star-controller --bin star-controller
    if ($LASTEXITCODE -ne 0) { throw 'star-controller.exe build failed' }
    & cargo build --locked --release --target $target -p star-mcp --bin star-mcp
    if ($LASTEXITCODE -ne 0) { throw 'star-mcp.exe build failed' }

    $stageArgs = @(
        'run', '--locked', '-p', 'star-package-release', '--', 'stage',
        '--architecture', $Architecture,
        '--binary-dir', $binaryDir,
        '--output', $stageDir,
        '--source-revision', $SourceRevision
    )
    & cargo @stageArgs
    if ($LASTEXITCODE -ne 0) { throw 'release stage generation failed' }

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

    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
    $iss = Join-Path $PSScriptRoot 'star-control.iss'
    $isccArgs = @(
        "/DAppVersion=$version",
        "/DTargetArch=$Architecture",
        "/DStageDir=$stageDir",
        "/DOutputDir=$outputDir",
        $iss
    )
    & $IsccPath @isccArgs
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed' }

    $installer = Join-Path $outputDir "star-control-windows-$Architecture-$version-setup.exe"
    if (-not (Test-Path -LiteralPath $installer)) {
        throw "예상 installer가 없습니다: $installer"
    }
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installer).Hash.ToLowerInvariant()
    [pscustomobject]@{
        installer = $installer
        sha256 = "sha256:$hash"
        architecture = $Architecture
        version = $version
        signing_state = 'unsigned_local'
    } | ConvertTo-Json

    if ($PortableZip) {
        $zip = Join-Path $outputDir "star-control-windows-$Architecture-$version-portable.zip"
        if (Test-Path -LiteralPath $zip) {
            throw "기존 portable ZIP을 덮어쓰지 않습니다: $zip"
        }
        Compress-Archive -Path (Join-Path $stageDir '*') -DestinationPath $zip -CompressionLevel Optimal
    }
}
finally {
    Pop-Location
}
