param(
    [Parameter(Mandatory = $true)]
    [string]$RunRoot,

    [Parameter(Mandatory = $true)]
    [string]$EvidencePath,

    [switch]$PrestartController
)

$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$run = [IO.Path]::GetFullPath($RunRoot)
$evidencePath = [IO.Path]::GetFullPath($EvidencePath)
$install = Join-Path $run 'install'
$workspace = Join-Path $run 'workspace'
$appData = Join-Path $workspace 'appdata'
$localAppData = Join-Path $workspace 'localappdata'
$userProfile = Join-Path $workspace 'userprofile'
$catalog = Join-Path $install 'catalog\tool-packages'
$manifestRoot = Join-Path $appData 'Star-Control\tools.d'
$manifestPath = Join-Path $manifestRoot 'arm64-smoke.toml'
$fixture = Join-Path $PSScriptRoot 'codex-e2e-fixture.ps1'

if ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne
    [Runtime.InteropServices.Architecture]::Arm64) {
    throw 'native ARM64 Windows is required; cross-build or x64 emulation is not accepted'
}
if (Test-Path -LiteralPath $run) {
    throw "ARM64 smoke RunRoot must be fresh: $run"
}
$existingControllers = @(Get-Process -Name 'star-controller' -ErrorAction SilentlyContinue)
if ($existingControllers.Count -ne 0) {
    throw 'ARM64 smoke requires no existing current-user Controller'
}

function Get-Sha256([string]$Path) {
    return 'sha256:' + (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-PeMachine([string]$Path) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        if ($reader.ReadUInt16() -ne 0x5a4d) {
            throw "not a PE executable: $Path"
        }
        $stream.Position = 0x3c
        $peOffset = $reader.ReadUInt32()
        $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne 0x00004550) {
            throw "invalid PE signature: $Path"
        }
        return $reader.ReadUInt16()
    } finally {
        $reader.Dispose()
        $stream.Dispose()
    }
}

function ConvertTo-TomlPath([string]$Path) {
    return $Path.Replace('\', '\\')
}

function Stop-IsolatedController {
    $expected = [IO.Path]::GetFullPath((Join-Path $install 'star-controller.exe'))
    foreach ($process in @(Get-Process -Name 'star-controller' -ErrorAction SilentlyContinue)) {
        try {
            $actual = [IO.Path]::GetFullPath($process.Path)
            if ($actual.Equals($expected, [StringComparison]::OrdinalIgnoreCase)) {
                Stop-Process -Id $process.Id -Force
                $null = $process.WaitForExit(5000)
            }
        } catch {
            if (-not $process.HasExited) {
                throw
            }
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

function Start-ControllerForOuterJobRunner {
    $startInfo = [Diagnostics.ProcessStartInfo]::new((Join-Path $install 'star-controller.exe'))
    $startInfo.ArgumentList.Add('--background')
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.Environment['APPDATA'] = $appData
    $startInfo.Environment['LOCALAPPDATA'] = $localAppData
    $startInfo.Environment['USERPROFILE'] = $userProfile
    $startInfo.Environment['STAR_CONTROL_RELEASE_TOOLS_DIR'] = $catalog
    $process = [Diagnostics.Process]::Start($startInfo)
    if ($null -eq $process) {
        throw 'runner could not prestart the native ARM64 Controller'
    }
    Start-Sleep -Seconds 2
    if ($process.HasExited) {
        $exitCode = $process.ExitCode
        $process.Dispose()
        throw "prestarted native ARM64 Controller exited with code $exitCode"
    }
    return $process
}

function Start-Gateway {
    $startInfo = [Diagnostics.ProcessStartInfo]::new((Join-Path $install 'star-mcp.exe'))
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.CreateNoWindow = $true
    $startInfo.Environment['APPDATA'] = $appData
    $startInfo.Environment['LOCALAPPDATA'] = $localAppData
    $startInfo.Environment['USERPROFILE'] = $userProfile
    $startInfo.Environment['STAR_CONTROL_RELEASE_TOOLS_DIR'] = $catalog
    $process = [Diagnostics.Process]::Start($startInfo)
    if ($null -eq $process) {
        throw 'failed to start native ARM64 Gateway'
    }
    return [pscustomobject]@{
        Process = $process
        ErrorTask = $process.StandardError.ReadToEndAsync()
    }
}

function Write-GatewayMessage($Gateway, $Message) {
    $line = $Message | ConvertTo-Json -Compress -Depth 100
    $Gateway.Process.StandardInput.WriteLine($line)
    $Gateway.Process.StandardInput.Flush()
}

function Read-GatewayResponse($Gateway, [int]$TimeoutMs = 30000) {
    $task = $Gateway.Process.StandardOutput.ReadLineAsync()
    if (-not $task.Wait($TimeoutMs)) {
        throw "Gateway response exceeded ${TimeoutMs}ms"
    }
    if ($null -eq $task.Result) {
        throw 'Gateway stdout closed before a response was received'
    }
    return $task.Result | ConvertFrom-Json -Depth 100
}

$script:requestId = 10
function Invoke-McpTool($Gateway, [string]$Name, $Arguments) {
    $id = $script:requestId
    $script:requestId++
    Write-GatewayMessage $Gateway ([ordered]@{
        jsonrpc = '2.0'
        id = $id
        method = 'tools/call'
        params = [ordered]@{
            name = $Name
            arguments = $Arguments
        }
    })
    $response = Read-GatewayResponse $Gateway 60000
    if ($null -ne $response.error) {
        throw "MCP JSON-RPC error for ${Name}: $($response.error | ConvertTo-Json -Compress -Depth 20)"
    }
    return $response.result
}

function Stop-Gateway($Gateway, [string]$StderrPath) {
    if ($null -eq $Gateway) {
        return
    }
    if (-not $Gateway.Process.HasExited) {
        $Gateway.Process.StandardInput.Close()
        if (-not $Gateway.Process.WaitForExit(5000)) {
            $Gateway.Process.Kill($true)
            $Gateway.Process.WaitForExit()
        }
    }
    $stderr = $Gateway.ErrorTask.GetAwaiter().GetResult()
    [IO.File]::WriteAllText($StderrPath, $stderr, [Text.UTF8Encoding]::new($false))
    $Gateway.Process.Dispose()
}

New-Item -ItemType Directory -Path $catalog, $manifestRoot, (Split-Path $evidencePath -Parent) -Force | Out-Null
$sourceRelease = Join-Path $repo 'target\release'
$binaries = @('star.exe', 'star-mcp.exe', 'star-controller.exe', 'star-fake-exe.exe')
foreach ($name in $binaries) {
    $source = Join-Path $sourceRelease $name
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "missing native release binary: $source"
    }
    $machine = Get-PeMachine $source
    if ($machine -ne 0xaa64) {
        throw "release binary is not ARM64 PE (0xaa64): $source machine=0x$($machine.ToString('x4'))"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $install $name)
}
Copy-Item -LiteralPath (Join-Path $repo 'catalog\tool-packages\star-control-core.toml') `
    -Destination (Join-Path $catalog 'star-control-core.toml')
$null = & $fixture -Phase prepare -RunRoot $run

$fakePath = Join-Path $workspace 'fixture-tools\star-fake-a.exe'
$fakeToml = ConvertTo-TomlPath $fakePath
$workingToml = ConvertTo-TomlPath (Split-Path $fakePath -Parent)
$manifest = @"
format_version = 1
package_id = "user.arm64-smoke"
package_version = "1.0.0"
display_name = "ARM64 native smoke"
description = "Native Windows ARM64 external process verification fixture."
enabled = true
backend_kinds = ["process"]

[[executables]]
executable_id = "fixture"
locator_kind = "absolute"
path = "$fakeToml"
update_policy = "follow_path"
protocol = "argv_v1"
interface_version_req = "*"
architectures = ["aarch64"]
working_directory = "fixed"
fixed_working_directory = "$workingToml"
isolation_compatibility = ["trusted_desktop"]
authenticode_policy = "record"

[[actions]]
tool_id = "user.arm64-smoke.echo"
backend_kind = "process"
backend_ref = "fixture"
display_name = "ARM64 Echo"
summary = "Run a native ARM64 fake executable."
description = "Proves native ARM64 process launch, Job containment, output drain, and result mapping."
permission_actions = ["local_read", "process_run"]
paid_action = "no"
idempotency = "read_only"

[[actions.parameters]]
name = "value"
type = "string"
description = "Value returned by the native ARM64 fixture."
required = true

[[actions.argv]]
kind = "literal"
value = "argv"

[[actions.argv]]
kind = "positional"
input = "value"

[actions.exit_codes]
success = [0]
empty = []
warning = []
retryable = []

[actions.output]
format = "text"
encoding = "utf8"
stderr_encoding = "utf8"
inline_limit_bytes = 4096
overflow = "artifact"
stdout_role = "data"
stderr_role = "log"

[actions.concurrency]
max_parallel = 1
exclusive_scope = "none"

[actions.cancel]
grace_ms = 500
"@
[IO.File]::WriteAllText($manifestPath, $manifest, [Text.UTF8Encoding]::new($false))

$env:APPDATA = $appData
$env:LOCALAPPDATA = $localAppData
$env:USERPROFILE = $userProfile
$env:STAR_CONTROL_RELEASE_TOOLS_DIR = $catalog
$script:star = Join-Path $install 'star.exe'
$gateway = $null
$prestartedController = $null
$gatewayStderrPath = Join-Path $run 'gateway.stderr.jsonl'
try {
    if ($PrestartController) {
        $prestartedController = Start-ControllerForOuterJobRunner
    }
    $start = Invoke-StarJson @('controller', 'start', '--background')
    if ($start.running -ne $true) {
        throw 'native ARM64 Controller did not start'
    }
    $candidateStatus = Invoke-StarJson @('tools', 'status', 'user.arm64-smoke', '--json')
    $candidate = @($candidateStatus.data.items)[0]
    if ($candidate.package_id -ne 'user.arm64-smoke' -or
        [string]::IsNullOrWhiteSpace([string]$candidate.candidate_manifest_hash)) {
        throw 'native ARM64 manifest candidate was not discovered'
    }
    $trust = Invoke-StarJson @(
        'tools', 'trust', 'user.arm64-smoke',
        '--manifest-hash', [string]$candidate.candidate_manifest_hash,
        '--json'
    )
    if ($trust.status -ne 'ok') {
        throw 'native ARM64 manifest trust failed'
    }

    $gateway = Start-Gateway
    Write-GatewayMessage $gateway ([ordered]@{
        jsonrpc = '2.0'
        id = 1
        method = 'initialize'
        params = [ordered]@{
            protocolVersion = '2025-11-25'
            capabilities = [ordered]@{}
            clientInfo = [ordered]@{ name = 'star-control-arm64-smoke'; version = '1.0.0' }
        }
    })
    $initialize = Read-GatewayResponse $gateway
    if ($initialize.result.protocolVersion -ne '2025-11-25' -or
        $initialize.result.serverInfo.name -ne 'star-control' -or
        $initialize.result.serverInfo.version -ne '0.1.0') {
        throw 'native ARM64 Gateway initialize response drifted'
    }
    $capabilityNames = @($initialize.result.capabilities.PSObject.Properties.Name)
    if ($capabilityNames.Count -ne 1 -or $capabilityNames[0] -ne 'tools' -or
        $initialize.result.capabilities.tools.PSObject.Properties.Name -contains 'listChanged') {
        throw 'native ARM64 Gateway advertised a forbidden capability'
    }
    Write-GatewayMessage $gateway ([ordered]@{
        jsonrpc = '2.0'
        method = 'notifications/initialized'
        params = [ordered]@{}
    })
    Write-GatewayMessage $gateway ([ordered]@{
        jsonrpc = '2.0'
        id = 2
        method = 'tools/list'
        params = [ordered]@{}
    })
    $toolsList = Read-GatewayResponse $gateway
    $toolNames = @($toolsList.result.tools.name)
    if ($toolNames.Count -ne 12 -or
        $toolNames[0] -ne 'star_tool_search' -or
        $toolNames[11] -ne 'star_approval_resolve') {
        throw 'native ARM64 Gateway did not expose the fixed 12-tool surface'
    }

    $registryStatus = Invoke-McpTool $gateway 'star_tool_registry_status' ([ordered]@{})
    $statusContent = $registryStatus.structuredContent
    if ($registryStatus.isError -ne $false -or
        $statusContent.status -ne 'ok' -or
        $statusContent.data.controller.instance_id -ne $start.instance_id) {
        throw 'native ARM64 Gateway to authenticated Controller IPC failed'
    }
    $search = Invoke-McpTool $gateway 'star_tool_search' ([ordered]@{
        query = 'user.arm64-smoke.echo'
        sources = @('user')
        limit = 10
    })
    $searchItems = @($search.structuredContent.data.items)
    if ($search.structuredContent.status -ne 'ok' -or
        $searchItems.Count -ne 1 -or
        $searchItems[0].readiness -ne 'ready') {
        throw 'native ARM64 user action was not ready in live search'
    }
    $describe = Invoke-McpTool $gateway 'star_tool_describe' ([ordered]@{
        tool_id = 'user.arm64-smoke.echo'
    })
    $descriptor = $describe.structuredContent.data
    if ($describe.structuredContent.status -ne 'ok' -or
        $descriptor.required_call_tool -ne 'star_tool_call_read_closed' -or
        $descriptor.executable_identity.architectures -notcontains 'aarch64') {
        throw 'native ARM64 descriptor identity or lane drifted'
    }
    $invoke = Invoke-McpTool $gateway 'star_tool_call_read_closed' ([ordered]@{
        tool_id = 'user.arm64-smoke.echo'
        descriptor_hash = [string]$descriptor.descriptor_hash
        arguments = [ordered]@{ value = 'native-arm64' }
        expected_revision = [int]$descriptor.registry_revision
        wait_mode = 'sync'
        idempotency_key = 'arm64-native-smoke'
    })
    $invokeContent = $invoke.structuredContent
    $externalResult = $invokeContent.data.result
    if ($invoke.isError -ne $false -or
        $invokeContent.status -ne 'ok' -or
        $externalResult.outcome -ne 'success' -or
        $externalResult.exit_code -ne 0 -or
        $externalResult.data.stdout -ne "argv:native-arm64`n" -or
        $invokeContent.data.output_provenance.external_untrusted_content -ne $true) {
        throw 'native ARM64 external process Runtime did not return the exact successful result'
    }

    $os = Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $evidence = [ordered]@{
        schema_id = 'star.mcp-arm64-native-smoke-evidence'
        schema_version = 1
        observed_at = [DateTimeOffset]::UtcNow.ToString('O')
        host = [ordered]@{
            os = [Environment]::OSVersion.VersionString
            display_version = [string]$os.DisplayVersion
            current_build = [int]$os.CurrentBuildNumber
            ubr = [int]$os.UBR
            os_architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
            process_architecture = [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
            processor_architecture = [string]$env:PROCESSOR_ARCHITECTURE
            runner_environment = [string]$env:RUNNER_ENVIRONMENT
            runner_image = [string]$env:ImageOS
        }
        binaries = [ordered]@{
            cli_sha256 = Get-Sha256 (Join-Path $install 'star.exe')
            gateway_sha256 = Get-Sha256 (Join-Path $install 'star-mcp.exe')
            controller_sha256 = Get-Sha256 (Join-Path $install 'star-controller.exe')
            fake_sha256 = Get-Sha256 $fakePath
            pe_machine = '0xaa64'
        }
        results = [ordered]@{
            controller_start = $true
            controller_launch_mode = if ($PrestartController) { 'runner_prestarted_outer_job_bound' } else { 'verified_client_fallback_start' }
            verified_existing_controller_identity = $true
            controller_instance_id = [string]$start.instance_id
            controller_pid = [int]$statusContent.data.controller.pid
            protocol_version = [string]$initialize.result.protocolVersion
            fixed_tool_count = $toolNames.Count
            only_tools_capability = $true
            authenticated_ipc = $true
            release_core_ready = @($statusContent.data.items | Where-Object { $_.package_id -eq 'star.control.core' -and $_.active_state -eq 'ready' }).Count -eq 1
            user_manifest_trusted = $true
            search_ready = $true
            descriptor_architecture = 'aarch64'
            external_process_outcome = [string]$externalResult.outcome
            external_process_exit_code = [int]$externalResult.exit_code
            external_process_stdout = [string]$externalResult.data.stdout
            external_process_untrusted_output = [bool]$invokeContent.data.output_provenance.external_untrusted_content
            gateway_pid = $gateway.Process.Id
        }
    }
    $json = $evidence | ConvertTo-Json -Depth 30
    [IO.File]::WriteAllText($evidencePath, $json + "`n", [Text.UTF8Encoding]::new($false))
    $json
} finally {
    try {
        Stop-Gateway $gateway $gatewayStderrPath
    } finally {
        Stop-IsolatedController
        if ($null -ne $prestartedController) {
            $prestartedController.Dispose()
        }
    }
}
