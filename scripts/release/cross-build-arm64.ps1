[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Get-RustIdentity {
    param([Parameter(Mandatory)][string]$RustcPath)

    $lines = @(& $RustcPath -Vv)
    if ($LASTEXITCODE -ne 0) {
        throw "rustc identity probe failed: $RustcPath"
    }
    $release = ($lines | Where-Object { $_ -like 'release:*' } | Select-Object -First 1) -replace '^release:\s*', ''
    $commit = ($lines | Where-Object { $_ -like 'commit-hash:*' } | Select-Object -First 1) -replace '^commit-hash:\s*', ''
    if ([string]::IsNullOrWhiteSpace($release) -or $commit -notmatch '^[0-9a-f]{40}$') {
        throw "rustc identity is incomplete: $RustcPath"
    }
    return [pscustomobject]@{ Release = $release; Commit = $commit }
}

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$activeSysroot = (& rustc --print sysroot).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($activeSysroot)) {
    throw 'active Rust sysroot is unavailable'
}
$activeRustc = Join-Path $activeSysroot 'bin\rustc.exe'
$activeIdentity = Get-RustIdentity -RustcPath $activeRustc
$toolchainsRoot = Split-Path $activeSysroot -Parent
$candidateRoots = @($activeSysroot) + @(
    Get-ChildItem -LiteralPath $toolchainsRoot -Directory -ErrorAction Stop |
        Sort-Object FullName |
        Select-Object -ExpandProperty FullName
)
$selected = $null
foreach ($candidate in $candidateRoots | Select-Object -Unique) {
    $rustc = Join-Path $candidate 'bin\rustc.exe'
    $cargo = Join-Path $candidate 'bin\cargo.exe'
    $cargoClippy = Join-Path $candidate 'bin\cargo-clippy.exe'
    $clippyDriver = Join-Path $candidate 'bin\clippy-driver.exe'
    $targetLib = Join-Path $candidate 'lib\rustlib\aarch64-pc-windows-msvc\lib'
    if (-not (Test-Path -LiteralPath $rustc -PathType Leaf) -or
        -not (Test-Path -LiteralPath $cargo -PathType Leaf) -or
        -not (Test-Path -LiteralPath $cargoClippy -PathType Leaf) -or
        -not (Test-Path -LiteralPath $clippyDriver -PathType Leaf) -or
        -not (Test-Path -LiteralPath $targetLib -PathType Container)) {
        continue
    }
    $identity = Get-RustIdentity -RustcPath $rustc
    if ($identity.Release -eq $activeIdentity.Release -and $identity.Commit -eq $activeIdentity.Commit) {
        $selected = [pscustomobject]@{
            Root = $candidate
            Cargo = $cargo
            CargoClippy = $cargoClippy
            Rustc = $rustc
            Rustdoc = Join-Path $candidate 'bin\rustdoc.exe'
            ClippyDriver = $clippyDriver
            Identity = $identity
        }
        break
    }
}

if ($null -eq $selected) {
    [Console]::Error.WriteLine(
        "exact Rust $($activeIdentity.Release) ($($activeIdentity.Commit)) ARM64 standard library is unavailable"
    )
    exit 3
}

$env:RUSTC = $selected.Rustc
$env:RUSTDOC = $selected.Rustdoc
$env:PATH = (Join-Path $selected.Root 'bin') + [IO.Path]::PathSeparator + $env:PATH
$rustStyleCorpusManifest = Join-Path $repoRoot 'specs\corpus\rust-style\multicrate\Cargo.toml'
$rustStyleCorpusTarget = Join-Path $repoRoot 'target\rust-style-arm64-corpus'
Push-Location $repoRoot
try {
    & $selected.Cargo build --workspace --release --target aarch64-pc-windows-msvc --locked
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $selected.Cargo check --manifest-path $rustStyleCorpusManifest --workspace --all-targets `
        --features rust-style-app/cli `
        --target aarch64-pc-windows-msvc --locked --offline --target-dir $rustStyleCorpusTarget
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $selected.Cargo clippy --manifest-path $rustStyleCorpusManifest --workspace --all-targets `
        --features rust-style-app/cli `
        --target aarch64-pc-windows-msvc --locked --offline --target-dir $rustStyleCorpusTarget `
        --no-deps -- -D warnings
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}

[pscustomobject]@{
    target = 'aarch64-pc-windows-msvc'
    rust_release = $selected.Identity.Release
    rust_commit = $selected.Identity.Commit
    toolchain_root = $selected.Root
    cargo = $selected.Cargo
    cargo_clippy = $selected.CargoClippy
    clippy_driver = $selected.ClippyDriver
    status = 'cross_build_complete'
    runtime_verification = 'native_unverified'
    rust_style_corpus = 'arm64_check_and_clippy_complete'
} | ConvertTo-Json -Compress
