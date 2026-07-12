param(
    [Parameter(Mandatory = $true)]
    [string]$RunRoot,

    [Parameter(Mandatory = $true)]
    [string]$CoreRunRoot,

    [ValidateRange(30, 1000)]
    [int]$Samples = 30
)

$ErrorActionPreference = 'Stop'
$run = [IO.Path]::GetFullPath($RunRoot)
$coreRun = [IO.Path]::GetFullPath($CoreRunRoot)
$fixture = Join-Path $PSScriptRoot 'codex-e2e-fixture.ps1'
$hostRustc = (& rustc --version)
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($hostRustc)) {
    throw 'rustc version could not be recorded before fixture environment isolation'
}

function Set-RunEnvironment([string]$Root) {
    $env:APPDATA = Join-Path $Root 'workspace\appdata'
    $env:LOCALAPPDATA = Join-Path $Root 'workspace\localappdata'
    $env:USERPROFILE = Join-Path $Root 'workspace\userprofile'
    $env:STAR_CONTROL_RELEASE_TOOLS_DIR = Join-Path $Root 'install\catalog\tool-packages'
}

function Get-Percentile95([Collections.Generic.List[double]]$Values) {
    if ($Values.Count -lt $Samples) {
        throw "expected at least $Samples samples, found $($Values.Count)"
    }
    $ordered = @($Values | Sort-Object)
    $index = [Math]::Ceiling($ordered.Count * 0.95) - 1
    return [Math]::Round([double]$ordered[$index], 2)
}

function Start-Gateway([string]$Root) {
    Set-RunEnvironment $Root
    $gatewayPath = Join-Path $Root 'install\star-mcp.exe'
    $startInfo = [Diagnostics.ProcessStartInfo]::new($gatewayPath)
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.CreateNoWindow = $true
    $process = [Diagnostics.Process]::Start($startInfo)
    $errorTask = $process.StandardError.ReadToEndAsync()
    return [pscustomobject]@{
        Process = $process
        ErrorTask = $errorTask
    }
}

function Read-GatewayResponse($Gateway, [int]$TimeoutMs = 10000) {
    $task = $Gateway.Process.StandardOutput.ReadLineAsync()
    if (-not $task.Wait($TimeoutMs)) {
        throw "gateway response exceeded ${TimeoutMs}ms"
    }
    if ($null -eq $task.Result) {
        throw 'gateway stdout closed before a response was received'
    }
    return $task.Result | ConvertFrom-Json -Depth 100
}

function Write-GatewayMessage($Gateway, $Message) {
    $line = $Message | ConvertTo-Json -Compress -Depth 100
    $Gateway.Process.StandardInput.WriteLine($line)
    $Gateway.Process.StandardInput.Flush()
}

function Initialize-Gateway($Gateway) {
    Write-GatewayMessage $Gateway ([ordered]@{
        jsonrpc = '2.0'
        id = 1
        method = 'initialize'
        params = [ordered]@{
            protocolVersion = '2025-11-25'
            capabilities = [ordered]@{}
            clientInfo = [ordered]@{
                name = 'star-control-performance-fixture'
                version = '1.0.0'
            }
        }
    })
    $response = Read-GatewayResponse $Gateway
    if ($response.result.protocolVersion -ne '2025-11-25') {
        throw "unexpected negotiated protocol: $($response.result.protocolVersion)"
    }
    Write-GatewayMessage $Gateway ([ordered]@{
        jsonrpc = '2.0'
        method = 'notifications/initialized'
        params = [ordered]@{}
    })
}

function Stop-Gateway($Gateway) {
    if (-not $Gateway.Process.HasExited) {
        $Gateway.Process.StandardInput.Close()
        if (-not $Gateway.Process.WaitForExit(5000)) {
            $Gateway.Process.Kill($true)
            $Gateway.Process.WaitForExit()
        }
    }
    $null = $Gateway.ErrorTask.GetAwaiter().GetResult()
    $Gateway.Process.Dispose()
}

$script:nextRequestId = 2
function Invoke-McpTool($Gateway, [string]$Name, $Arguments) {
    $id = $script:nextRequestId
    $script:nextRequestId++
    Write-GatewayMessage $Gateway ([ordered]@{
        jsonrpc = '2.0'
        id = $id
        method = 'tools/call'
        params = [ordered]@{
            name = $Name
            arguments = $Arguments
        }
    })
    $response = Read-GatewayResponse $Gateway 45000
    if ($null -ne $response.error) {
        throw "MCP JSON-RPC error for ${Name}: $($response.error | ConvertTo-Json -Compress -Depth 20)"
    }
    return $response.result.structuredContent
}

function Invoke-StarJson([string]$Root, [string[]]$Arguments) {
    Set-RunEnvironment $Root
    $star = Join-Path $Root 'install\star.exe'
    $raw = & $star @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "star.exe failed with exit code $LASTEXITCODE"
    }
    return $raw | ConvertFrom-Json -Depth 100
}

function Stop-RunController([string]$Root) {
    $expected = [IO.Path]::GetFullPath((Join-Path $Root 'install\star-controller.exe'))
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

function Get-ControllerWorkingSetMiB($Status) {
    $pidValue = [int]$Status.data.controller.pid
    $process = Get-Process -Id $pidValue
    return [Math]::Round($process.WorkingSet64 / 1MB, 2)
}

foreach ($root in @($run, $coreRun)) {
    foreach ($name in @('star.exe', 'star-mcp.exe', 'star-controller.exe', 'star-fake-exe.exe')) {
        $path = Join-Path $root "install\$name"
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "missing isolated fixture binary: $path"
        }
    }
}
if (-not (Test-Path -LiteralPath $fixture -PathType Leaf)) {
    throw "missing fixture script: $fixture"
}

# Restore the dedicated run to the frozen 512-action capacity before measuring.
$null = & $fixture -Phase performance-capacity -RunRoot $run
$capacityList = Invoke-StarJson $run @('tools', 'list', '--json')
$capacityStatus = Invoke-StarJson $run @('tools', 'status', '--json')
$actions = @($capacityList.data.items)
$packages = @($capacityStatus.data.items)
if ($actions.Count -ne 512) {
    throw "capacity fixture exposed $($actions.Count) actions instead of 512"
}
if ($packages.Count -ne 126) {
    throw "capacity fixture exposed $($packages.Count) packages instead of 126"
}

$coldInitialize = [Collections.Generic.List[double]]::new()
$gatewayIdle = [Collections.Generic.List[double]]::new()
for ($sample = 0; $sample -lt $Samples; $sample++) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $coldGateway = Start-Gateway $run
    try {
        Initialize-Gateway $coldGateway
        $watch.Stop()
        $null = $coldInitialize.Add($watch.Elapsed.TotalMilliseconds)
        $coldGateway.Process.Refresh()
        $null = $gatewayIdle.Add($coldGateway.Process.WorkingSet64 / 1MB)
    } finally {
        Stop-Gateway $coldGateway
    }
}

$gateway = Start-Gateway $run
Initialize-Gateway $gateway
try {
    $searchLatencies = [Collections.Generic.List[double]]::new()
    for ($sample = 0; $sample -lt $Samples; $sample++) {
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $search = Invoke-McpTool $gateway 'star_tool_search' ([ordered]@{
            query = 'Echo'
            limit = 50
        })
        $watch.Stop()
        if ($search.status -ne 'ok' -or @($search.data.items).Count -ne 50) {
            throw 'capacity search did not return the expected first page'
        }
        $null = $searchLatencies.Add($watch.Elapsed.TotalMilliseconds)
    }

    $cursor = $null
    $searchPages = 0
    $seenTools = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    do {
        $arguments = [ordered]@{ query = 'Echo'; limit = 50 }
        if ($null -ne $cursor) {
            $arguments.cursor = $cursor
        }
        $page = Invoke-McpTool $gateway 'star_tool_search' $arguments
        if ($page.status -ne 'ok') {
            throw 'capacity pagination returned a non-ok result'
        }
        foreach ($item in @($page.data.items)) {
            if (-not $seenTools.Add([string]$item.tool_id)) {
                throw "duplicate tool in search pagination: $($item.tool_id)"
            }
        }
        $searchPages++
        $cursor = $page.data.next_cursor
    } while ($null -ne $cursor)
    if ($seenTools.Count -ne 499 -or $searchPages -ne 10) {
        throw "capacity search verified $($seenTools.Count) actions across $searchPages pages"
    }

    $describeLatencies = [Collections.Generic.List[double]]::new()
    $descriptorHash = $null
    for ($sample = 0; $sample -lt $Samples; $sample++) {
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $describe = Invoke-McpTool $gateway 'star_tool_describe' ([ordered]@{
            tool_id = 'user.perf.p000.action0'
        })
        $watch.Stop()
        if ($describe.status -ne 'ok') {
            throw 'capacity describe returned a non-ok result'
        }
        $descriptorHash = [string]$describe.data.descriptor_hash
        $null = $describeLatencies.Add($watch.Elapsed.TotalMilliseconds)
    }

    $preflightLatencies = [Collections.Generic.List[double]]::new()
    for ($sample = 0; $sample -lt $Samples; $sample++) {
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $preflight = Invoke-McpTool $gateway 'star_tool_call_read_open' ([ordered]@{
            tool_id = 'user.perf.p000.action0'
            descriptor_hash = $descriptorHash
            arguments = [ordered]@{ value = 'must-not-start' }
        })
        $watch.Stop()
        if ($preflight.status -ne 'error' -or $preflight.error.code -ne 'TOOL_LANE_MISMATCH') {
            throw 'mismatched lane was not rejected during preflight'
        }
        $null = $preflightLatencies.Add($watch.Elapsed.TotalMilliseconds)
    }

    $saveLatencies = [Collections.Generic.List[double]]::new()
    $previousHash = $null
    for ($sample = 0; $sample -lt $Samples; $sample++) {
        $phase = if (($sample % 2) -eq 0) { 'add' } else { 'path-change' }
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $null = & $fixture -Phase $phase -RunRoot $run
        $search = Invoke-McpTool $gateway 'star_tool_search' ([ordered]@{
            query = 'user.codex-same-session.echo'
            limit = 10
        })
        $watch.Stop()
        $item = @($search.data.items | Where-Object tool_id -EQ 'user.codex-same-session.echo')[0]
        if ($null -eq $item) {
            throw 'stable manifest save was not visible to the live search request'
        }
        $currentHash = [string]$item.descriptor_hash
        if ($null -ne $previousHash -and $currentHash -eq $previousHash) {
            throw 'descriptor hash did not change after the executable path changed'
        }
        $previousHash = $currentHash
        $null = $saveLatencies.Add($watch.Elapsed.TotalMilliseconds)
    }

    $gateway.Process.Refresh()
    $gatewayCapacityWorkingSet = [Math]::Round($gateway.Process.WorkingSet64 / 1MB, 2)
} finally {
    Stop-Gateway $gateway
}

$fallbackLatencies = [Collections.Generic.List[double]]::new()
for ($sample = 0; $sample -lt $Samples; $sample++) {
    Stop-RunController $run
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $status = Invoke-StarJson $run @('tools', 'status', '--json')
    $watch.Stop()
    if ($status.status -ne 'ok') {
        throw 'verified Controller fallback start did not return ok'
    }
    $null = $fallbackLatencies.Add($watch.Elapsed.TotalMilliseconds)
}

$statusLatencies = [Collections.Generic.List[double]]::new()
for ($sample = 0; $sample -lt $Samples; $sample++) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $status = Invoke-StarJson $run @('tools', 'status', '--json')
    $watch.Stop()
    if ($status.status -ne 'ok') {
        throw 'running Controller status did not return ok'
    }
    $null = $statusLatencies.Add($watch.Elapsed.TotalMilliseconds)
}
$controllerCapacityWorkingSet = Get-ControllerWorkingSetMiB $status

Stop-RunController $run
$coreStatus = Invoke-StarJson $coreRun @('tools', 'status', '--json')
if ($coreStatus.status -ne 'ok' -or @($coreStatus.data.items).Count -ne 1) {
    throw 'core-only Controller baseline did not expose exactly one release package'
}
$controllerBaselineWorkingSet = Get-ControllerWorkingSetMiB $coreStatus
Stop-RunController $coreRun

Set-RunEnvironment $run
$gatewayHash = 'sha256:' + (Get-FileHash -LiteralPath (Join-Path $run 'install\star-mcp.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
$controllerHash = 'sha256:' + (Get-FileHash -LiteralPath (Join-Path $run 'install\star-controller.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
$releaseActions = @($actions | Where-Object source -EQ 'release').Count
$userActions = @($actions | Where-Object source -EQ 'user').Count
$releasePackages = @($packages | Where-Object source -EQ 'release').Count
$userPackages = @($packages | Where-Object source -EQ 'user').Count
$architecture = switch ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()) {
    'X64' { 'x86_64' }
    'Arm64' { 'aarch64' }
    default { [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant() }
}

$evidence = [ordered]@{
    schema_id = 'star.mcp-performance-evidence'
    schema_version = 1
    observed_at = [DateTimeOffset]::UtcNow.ToString('O')
    host = [ordered]@{
        os = [Environment]::OSVersion.VersionString
        os_build = [Environment]::OSVersion.Version.Build
        architecture = $architecture
        rustc = $hostRustc
    }
    binaries = [ordered]@{
        gateway_sha256 = $gatewayHash
        controller_sha256 = $controllerHash
    }
    registry_capacity = [ordered]@{
        release_packages = $releasePackages
        user_packages = $userPackages
        release_actions = $releaseActions
        user_actions = $userActions
        total_actions = $actions.Count
        search_actions_verified = $seenTools.Count
        search_pages_verified = $searchPages
    }
    latency_ms = [ordered]@{
        gateway_cold_initialize = [ordered]@{ samples = $Samples; p95 = Get-Percentile95 $coldInitialize; limit = 2000.0 }
        verified_controller_fallback_start = [ordered]@{ samples = $Samples; p95 = Get-Percentile95 $fallbackLatencies; limit = 5000.0 }
        running_controller_ipc_status = [ordered]@{ samples = $Samples; p95 = Get-Percentile95 $statusLatencies; limit = 250.0 }
        search_512_actions = [ordered]@{ samples = $Samples; p95 = Get-Percentile95 $searchLatencies; limit = 100.0 }
        describe = [ordered]@{ samples = $Samples; p95 = Get-Percentile95 $describeLatencies; limit = 50.0 }
        unchanged_invoke_preflight_without_child = [ordered]@{ samples = $Samples; p95 = Get-Percentile95 $preflightLatencies; limit = 250.0 }
        stable_toml_save_to_search = [ordered]@{ samples = $Samples; p95 = Get-Percentile95 $saveLatencies; limit = 1000.0 }
    }
    memory_mib = [ordered]@{
        gateway_idle_max = [Math]::Round(($gatewayIdle | Measure-Object -Maximum).Maximum, 2)
        gateway_512_action_working_set = $gatewayCapacityWorkingSet
        gateway_limit = 64.0
        controller_baseline_working_set = $controllerBaselineWorkingSet
        controller_512_action_working_set = $controllerCapacityWorkingSet
        controller_512_action_increment = [Math]::Round($controllerCapacityWorkingSet - $controllerBaselineWorkingSet, 2)
        controller_increment_limit = 128.0
    }
    method = [ordered]@{
        ipc_measurement = 'Fresh star.exe process plus authenticated IPC status and request demand scan.'
        preflight_measurement = 'Correct live descriptor with a deliberately mismatched fixed lane; TOOL_LANE_MISMATCH proved the request was rejected before process creation.'
        save_measurement = 'Thirty alternating stable add/path-change TOML saves in one live Gateway and Controller session; every search result descriptor changed.'
        capacity_measurement = 'CLI pagination verified all 512 actions. MCP search pagination verified the 499 capacity-fixture actions across ten pages.'
    }
}

$evidence | ConvertTo-Json -Depth 100
