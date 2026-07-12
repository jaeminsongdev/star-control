param(
    [Parameter(Mandatory = $true)]
    [string]$RunRoot
)

$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$run = [IO.Path]::GetFullPath($RunRoot)
$install = Join-Path $run 'install'
$workspace = Join-Path $run 'workspace'
$codexFixture = Join-Path $PSScriptRoot 'codex-e2e-fixture.ps1'

if (Test-Path -LiteralPath $run) {
    throw "management CLI RunRoot must be new: $run"
}

# Fail before creating the isolated fixture tree when the current-user pipe is
# already owned. A newly requested RunRoot cannot legitimately own a running
# Controller yet, so any existing instance is foreign to this fixture.
$foreignControllers = @(Get-Process -Name 'star-controller' -ErrorAction SilentlyContinue |
    Where-Object { $_.Path })
if ($foreignControllers.Count -ne 0) {
    throw 'another Controller owns the current-user pipe; stop it before this isolated fixture'
}

function Stop-IsolatedController {
    $expected = [IO.Path]::GetFullPath((Join-Path $install 'star-controller.exe'))
    foreach ($process in @(Get-Process -Name 'star-controller' -ErrorAction SilentlyContinue)) {
        if (-not $process.Path) {
            continue
        }
        $actual = [IO.Path]::GetFullPath($process.Path)
        if ($actual.Equals($expected, [StringComparison]::OrdinalIgnoreCase)) {
            Stop-Process -Id $process.Id -Force
            $null = $process.WaitForExit(5000)
        }
    }
}

function Invoke-StarJson([string[]]$Arguments) {
    $raw = & $script:star @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "star.exe failed with exit code $LASTEXITCODE for: $($Arguments -join ' ')"
    }
    return ($raw -join "`n") | ConvertFrom-Json -Depth 100
}

function Get-AutostartValue {
    $subkey = 'Software\Microsoft\Windows\CurrentVersion\Run'
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($subkey, $false)
    if ($null -eq $key) {
        throw "current-user Run key does not exist: $subkey"
    }
    try {
        $exists = @($key.GetValueNames()) -contains 'Star-Control'
        return [pscustomobject]@{
            Exists = $exists
            Kind = if ($exists) { $key.GetValueKind('Star-Control').ToString() } else { $null }
            Value = if ($exists) {
                [string]$key.GetValue(
                    'Star-Control',
                    $null,
                    [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
                )
            } else {
                $null
            }
        }
    } finally {
        $key.Dispose()
    }
}

$initialAutostart = Get-AutostartValue
if ($initialAutostart.Exists) {
    throw 'Star-Control autostart value already exists; the isolated fixture will not replace it'
}

New-Item -ItemType Directory -Path (Join-Path $install 'catalog\tool-packages') -Force | Out-Null
foreach ($name in @('star.exe', 'star-mcp.exe', 'star-controller.exe', 'star-fake-exe.exe')) {
    $source = Join-Path $repo "target\release\$name"
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "missing release binary: $source"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $install $name)
}
Copy-Item -LiteralPath (Join-Path $repo 'catalog\tool-packages\star-control-core.toml') `
    -Destination (Join-Path $install 'catalog\tool-packages\star-control-core.toml')

$null = & $codexFixture -Phase prepare -RunRoot $run
$null = & $codexFixture -Phase add -RunRoot $run
$manifestPath = Join-Path $workspace 'appdata\Star-Control\tools.d\codex-same-session.toml'
$manifest = [IO.File]::ReadAllText($manifestPath)
$probe = @"
authenticode_policy = "record"

[executables.probe]
kind = "argv"
args = ["probe-version"]
output_format = "semver_line"
version_pattern = "^(?<product>[0-9]+\\.[0-9]+\\.[0-9]+) interface=(?<interface>[0-9]+\\.[0-9]+\\.[0-9]+)$"
"@
$updated = $manifest.Replace('authenticode_policy = "record"', $probe.TrimEnd())
if ($updated -eq $manifest) {
    throw 'fixture manifest probe insertion point was not found'
}
[IO.File]::WriteAllText($manifestPath, $updated, [Text.UTF8Encoding]::new($false))

$env:APPDATA = Join-Path $workspace 'appdata'
$env:LOCALAPPDATA = Join-Path $workspace 'localappdata'
$env:USERPROFILE = Join-Path $workspace 'userprofile'
$env:STAR_CONTROL_RELEASE_TOOLS_DIR = Join-Path $install 'catalog\tool-packages'
$script:star = Join-Path $install 'star.exe'
$controllerFullPath = [IO.Path]::GetFullPath((Join-Path $install 'star-controller.exe'))
$controllerCanonicalPath = if ($controllerFullPath.StartsWith('\\?\')) {
    $controllerFullPath
} else {
    '\\?\' + $controllerFullPath
}
$expectedAutostartCommand = '"' + $controllerCanonicalPath + '" --background'

try {
    $validate = Invoke-StarJson @('tools', 'validate', $manifestPath, '--source', 'user', '--json')
    if ($validate.status -ne 'ok' -or $validate.data.valid -ne $true) {
        throw 'tools validate did not accept the valid fixture'
    }

    $initialStatus = Invoke-StarJson @('tools', 'status', 'user.codex-same-session', '--json')
    $candidate = @($initialStatus.data.items)[0]
    if ($candidate.trust_state -ne 'untrusted') {
        throw 'safe_default candidate unexpectedly started trusted'
    }

    $trust = Invoke-StarJson @(
        'tools', 'trust', 'user.codex-same-session',
        '--manifest-hash', [string]$candidate.candidate_manifest_hash,
        '--json'
    )
    if ($trust.status -ne 'ok') {
        throw 'tools trust failed'
    }

    $list = Invoke-StarJson @('tools', 'list', '--source', 'user', '--readiness', 'ready', '--json')
    $toolIds = @($list.data.items.tool_id)
    if ($toolIds.Count -ne 3 -or $toolIds -notcontains 'user.codex-same-session.echo') {
        throw 'trusted user package was not listed completely'
    }

    $describe = Invoke-StarJson @('tools', 'describe', 'user.codex-same-session.echo', '--json')
    if ($describe.status -ne 'ok' -or $describe.data.trust_basis -ne 'explicit_trust_store') {
        throw 'tools describe did not expose trusted provenance'
    }

    $probeResult = Invoke-StarJson @(
        'tools', 'probe', 'user.codex-same-session', '--executable', 'fixture', '--json'
    )
    if ($probeResult.status -ne 'ok' -or
        $probeResult.data.product_version -ne '1.2.3' -or
        $probeResult.data.interface_version -ne '1.0.0' -or
        $probeResult.data.authenticode.network_access -ne $false) {
        throw 'tools probe did not return the frozen offline identity result'
    }
    $afterProbeStatus = Invoke-StarJson @(
        'tools', 'status', 'user.codex-same-session', '--json'
    )
    $probeStatusItem = @($afterProbeStatus.data.items)[0]
    if ([string]::IsNullOrWhiteSpace([string]$probeStatusItem.last_probe_at)) {
        throw 'successful explicit probe did not update status last_probe_at'
    }

    $revoke = Invoke-StarJson @(
        'tools', 'revoke', 'user.codex-same-session', '--cancel-running',
        '--reason', 'isolated management CLI evidence', '--json'
    )
    if ($revoke.status -ne 'ok' -or $revoke.data.revoked -ne $true) {
        throw 'tools revoke failed'
    }
    $afterRevokeList = Invoke-StarJson @('tools', 'list', '--source', 'user', '--json')
    $afterRevokeStatus = Invoke-StarJson @(
        'tools', 'status', 'user.codex-same-session', '--json'
    )
    $revokedItem = @($afterRevokeStatus.data.items)[0]
    if (@($afterRevokeList.data.items).Count -ne 0 -or
        $revokedItem.active_state -ne 'last_known_good' -or
        $revokedItem.candidate_state -ne 'revoked' -or
        $revokedItem.trust_state -ne 'untrusted') {
        throw 'revoked package was not status-only with the frozen state labels'
    }

    $fake = Join-Path $workspace 'fixture-tools\star-fake-a.exe'
    $scaffoldPath = Join-Path $workspace 'scaffold.toml'
    $scaffold = Invoke-StarJson @('tools', 'scaffold', $fake, '--output', $scaffoldPath)
    $scaffoldText = [IO.File]::ReadAllText($scaffoldPath)
    $scaffoldHash = [regex]::Match(
        $scaffoldText,
        'sha256 = "(?<hash>sha256:[0-9a-f]{64})"'
    ).Groups['hash'].Value
    $actualHash = 'sha256:' + (Get-FileHash -LiteralPath $fake -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($scaffold.sha256 -ne $actualHash -or
        $scaffoldHash -ne $actualHash -or
        -not $scaffoldText.Contains('enabled = false') -or
        -not $scaffoldText.Contains('update_policy = "pinned_hash"') -or
        [regex]::Matches($scaffoldText, '\[\[actions\]\]').Count -ne 0) {
        throw 'tools scaffold did not produce an exact disabled pinned-hash draft'
    }
    $scaffoldValidate = Invoke-StarJson @(
        'tools', 'validate', $scaffoldPath, '--source', 'user', '--json'
    )
    if ($scaffoldValidate.status -ne 'ok' -or $scaffoldValidate.data.valid -ne $true) {
        throw 'scaffold output did not validate'
    }

    Stop-IsolatedController
    $start = Invoke-StarJson @('controller', 'start', '--background')
    $runningStatus = Invoke-StarJson @('tools', 'status', '--json')
    if ($start.running -ne $true -or
        $start.instance_id -ne $runningStatus.data.controller.instance_id) {
        throw 'controller start --background did not produce the status instance'
    }
    $autostartBefore = Invoke-StarJson @('controller', 'autostart', 'status')
    if ($autostartBefore.state -ne 'disabled') {
        throw 'controller autostart did not begin disabled'
    }
    $autostartEnable = Invoke-StarJson @('controller', 'autostart', 'enable')
    $autostartEnabled = Invoke-StarJson @('controller', 'autostart', 'status')
    $enabledValue = Get-AutostartValue
    if ($autostartEnable.state -ne 'enabled' -or
        $autostartEnabled.state -ne 'enabled' -or
        -not $enabledValue.Exists -or
        $enabledValue.Kind -ne 'String' -or
        $enabledValue.Value -ne $expectedAutostartCommand) {
        throw 'controller autostart enable did not create the exact owned REG_SZ command'
    }
    $autostartEnableAgain = Invoke-StarJson @('controller', 'autostart', 'enable')
    $enabledValueAgain = Get-AutostartValue
    if ($autostartEnableAgain.state -ne 'enabled' -or
        $enabledValueAgain.Value -ne $enabledValue.Value) {
        throw 'controller autostart enable was not idempotent'
    }
    $autostartDisable = Invoke-StarJson @('controller', 'autostart', 'disable')
    $autostartDisabled = Invoke-StarJson @('controller', 'autostart', 'status')
    $disabledValue = Get-AutostartValue
    if ($autostartDisable.state -ne 'disabled' -or
        $autostartDisabled.state -ne 'disabled' -or
        $disabledValue.Exists) {
        throw 'controller autostart disable did not remove the exact owned command'
    }
    $autostartDisableAgain = Invoke-StarJson @('controller', 'autostart', 'disable')
    if ($autostartDisableAgain.state -ne 'disabled' -or (Get-AutostartValue).Exists) {
        throw 'controller autostart disable was not idempotent'
    }

    $architecture = switch ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()) {
        'X64' { 'x86_64' }
        'Arm64' { 'aarch64' }
        default { [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant() }
    }
    [ordered]@{
        schema_id = 'star.management-cli-smoke-evidence'
        schema_version = 1
        observed_at = [DateTimeOffset]::UtcNow.ToString('O')
        host = [ordered]@{
            os = [Environment]::OSVersion.VersionString
            os_build = [Environment]::OSVersion.Version.Build
            architecture = $architecture
        }
        binaries = [ordered]@{
            cli_sha256 = 'sha256:' + (Get-FileHash -LiteralPath $script:star -Algorithm SHA256).Hash.ToLowerInvariant()
            controller_sha256 = 'sha256:' + (Get-FileHash -LiteralPath (Join-Path $install 'star-controller.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
            fake_sha256 = $actualHash
        }
        results = [ordered]@{
            validate = $true
            trust = $true
            list_count = $toolIds.Count
            describe_trust_basis = $describe.data.trust_basis
            descriptor_hash = $describe.data.descriptor_hash
            probe_product_version = $probeResult.data.product_version
            probe_interface_version = $probeResult.data.interface_version
            probe_network_access = $probeResult.data.authenticode.network_access
            probe_recorded = $true
            last_probe_at = $probeStatusItem.last_probe_at
            revoke = $true
            revoked_list_count = @($afterRevokeList.data.items).Count
            revoked_active_state = $revokedItem.active_state
            revoked_candidate_state = $revokedItem.candidate_state
            scaffold = $true
            scaffold_sha256 = $actualHash
            scaffold_validate = $true
            controller_start = $true
            controller_instance_id = $start.instance_id
            autostart_initial_state = $autostartBefore.state
            autostart_enabled_state = $autostartEnabled.state
            autostart_disabled_state = $autostartDisabled.state
            autostart_value_kind = $enabledValue.Kind
            autostart_exact_command = $true
            autostart_enable_idempotent = $true
            autostart_disable_idempotent = $true
            autostart_mutation_executed = $true
            autostart_original_state_restored = $true
        }
        method = [ordered]@{
            fixture = 'tools/mcp-management-cli-fixture.ps1'
            management_flow = 'validate -> trust -> list/describe -> probe -> revoke -> scaffold/validate -> controller start/status -> autostart enable/status/disable/status'
            autostart = 'The initially absent exact HKCU Run value was enabled twice, checked as owned REG_SZ, disabled twice, and verified absent again.'
        }
    } | ConvertTo-Json -Depth 100
} finally {
    try {
        $currentAutostart = Get-AutostartValue
        if (-not $initialAutostart.Exists -and $currentAutostart.Exists) {
            if ($currentAutostart.Value -ne $expectedAutostartCommand) {
                throw 'autostart changed to a foreign value during the fixture; it was preserved'
            }
            $null = Invoke-StarJson @('controller', 'autostart', 'disable')
            if ((Get-AutostartValue).Exists) {
                throw 'fixture could not restore the initially absent autostart value'
            }
        }
    } finally {
        Stop-IsolatedController
    }
}
