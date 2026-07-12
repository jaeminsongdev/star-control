param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('prepare', 'add', 'path-change', 'byte-swap', 'invalidate', 'performance-capacity', 'snapshot', 'kill-gateway', 'marker-state')]
    [string]$Phase,

    [Parameter(Mandatory = $true)]
    [string]$RunRoot
)

$ErrorActionPreference = 'Stop'
$run = [IO.Path]::GetFullPath($RunRoot)
$install = Join-Path $run 'install'
$workspace = Join-Path $run 'workspace'
$appData = Join-Path $workspace 'appdata'
$toolRoot = Join-Path $workspace 'fixture-tools'
$manifestRoot = Join-Path $appData 'Star-Control\tools.d'
$manifestPath = Join-Path $manifestRoot 'codex-same-session.toml'
$toolA = Join-Path $toolRoot 'star-fake-a.exe'
$toolB = Join-Path $toolRoot 'star-fake-b.exe'
$marker = Join-Path $toolRoot 'paid-side-effect.txt'
$longPid = Join-Path $toolRoot 'long-child.pid'

function ConvertTo-TomlPath([string]$Path) {
    return $Path.Replace('\', '\\')
}

function Write-Manifest([string]$ExecutablePath) {
    $exe = ConvertTo-TomlPath $ExecutablePath
    $working = ConvertTo-TomlPath $toolRoot
    $manifest = @"
format_version = 1
package_id = "user.codex-same-session"
package_version = "1.0.0"
display_name = "Codex same-session fixture"
description = "Local-only release-gate fixture for actual Codex MCP host verification."
enabled = true
backend_kinds = ["process"]

[[executables]]
executable_id = "fixture"
locator_kind = "absolute"
path = "$exe"
update_policy = "follow_path"
protocol = "argv_v1"
interface_version_req = "*"
architectures = ["x86_64"]
working_directory = "fixed"
fixed_working_directory = "$working"
isolation_compatibility = ["trusted_desktop"]
authenticode_policy = "record"

[[actions]]
tool_id = "user.codex-same-session.echo"
backend_kind = "process"
backend_ref = "fixture"
display_name = "Echo same-session value"
summary = "Echo a local fixture value."
description = "Runs a local fake executable without network or persistent side effects."
permission_actions = ["local_read", "process_run"]
paid_action = "no"
idempotency = "read_only"

[[actions.parameters]]
name = "value"
type = "string"
description = "Value returned by the local fixture."
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

[[actions]]
tool_id = "user.codex-same-session.paid"
backend_kind = "process"
backend_ref = "fixture"
display_name = "Paid gate fixture"
summary = "Verify paid approval before a local marker write."
description = "Would write one local marker, but the paid gate must stop it before process creation."
permission_actions = ["local_write", "process_run", "paid_action"]
paid_action = "yes"
idempotency = "non_idempotent"

[[actions.parameters]]
name = "marker"
type = "string"
description = "Absolute marker path used only by the isolated fixture."
required = true

[[actions.argv]]
kind = "literal"
value = "marker-sleep"

[[actions.argv]]
kind = "positional"
input = "marker"

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

[[actions]]
tool_id = "user.codex-same-session.long"
backend_kind = "process"
backend_ref = "fixture"
display_name = "Long cancellable fixture"
summary = "Start a cancellable local operation."
description = "Writes its child PID and waits so Operation lookup and cancellation can be verified."
permission_actions = ["local_read", "process_run"]
paid_action = "no"
idempotency = "non_idempotent"
execution_mode = "detachable"
expected_duration_ms = 30000
cancel_mode = "terminate_job"

[[actions.parameters]]
name = "pid_file"
type = "string"
description = "Absolute PID evidence path inside the isolated fixture root."
required = true

[[actions.argv]]
kind = "literal"
value = "record-pid-sleep"

[[actions.argv]]
kind = "positional"
input = "pid_file"

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
}

function Write-PerformanceCapacityManifests {
    Write-Manifest $toolA
    $template = [IO.File]::ReadAllText($manifestPath)
    $parts = $template.Split(@('[[actions]]'), [StringSplitOptions]::None)
    if ($parts.Count -lt 2) {
        throw 'capacity fixture template must contain at least one action'
    }

    $header = $parts[0]
    $action = $parts[1]
    for ($packageIndex = 0; $packageIndex -lt 125; $packageIndex++) {
        $packageId = 'user.perf.p{0:d3}' -f $packageIndex
        $actionCount = if ($packageIndex -eq 0) { 3 } else { 4 }
        $contents = $header.Replace('user.codex-same-session', $packageId)
        for ($actionIndex = 0; $actionIndex -lt $actionCount; $actionIndex++) {
            $actionId = "$packageId.action$actionIndex"
            $contents += '[[actions]]'
            $contents += $action.Replace(
                'user.codex-same-session.echo',
                $actionId
            )
        }
        $target = if ($packageIndex -eq 0) {
            $manifestPath
        } else {
            Join-Path $manifestRoot ('perf-p{0:d3}.toml' -f $packageIndex)
        }
        [IO.File]::WriteAllText($target, $contents, [Text.UTF8Encoding]::new($false))
    }

    $configPath = Join-Path $appData 'Star-Control\config.toml'
    [IO.File]::WriteAllText(
        $configPath,
        "schema_version = 1`npolicy_profile = `"star.policy-profile.personal-auto`"`n",
        [Text.UTF8Encoding]::new($false)
    )
}

switch ($Phase) {
    'prepare' {
        New-Item -ItemType Directory -Path $toolRoot, $manifestRoot -Force | Out-Null
        $gateway = Join-Path $install 'star-mcp.exe'
        $controller = Join-Path $install 'star-controller.exe'
        if (-not (Test-Path -LiteralPath $gateway) -or -not (Test-Path -LiteralPath $controller)) {
            throw 'isolated install must contain star-mcp.exe and star-controller.exe'
        }
        $installManifest = [ordered]@{
            schema_id = 'star.controller-install-manifest'
            schema_version = 1
            product_version = '0.1.0'
            gateway_sha256 = 'sha256:' + (Get-FileHash -LiteralPath $gateway -Algorithm SHA256).Hash.ToLowerInvariant()
            controller_path = [IO.Path]::GetFullPath($controller)
            controller_sha256 = 'sha256:' + (Get-FileHash -LiteralPath $controller -Algorithm SHA256).Hash.ToLowerInvariant()
        } | ConvertTo-Json -Compress
        [IO.File]::WriteAllText(
            (Join-Path $install 'star-control-install.v1.json'),
            $installManifest,
            [Text.UTF8Encoding]::new($false)
        )
        Copy-Item -LiteralPath (Join-Path $install 'star-fake-exe.exe') -Destination $toolA -Force
        Copy-Item -LiteralPath (Join-Path $install 'star-fake-exe.exe') -Destination $toolB -Force
        [pscustomobject]@{
            phase = $Phase
            tool_a = $toolA
            tool_b = $toolB
            tool_a_sha256 = 'sha256:' + (Get-FileHash -LiteralPath $toolA -Algorithm SHA256).Hash.ToLowerInvariant()
            tool_b_sha256 = 'sha256:' + (Get-FileHash -LiteralPath $toolB -Algorithm SHA256).Hash.ToLowerInvariant()
            marker = $marker
            long_pid_file = $longPid
        } | ConvertTo-Json -Compress
    }
    'add' {
        Write-Manifest $toolA
        [pscustomobject]@{ phase = $Phase; manifest = $manifestPath; executable = $toolA } | ConvertTo-Json -Compress
    }
    'path-change' {
        Write-Manifest $toolB
        [pscustomobject]@{ phase = $Phase; manifest = $manifestPath; executable = $toolB } | ConvertTo-Json -Compress
    }
    'byte-swap' {
        $stream = [IO.File]::Open($toolB, [IO.FileMode]::Append, [IO.FileAccess]::Write, [IO.FileShare]::Read)
        try {
            $overlay = [Text.Encoding]::ASCII.GetBytes("`nSTAR-CONTROL-CODEX-E2E-V2`n")
            $stream.Write($overlay, 0, $overlay.Length)
            $stream.Flush($true)
        } finally {
            $stream.Dispose()
        }
        [pscustomobject]@{
            phase = $Phase
            executable = $toolB
            sha256 = 'sha256:' + (Get-FileHash -LiteralPath $toolB -Algorithm SHA256).Hash.ToLowerInvariant()
        } | ConvertTo-Json -Compress
    }
    'performance-capacity' {
        Write-PerformanceCapacityManifests
        [pscustomobject]@{
            phase = $Phase
            package_count = 125
            action_count = 499
            expected_total_actions_with_core = 512
            manifest_root = $manifestRoot
        } | ConvertTo-Json -Compress
    }
    'invalidate' {
        [IO.File]::WriteAllText($manifestPath, 'format_version = [invalid', [Text.UTF8Encoding]::new($false))
        [pscustomobject]@{ phase = $Phase; manifest = $manifestPath } | ConvertTo-Json -Compress
    }
    'snapshot' {
        $names = @('codex', 'star-mcp', 'star-controller')
        $items = foreach ($name in $names) {
            Get-Process -Name $name -ErrorAction SilentlyContinue |
                Where-Object { $_.Path -and [IO.Path]::GetFullPath($_.Path).StartsWith($run, [StringComparison]::OrdinalIgnoreCase) } |
                ForEach-Object {
                    [pscustomobject]@{
                        name = $_.ProcessName
                        pid = $_.Id
                        path = $_.Path
                        started_at = $_.StartTime.ToUniversalTime().ToString('o')
                    }
                }
        }
        [pscustomobject]@{
            phase = $Phase
            timestamp = [DateTime]::UtcNow.ToString('o')
            processes = @($items)
            manifest_sha256 = if (Test-Path -LiteralPath $manifestPath) { 'sha256:' + (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant() } else { $null }
            executable_sha256 = if (Test-Path -LiteralPath $toolB) { 'sha256:' + (Get-FileHash -LiteralPath $toolB -Algorithm SHA256).Hash.ToLowerInvariant() } else { $null }
        } | ConvertTo-Json -Depth 5 -Compress
    }
    'kill-gateway' {
        $gateway = Get-Process -Name 'star-mcp' -ErrorAction Stop |
            Where-Object { $_.Path -and [IO.Path]::GetFullPath($_.Path).StartsWith($run, [StringComparison]::OrdinalIgnoreCase) } |
            Select-Object -First 1
        if (-not $gateway) { throw 'isolated star-mcp process not found' }
        $gatewayPid = $gateway.Id
        Stop-Process -Id $gatewayPid -Force
        [pscustomobject]@{ phase = $Phase; killed_gateway_pid = $gatewayPid; timestamp = [DateTime]::UtcNow.ToString('o') } | ConvertTo-Json -Compress
    }
    'marker-state' {
        $pidValue = if (Test-Path -LiteralPath $longPid) { [IO.File]::ReadAllText($longPid).Trim() } else { $null }
        [pscustomobject]@{
            phase = $Phase
            paid_marker_exists = Test-Path -LiteralPath $marker
            long_pid = $pidValue
            long_process_running = if ($pidValue) { [bool](Get-Process -Id ([int]$pidValue) -ErrorAction SilentlyContinue) } else { $false }
        } | ConvertTo-Json -Compress
    }
}
