param(
    [Parameter(Mandatory = $true)]
    [string]$RunRoot,

    [Parameter(Mandatory = $true)]
    [string]$InspectorCache,

    [Parameter(Mandatory = $false)]
    [string]$ReleaseBinaryRoot
)

$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$run = [IO.Path]::GetFullPath($RunRoot)
$cache = [IO.Path]::GetFullPath($InspectorCache)
$releaseBinaries = if ([string]::IsNullOrWhiteSpace($ReleaseBinaryRoot)) {
    [IO.Path]::GetFullPath((Join-Path $repo 'target\release'))
} else {
    [IO.Path]::GetFullPath($ReleaseBinaryRoot)
}
$fixtureBinaries = [IO.Path]::GetFullPath((Join-Path $repo 'target\release'))
$install = Join-Path $run 'install'
$rawRoot = Join-Path $run 'raw'
$workspace = Join-Path $run 'workspace'
$catalog = Join-Path $install 'catalog\tool-packages'
$fixture = Join-Path $PSScriptRoot 'codex-e2e-fixture.ps1'

if (Test-Path -LiteralPath $run) {
    throw "RunRoot must be a fresh path: $run"
}
if (-not (Test-Path -LiteralPath $cache -PathType Container)) {
    throw "Inspector cache does not exist: $cache"
}
if (-not (Test-Path -LiteralPath $releaseBinaries -PathType Container)) {
    throw "Release binary root does not exist: $releaseBinaries"
}

function Get-Sha256([string]$Path) {
    return 'sha256:' + (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-RelativeRepoPath([string]$Path) {
    return [IO.Path]::GetRelativePath($repo, $Path).Replace('\', '/')
}

function Get-RawAttestation([string]$Path) {
    $info = Get-Item -LiteralPath $Path
    $text = [IO.File]::ReadAllText($Path)
    return [ordered]@{
        path = Get-RelativeRepoPath $Path
        bytes = $info.Length
        lines = if ($text.Length -eq 0) { 0 } else { [regex]::Matches($text, "`n").Count + 1 }
        sha256 = Get-Sha256 $Path
    }
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

function Get-IsolatedController {
    $expected = [IO.Path]::GetFullPath((Join-Path $install 'star-controller.exe'))
    $matches = @(
        Get-Process -Name 'star-controller' -ErrorAction SilentlyContinue |
            Where-Object {
                try {
                    [IO.Path]::GetFullPath($_.Path).Equals(
                        $expected,
                        [StringComparison]::OrdinalIgnoreCase
                    )
                } catch {
                    $false
                }
            }
    )
    if ($matches.Count -ne 1) {
        throw "expected one isolated Controller, found $($matches.Count)"
    }
    return $matches[0]
}

function Invoke-Inspector(
    [string]$Role,
    [string[]]$Arguments,
    [string]$NodePath,
    [string]$CliPath,
    [string]$CliWorkingDirectory,
    [string]$GatewayPath
) {
    $stdoutPath = Join-Path $rawRoot "$Role.stdout.json"
    $stderrPath = Join-Path $rawRoot "$Role.stderr.jsonl"
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $NodePath
    $startInfo.WorkingDirectory = $CliWorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.CreateNoWindow = $true
    foreach ($argument in @($CliPath, '--cli', $GatewayPath) + $Arguments) {
        $startInfo.ArgumentList.Add($argument)
    }
    $startInfo.Environment['APPDATA'] = Join-Path $workspace 'appdata'
    $startInfo.Environment['LOCALAPPDATA'] = Join-Path $workspace 'localappdata'
    $startInfo.Environment['USERPROFILE'] = Join-Path $workspace 'userprofile'
    $startInfo.Environment['STAR_CONTROL_RELEASE_TOOLS_DIR'] = $catalog

    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $watch = [Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) {
        throw "failed to start Inspector role $Role"
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(60000)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw "Inspector role $Role exceeded 60000ms"
    }
    $stdout = $stdoutTask.GetAwaiter().GetResult()
    $stderr = $stderrTask.GetAwaiter().GetResult()
    $watch.Stop()
    $exitCode = $process.ExitCode
    $process.Dispose()

    [IO.File]::WriteAllText($stdoutPath, $stdout, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($stderrPath, $stderr, [Text.UTF8Encoding]::new($false))
    if ($exitCode -ne 0) {
        throw "Inspector role $Role failed with exit code ${exitCode}: $stderr"
    }
    $parsed = $stdout | ConvertFrom-Json -Depth 100
    return [pscustomobject]@{
        Data = $parsed
        ExitCode = $exitCode
        DurationMs = [Math]::Round($watch.Elapsed.TotalMilliseconds, 2)
        Stdout = Get-RawAttestation $stdoutPath
        Stderr = Get-RawAttestation $stderrPath
    }
}

$fixedTools = @(
    [ordered]@{ name = 'star_tool_search'; title = 'Search Star-Control Tools'; description = 'Search the current Star-Control live registry for an action. Call describe before invoking a result.'; readOnly = $true; destructive = $false; idempotent = $true; openWorld = $false },
    [ordered]@{ name = 'star_tool_describe'; title = 'Describe a Star-Control Tool'; description = 'Return the current Schema, risk lane, executable readiness, and descriptor hash for one action.'; readOnly = $true; destructive = $false; idempotent = $true; openWorld = $false },
    [ordered]@{ name = 'star_tool_registry_status'; title = 'Inspect the Tool Registry'; description = 'Inspect live registry revisions, packages, watchers, last-known-good state, and diagnostics.'; readOnly = $true; destructive = $false; idempotent = $true; openWorld = $false },
    [ordered]@{ name = 'star_tool_call_read_closed'; title = 'Run a Local Read Action'; description = 'Invoke the described local read-only action. The descriptor must require this exact lane.'; readOnly = $true; destructive = $false; idempotent = $false; openWorld = $false },
    [ordered]@{ name = 'star_tool_call_read_open'; title = 'Run an External Read Action'; description = 'Invoke the described read-only action that may access external systems.'; readOnly = $true; destructive = $false; idempotent = $false; openWorld = $true },
    [ordered]@{ name = 'star_tool_call_write_closed'; title = 'Run a Local Write Action'; description = 'Invoke the described non-destructive local mutation.'; readOnly = $false; destructive = $false; idempotent = $false; openWorld = $false },
    [ordered]@{ name = 'star_tool_call_destructive_closed'; title = 'Run a Destructive Local Action'; description = 'Invoke the described destructive local action after policy checks.'; readOnly = $false; destructive = $true; idempotent = $false; openWorld = $false },
    [ordered]@{ name = 'star_tool_call_write_open'; title = 'Run an External Write Action'; description = 'Invoke the described non-destructive action that changes or uses an external system.'; readOnly = $false; destructive = $false; idempotent = $false; openWorld = $true },
    [ordered]@{ name = 'star_tool_call_destructive_open'; title = 'Run a Destructive External Action'; description = 'Invoke the described destructive external action after policy checks.'; readOnly = $false; destructive = $true; idempotent = $false; openWorld = $true },
    [ordered]@{ name = 'star_tool_operation_get'; title = 'Get an Operation'; description = 'Read durable status, progress, and result for a Star-Control operation.'; readOnly = $true; destructive = $false; idempotent = $true; openWorld = $false },
    [ordered]@{ name = 'star_tool_operation_cancel'; title = 'Cancel an Operation'; description = 'Request cancellation of a durable operation and return its current state.'; readOnly = $false; destructive = $true; idempotent = $true; openWorld = $true },
    [ordered]@{ name = 'star_approval_resolve'; title = 'Resolve an Approval'; description = "Record the user's approve or deny decision for the exact approval scope."; readOnly = $false; destructive = $true; idempotent = $true; openWorld = $true }
)
$requiredCoreToolIds = @(
    'star.core.goal.start',
    'star.core.goal.answer',
    'star.core.plan.get',
    'star.core.plan.update',
    'star.core.run.continue',
    'star.core.status.get',
    'star.core.goal.pause',
    'star.core.goal.resume',
    'star.core.goal.cancel',
    'star.core.evidence.get',
    'star.core.merge.status',
    'star.core.handoff.get',
    'star.core.doctor',
    'star.core.project.list',
    'star.core.project.status',
    'star.core.validation.plan',
    'star.core.validation.run'
)

$existingControllers = @(Get-Process -Name 'star-controller' -ErrorAction SilentlyContinue)
if ($existingControllers.Count -ne 0) {
    $details = @(
        $existingControllers | ForEach-Object {
            try { "pid=$($_.Id) path=$($_.Path)" } catch { "pid=$($_.Id) path=<unavailable>" }
        }
    ) -join '; '
    throw "Inspector fixture requires no existing current-user Controller: $details"
}

New-Item -ItemType Directory -Path $catalog, $rawRoot -Force | Out-Null
foreach ($name in @('star.exe', 'star-mcp.exe', 'star-controller.exe')) {
    $source = Join-Path $releaseBinaries $name
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "missing candidate release binary: $source"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $install $name)
}
$fakeSource = Join-Path $fixtureBinaries 'star-fake-exe.exe'
if (-not (Test-Path -LiteralPath $fakeSource -PathType Leaf)) {
    throw "missing test-only fixture binary: $fakeSource"
}
Copy-Item -LiteralPath $fakeSource -Destination (Join-Path $install 'star-fake-exe.exe')
Copy-Item -LiteralPath (Join-Path $repo 'catalog\tool-packages\star-control-core.toml') `
    -Destination (Join-Path $catalog 'star-control-core.toml')
Copy-Item -LiteralPath (Join-Path $repo 'catalog\tool-packages\schemas') `
    -Destination (Join-Path $catalog 'schemas') -Recurse
$null = & $fixture -Phase prepare -RunRoot $run

$inspectorPackages = @(
    Get-ChildItem -LiteralPath $cache -Recurse -Filter package.json -File |
        Where-Object { $_.FullName -match '\\node_modules\\@modelcontextprotocol\\inspector\\package.json$' }
)
if ($inspectorPackages.Count -ne 1) {
    throw "expected one installed Inspector package, found $($inspectorPackages.Count)"
}
$inspectorPackagePath = $inspectorPackages[0].FullName
$nodeModules = Split-Path (Split-Path (Split-Path $inspectorPackagePath -Parent) -Parent) -Parent
$inspectorCliPackagePath = Join-Path $nodeModules '@modelcontextprotocol\inspector-cli\package.json'
$sdkPackagePath = Join-Path $nodeModules '@modelcontextprotocol\sdk\package.json'
$cliPath = Join-Path $nodeModules '@modelcontextprotocol\inspector-cli\build\cli.js'
$cliWorkingDirectory = Split-Path $cliPath -Parent
$lockPath = Join-Path (Split-Path $nodeModules -Parent) 'package-lock.json'
foreach ($path in @($inspectorCliPackagePath, $sdkPackagePath, $cliPath, $lockPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing Inspector installation file: $path"
    }
}

$inspectorPackage = Get-Content -LiteralPath $inspectorPackagePath -Raw | ConvertFrom-Json
$inspectorCliPackage = Get-Content -LiteralPath $inspectorCliPackagePath -Raw | ConvertFrom-Json
$sdkPackage = Get-Content -LiteralPath $sdkPackagePath -Raw | ConvertFrom-Json
$lock = Get-Content -LiteralPath $lockPath -Raw | ConvertFrom-Json -Depth 100 -AsHashtable
$lockedInspector = $lock['packages']['node_modules/@modelcontextprotocol/inspector']
$lockedInspectorCli = $lock['packages']['node_modules/@modelcontextprotocol/inspector-cli']
$lockedSdk = $lock['packages']['node_modules/@modelcontextprotocol/sdk']
if ($inspectorPackage.version -ne '0.22.0' -or
    $inspectorCliPackage.version -ne '0.22.0' -or
    $lockedInspector['integrity'] -ne 'sha512-HUyvF+6C3e/sL3wZSc71Li1SkuWysixblFpVdm8csJKBOlT2kNG5kWP0AAgdXRiRWRZ27ZajNtagYgwoJ+QBpQ==' -or
    $lockedInspectorCli['integrity'] -ne 'sha512-Z3NHqa1zTjZyfcd3qJcpIwqiSG7QlR3YkYPFAIoMsPw3hId0AoHtlG4SRueJzymCsF9Rqein3NzUn3qT+aqUBw==') {
    throw 'Inspector exact-version installation or registry integrity does not match 0.22.0'
}

$nodePath = (Get-Command node -CommandType Application).Source
$gatewayPath = Join-Path $install 'star-mcp.exe'
$controllerPath = Join-Path $install 'star-controller.exe'
$toolsList = $null
$registryStatus = $null
$search = $null
$coreSearch = $null
$coreDescriptions = @()
$controllerAfterStatus = $null
$controllerAfterSearch = $null
try {
    $toolsList = Invoke-Inspector 'tools-list' @('--method', 'tools/list') $nodePath $cliPath $cliWorkingDirectory $gatewayPath
    $listedTools = @($toolsList.Data.tools)
    if ($listedTools.Count -ne $fixedTools.Count) {
        throw "expected 12 fixed tools, found $($listedTools.Count)"
    }
    if ($toolsList.Data.PSObject.Properties.Name -contains 'nextCursor') {
        throw 'fixed tools/list unexpectedly returned nextCursor'
    }
    for ($index = 0; $index -lt $fixedTools.Count; $index++) {
        $expected = $fixedTools[$index]
        $actual = $listedTools[$index]
        if ($actual.name -ne $expected.name -or
            $actual.title -ne $expected.title -or
            $actual.description -ne $expected.description -or
            $actual.annotations.readOnlyHint -ne $expected.readOnly -or
            $actual.annotations.destructiveHint -ne $expected.destructive -or
            $actual.annotations.idempotentHint -ne $expected.idempotent -or
            $actual.annotations.openWorldHint -ne $expected.openWorld -or
            $actual.annotations.PSObject.Properties.Name -contains 'title' -or
            $actual.PSObject.Properties.Name -contains '_meta' -or
            $actual.PSObject.Properties.Name -contains 'execution') {
            throw "fixed tool metadata drift at index ${index}: $($expected.name)"
        }
        $inputId = "urn:star-control:schema:star.mcp.$($expected.name).input:v1"
        $resultId = "urn:star-control:schema:star.mcp.$($expected.name).result:v1"
        if ($actual.inputSchema.'$schema' -ne 'https://json-schema.org/draft/2020-12/schema' -or
            $actual.outputSchema.'$schema' -ne 'https://json-schema.org/draft/2020-12/schema' -or
            $actual.inputSchema.'$id' -ne $inputId -or
            $actual.outputSchema.'$id' -ne $resultId -or
            $actual.inputSchema.additionalProperties -ne $false -or
            $actual.outputSchema.additionalProperties -ne $false) {
            throw "fixed tool Schema drift: $($expected.name)"
        }
    }
    if ($toolsList.Stdout.bytes -le 0 -or
        [IO.File]::ReadAllText((Join-Path $rawRoot 'tools-list.stdout.json')).Contains('"$ref"')) {
        throw 'tools/list output was empty or contained a remote Schema reference'
    }

    $registryStatus = Invoke-Inspector 'registry-status' @(
        '--method', 'tools/call',
        '--tool-name', 'star_tool_registry_status'
    ) $nodePath $cliPath $cliWorkingDirectory $gatewayPath
    $statusContent = $registryStatus.Data.structuredContent
    if ($registryStatus.Data.isError -ne $false -or
        $statusContent.status -ne 'ok' -or
        $statusContent.schema_id -ne 'star.mcp.star_tool_registry_status.result') {
        throw 'Inspector registry status call did not return a successful structured result'
    }
    $release = @($statusContent.data.items | Where-Object { $_.package_id -eq 'star.control.core' })
    if ($release.Count -ne 1 -or
        $release[0].source -ne 'release' -or
        $release[0].trust_state -ne 'trusted' -or
        $release[0].trust_basis -ne 'release_catalog') {
        throw 'Inspector registry status did not expose the trusted core release package'
    }
    $controllerAfterStatus = Get-IsolatedController
    if ([int]$statusContent.data.controller.pid -ne $controllerAfterStatus.Id) {
        throw 'Controller PID in structured status does not match the isolated process'
    }

    $search = Invoke-Inspector 'search' @(
        '--method', 'tools/call',
        '--tool-name', 'star_tool_search',
        '--tool-arg', 'query=goal'
    ) $nodePath $cliPath $cliWorkingDirectory $gatewayPath
    $searchContent = $search.Data.structuredContent
    $searchItems = @($searchContent.data.items)
    if ($search.Data.isError -ne $false -or
        $searchContent.status -ne 'ok' -or
        $searchContent.schema_id -ne 'star.mcp.star_tool_search.result' -or
        $searchItems.Count -lt 1 -or
        $searchItems.tool_id -notcontains 'star.core.goal.start' -or
        @($searchItems | Where-Object { $_.source -ne 'release' -or $_.readiness -ne 'ready' }).Count -ne 0) {
        throw 'Inspector search call did not return ready release actions'
    }
    $controllerAfterSearch = Get-IsolatedController
    if ($controllerAfterSearch.Id -ne $controllerAfterStatus.Id) {
        throw 'Controller PID changed between Inspector tool calls'
    }

    $coreSearch = Invoke-Inspector 'core-search' @(
        '--method', 'tools/call',
        '--tool-name', 'star_tool_search',
        '--tool-arg', 'query=star.core',
        '--tool-arg', 'sources=["release"]',
        '--tool-arg', 'readiness=["ready"]',
        '--tool-arg', 'limit=50'
    ) $nodePath $cliPath $cliWorkingDirectory $gatewayPath
    $coreSearchContent = $coreSearch.Data.structuredContent
    $coreSearchItems = @($coreSearchContent.data.items)
    $coreSearchIds = @($coreSearchItems.tool_id | Sort-Object)
    $expectedCoreIds = @($requiredCoreToolIds | Sort-Object)
    if ($coreSearch.Data.isError -ne $false -or
        $coreSearchContent.status -ne 'ok' -or
        $coreSearchContent.schema_id -ne 'star.mcp.star_tool_search.result' -or
        $coreSearchItems.Count -ne $requiredCoreToolIds.Count -or
        @(Compare-Object $coreSearchIds $expectedCoreIds).Count -ne 0 -or
        @($coreSearchItems | Where-Object { $_.source -ne 'release' -or $_.readiness -ne 'ready' }).Count -ne 0) {
        throw 'Inspector core search did not return the exact 17 ready release actions'
    }

    foreach ($toolId in $requiredCoreToolIds) {
        $role = 'describe-' + $toolId.Replace('.', '-')
        $description = Invoke-Inspector $role @(
            '--method', 'tools/call',
            '--tool-name', 'star_tool_describe',
            '--tool-arg', "tool_id=$toolId"
        ) $nodePath $cliPath $cliWorkingDirectory $gatewayPath
        $content = $description.Data.structuredContent
        if ($description.Data.isError -ne $false -or
            $content.status -ne 'ok' -or
            $content.schema_id -ne 'star.mcp.star_tool_describe.result' -or
            $content.data.tool_id -ne $toolId -or
            $content.data.source -ne 'release' -or
            $content.data.readiness -ne 'ready' -or
            [string]::IsNullOrWhiteSpace([string]$content.data.descriptor_hash) -or
            [string]::IsNullOrWhiteSpace([string]$content.data.required_call_tool) -or
            $null -eq $content.data.input_schema -or
            $null -eq $content.data.output_schema) {
            throw "Inspector describe did not return a complete ready descriptor: $toolId"
        }
        $coreDescriptions += [pscustomobject]@{
            ToolId = $toolId
            DescriptorHash = [string]$content.data.descriptor_hash
            RequiredCallTool = [string]$content.data.required_call_tool
            RiskLane = [string]$content.data.risk_lane
            Raw = [ordered]@{
                role = $role
                duration_ms = $description.DurationMs
                stdout = $description.Stdout
                stderr = $description.Stderr
            }
        }
    }
    $controllerAfterCoreAudit = Get-IsolatedController
    if ($controllerAfterCoreAudit.Id -ne $controllerAfterStatus.Id) {
        throw 'Controller PID changed during required core describe audit'
    }

    $architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
    if ($architecture -eq 'x64') { $architecture = 'x86_64' }
    $evidence = [ordered]@{
        schema_id = 'star.mcp-inspector-evidence'
        schema_version = 1
        observed_at = [DateTime]::UtcNow.ToString('o')
        host = [ordered]@{
            os = [Environment]::OSVersion.VersionString
            os_build = [Environment]::OSVersion.Version.Build
            architecture = $architecture
            node_version = (& $nodePath --version).Trim()
            node_sha256 = Get-Sha256 $nodePath
            npm_version = (& npm --version).Trim()
        }
        inspector = [ordered]@{
            package = '@modelcontextprotocol/inspector'
            version = [string]$inspectorPackage.version
            integrity = [string]$lockedInspector['integrity']
            package_json_sha256 = Get-Sha256 $inspectorPackagePath
            cli_package = '@modelcontextprotocol/inspector-cli'
            cli_version = [string]$inspectorCliPackage.version
            cli_integrity = [string]$lockedInspectorCli['integrity']
            cli_entrypoint_sha256 = Get-Sha256 $cliPath
            sdk_version = [string]$sdkPackage.version
            sdk_integrity = [string]$lockedSdk['integrity']
            package_lock_sha256 = Get-Sha256 $lockPath
            mode = 'official_cli_stdio'
            cwd_workaround = 'inspector-0.22.0-relative-package-json-resolution'
        }
        binaries = [ordered]@{
            release_binary_root = $releaseBinaries
            gateway_sha256 = Get-Sha256 $gatewayPath
            controller_sha256 = Get-Sha256 $controllerPath
        }
        results = [ordered]@{
            tools_list = $true
            fixed_tool_count = $listedTools.Count
            fixed_tool_names = @($listedTools.name)
            fixed_titles_descriptions_annotations = $true
            fully_resolved_input_output_schemas = $true
            tools_list_exit_code = $toolsList.ExitCode
            registry_status = $true
            registry_status_exit_code = $registryStatus.ExitCode
            controller_pid = $controllerAfterStatus.Id
            controller_instance_id = [string]$statusContent.data.controller.instance_id
            registry_revision = [int]$statusContent.data.registry_revision
            release_package_id = [string]$release[0].package_id
            release_trust_basis = [string]$release[0].trust_basis
            search = $true
            search_exit_code = $search.ExitCode
            search_query = 'goal'
            search_result_count = $searchItems.Count
            search_contains_goal_start = $true
            required_core_search = $true
            required_core_count = $coreSearchItems.Count
            required_core_tool_ids = @($requiredCoreToolIds)
            required_core_describe_count = $coreDescriptions.Count
            required_core_descriptors = @(
                $coreDescriptions | ForEach-Object {
                    [ordered]@{
                        tool_id = $_.ToolId
                        descriptor_hash = $_.DescriptorHash
                        required_call_tool = $_.RequiredCallTool
                        risk_lane = $_.RiskLane
                    }
                }
            )
            controller_pid_unchanged_between_calls = $true
        }
        raw_evidence = @(
            [ordered]@{ role = 'tools_list'; duration_ms = $toolsList.DurationMs; stdout = $toolsList.Stdout; stderr = $toolsList.Stderr },
            [ordered]@{ role = 'registry_status'; duration_ms = $registryStatus.DurationMs; stdout = $registryStatus.Stdout; stderr = $registryStatus.Stderr },
            [ordered]@{ role = 'search'; duration_ms = $search.DurationMs; stdout = $search.Stdout; stderr = $search.Stderr },
            [ordered]@{ role = 'core_search'; duration_ms = $coreSearch.DurationMs; stdout = $coreSearch.Stdout; stderr = $coreSearch.Stderr }
        ) + @(
            $coreDescriptions | ForEach-Object { $_.Raw }
        )
    }
    $evidence | ConvertTo-Json -Depth 20
} finally {
    Stop-IsolatedController
}
