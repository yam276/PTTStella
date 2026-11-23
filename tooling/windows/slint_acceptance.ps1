# Windows PowerShell 5.1-compatible. Dot-source only to test receipt validation.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Require-Check([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Require-True($Value, [string]$Message) {
    Require-Check (($Value -is [bool]) -and $Value) $Message
}

function Read-Receipt([string]$Directory, [string]$Name) {
    $path = Join-Path $Directory $Name
    $file = Get-Item -LiteralPath $path
    Require-Check (($file.Length -gt 0) -and ($file.Length -le 4MB)) "Invalid receipt size: $Name"
    $value = Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
    Require-Check ($null -ne $value) "Empty receipt: $Name"
    return $value
}

function Require-CloseAck([string]$Text, [string]$Fixture) {
    $acks = @(
        foreach ($line in [regex]::Split($Text, '\r?\n')) {
            if (-not $line.TrimStart().StartsWith('{')) { continue }
            try { $entry = $line | ConvertFrom-Json } catch { continue }
            if ($null -ne $entry -and $null -ne $entry.PSObject.Properties['close']) { $entry }
        }
    )
    Require-Check ($acks.Count -eq 1) 'Expected exactly one strict peer Close result'
    $ack = $acks[0]
    Require-Check (($ack.close -ceq 'acknowledged') -and
        ($ack.expected_close -ceq 'acknowledged') -and
        ($ack.status -ceq 'passed') -and ($ack.fixture -ceq $Fixture)) 'Strict peer Close failed'
}

function Require-CaseReceipts([string]$Directory, [string]$Kind) {
    switch ($Kind) {
        'shell' {
            $shell = Read-Receipt $Directory 'shell-result.json'
            Require-Check ($shell.status -ceq 'passed') 'Shell did not pass'
            $notice = Read-Receipt $Directory 'notice-result.json'
            Require-Check ($notice.status -ceq 'passed_pending_exit_audit') 'Notice actions incomplete'
            foreach ($key in @('hover_on_standard_child', 'toolkit_tab_focus', 'fade_pause',
                'reduced_motion_change', 'replacement_deadline', 'error_priority',
                'persistent_unknown', 'expiry_preserves_focus')) {
                Require-True $notice.$key "Notice check failed: $key"
            }
            $closed = Read-Receipt $Directory 'notice-closed.json'
            Require-Check ($closed.status -ceq 'passed') 'Notice close audit failed'
            Require-True $closed.stopped 'Notice owner still active'
            Require-Check (($closed.timer_running -is [bool]) -and -not $closed.timer_running -and
                ($closed.model_deadline -is [bool]) -and -not $closed.model_deadline) 'Notice timer/deadline retained'
        }
        'motion' {
            $result = Read-Receipt $Directory 'motion-result.json'
            Require-Check ($result.status -ceq 'passed_pending_exit_audit') 'Motion actions incomplete'
            foreach ($reading in @($result.initial, $result.final)) {
                Require-Check (($reading.state -ceq 'known') -and
                    ($reading.source -ceq 'windows_uisettings') -and
                    ($reading.reduced -is [bool])) 'Actual Windows motion source required'
            }
            Require-Check ($result.initial.reduced -eq $result.final.reduced) 'OS motion changed during read-only case'
            Require-Check (($result.actual_os_change_and_restore -is [bool]) -and
                -not $result.actual_os_change_and_restore) 'Unexpected OS change scope'
            Require-True $result.notice_policy_verified 'Motion notification policy unverified'
            Require-True $result.zero_bbs_input 'Motion case sent BBS input'
            $closed = Read-Receipt $Directory 'motion-closed.json'
            Require-Check ($closed.status -ceq 'passed') 'Motion close audit failed'
            Require-True $closed.ownership.source.listener_removed 'Windows motion listener retained'
            Require-True $closed.ownership.ui_listener_joined 'Motion UI listener join unproven'
            Require-Check ($null -eq $closed.ownership.source.worker_joined) 'Windows callback audit claims a worker'
        }
        'release' {
            $actions = Read-Receipt $Directory 'release-actions.json'
            Require-Check ($actions.status -ceq 'passed_pending_exit_audit') 'Release actions incomplete'
            foreach ($key in @('synthetic_default_disabled', 'remote_url_ignored',
                'failed_retry_retained_offer', 'manual_retry_toolkit_space_key',
                'off_page_and_closed_actions_rejected', 'pending_http_at_return')) {
                Require-True $actions.$key "Release check failed: $key"
            }
            $closed = Read-Receipt $Directory 'release-closed.json'
            Require-Check (($closed.status -ceq 'passed') -and ($closed.request_count -eq 3) -and
                (@($closed.requests).Count -eq 3)) 'Release HTTP audit incomplete'
            foreach ($key in @('settings_unchanged', 'stalled_http_closed', 'status_task_retired', 'worker_joined')) {
                Require-True $closed.$key "Release close failed: $key"
            }
        }
        'search' {
            $result = Read-Receipt $Directory 'settings-search.json'
            Require-Check (($result.status -ceq 'passed') -and (@($result.cases).Count -eq 48)) 'Settings search matrix incomplete'
            $keys = @($result.cases | ForEach-Object { "$($_.language)/$($_.focused_field)" } | Select-Object -Unique)
            Require-Check ($keys.Count -eq 48) 'Settings search destinations duplicated'
            foreach ($key in @('incomplete_draft_preserved', 'private_name_not_indexed',
                'stale_focus_cancelled', 'saved_settings_restored')) {
                Require-True $result.$key "Settings search failed: $key"
            }
            $closed = Read-Receipt $Directory 'sites-closed.json'
            Require-Check ($closed.status -ceq 'passed') 'Settings manager close failed'
            Require-True $closed.stopped 'Settings manager not stopped'
        }
        'capture' {
            $result = Read-Receipt $Directory 'capture-result.json'
            Require-Check ($result.case -ceq 'current') 'Wrong capture case'
            Require-True $result.saved 'Capture not saved'
            Require-True $result.strict_station_peer 'Capture peer unproven'
            $closed = Read-Receipt $Directory 'capture-closed.json'
            foreach ($key in @('capture_retired', 'timer_stopped', 'file_worker_joined', 'result_collected', 'window_released')) {
                Require-True $closed.$key "Capture close failed: $key"
            }
        }
        'video' {
            $ready = Read-Receipt $Directory 'video-exit-ready.json'
            Require-Check (($ready.status -ceq 'prepared') -and ($ready.stage -ceq 'playing')) 'Video never reached playback'
            foreach ($key in @('playing', 'renderer_owned', 'session_owned')) {
                Require-True $ready.$key "Video preparation failed: $key"
            }
            $samples = @($ready.samples)
            Require-Check ($samples.Count -gt 0) 'Video pixel samples empty'
            Require-Check ($ready.render_path -cin @('software', 'opengl')) 'Unknown video render path'
            foreach ($sample in $samples) {
                Require-True $sample.passed 'Video pixel sample failed'
                Require-Check ((@($sample.pixels).Count -eq 4) -and
                    (@($sample.expected).Count -eq 4)) 'Video sample incomplete'
                if ($ready.render_path -ceq 'software') {
                    Require-Check (($sample.render_path -ceq 'software') -and
                        ($sample.frame_serial -is [long] -or $sample.frame_serial -is [int]) -and
                        ($sample.frame_serial -gt 0)) 'Software frame identity missing'
                    $dimensions = @($sample.size)
                    Require-Check ($dimensions.Count -eq 2) 'Software frame size missing'
                } else {
                    # The OpenGL producer samples the rendered framebuffer, not a
                    # software frame. Its identity is the sampled x/y/width/height.
                    $rect = @($sample.rect)
                    Require-Check ($rect.Count -eq 4) 'OpenGL sample rectangle missing'
                    foreach ($origin in @($rect[0], $rect[1])) {
                        Require-Check (($origin -is [int] -or $origin -is [long]) -and
                            ($origin -ge 0)) 'Invalid OpenGL sample origin'
                    }
                    $dimensions = @($rect[2], $rect[3])
                }
                foreach ($dimension in $dimensions) {
                    Require-Check (($dimension -is [int] -or $dimension -is [long]) -and
                        ($dimension -gt 0)) 'Invalid video sample dimensions'
                }
                $colors = @(@(255, 0, 0), @(0, 0, 255), @(255, 255, 0), @(0, 255, 255))
                for ($i = 0; $i -lt 4; $i++) {
                    Require-Check ((@($sample.pixels[$i]).Count -eq 4) -and
                        (@($sample.expected[$i]).Count -eq 3)) 'Invalid pixel channels'
                    for ($channel = 0; $channel -lt 3; $channel++) {
                        $pixel = $sample.pixels[$i][$channel]
                        Require-Check (($pixel -is [ValueType]) -and ($pixel -isnot [bool]) -and
                            ($pixel -ge 0) -and ($pixel -le 255) -and
                            ([Math]::Abs($pixel - $colors[$i][$channel]) -le 30) -and
                            ($sample.expected[$i][$channel] -eq $colors[$i][$channel])) 'Quadrant pixels do not match fixture'
                    }
                }
            }
            $closed = Read-Receipt $Directory 'video-exit-closed.json'
            Require-Check ($closed.status -ceq 'passed') 'Video exit audit failed'
            foreach ($key in @('normal_exit', 'renderer_retired', 'session_joined', 'video_task_empty',
                'image_worker_empty', 'offer_empty', 'pending_empty')) {
                Require-True $closed.$key "Video exit failed: $key"
            }
        }
        default { throw "Unknown case kind: $Kind" }
    }
}

function Require-MotionUnavailable([string]$Directory) {
    $unavailable = Read-Receipt $Directory 'motion-not-ready.json'
    Require-Check (($unavailable.status -ceq 'not_ready') -and
        ($unavailable.reading.state -ceq 'unavailable')) 'Invalid motion availability result'
    $closed = Read-Receipt $Directory 'motion-closed.json'
    Require-Check ($closed.status -ceq 'passed') 'Unavailable motion source did not close'
    if ($null -eq $closed.ownership.source) {
        Require-Check ($null -eq $closed.ownership.ui_listener_joined) 'Listener exists without motion source'
    } else {
        Require-True $closed.ownership.source.listener_removed 'Unavailable motion listener retained'
        Require-True $closed.ownership.ui_listener_joined 'Unavailable motion UI join unproven'
        Require-Check ($null -eq $closed.ownership.source.worker_joined) 'Windows unavailable audit claims a worker'
    }
}

function Invoke-SlintNativeChecks([string]$Root) {
    Require-Check (([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) -and
        [Environment]::Is64BitOperatingSystem) 'The native runner requires Windows x64'
    $rootPath = (Get-Item -LiteralPath $Root).FullName
    $exe = Join-Path $rootPath 'pttstella-slint.exe'
    $candidate = Read-Receipt $rootPath 'candidate.json'
    foreach ($name in @('pttstella-slint.exe', 'libmpv-2.dll', 'fixtures/quadrants.mp4',
        'fixtures/quadrants.webm', 'fixtures/quadrants.ogv')) {
        $hash = (Get-FileHash -LiteralPath (Join-Path $rootPath $name) -Algorithm SHA256).Hash.ToLowerInvariant()
        Require-Check ($hash -ceq $candidate.payload_sha256.$name) "Candidate payload hash mismatch: $name"
    }
    $output = Join-Path $rootPath ('evidence/native-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss') + '-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $output | Out-Null
    $cases = @(
        @{ name = 'shell-software'; kind = 'shell'; software = $true },
        @{ name = 'shell-default'; kind = 'shell'; software = $false },
        @{ name = 'motion-software'; kind = 'motion'; software = $true },
        @{ name = 'motion-default'; kind = 'motion'; software = $false },
        @{ name = 'release'; kind = 'release'; software = $true },
        @{ name = 'settings-search'; kind = 'search'; software = $true },
        @{ name = 'capture'; kind = 'capture'; software = $true }
    )
    foreach ($format in @('mp4', 'webm', 'ogv')) {
        $cases += @{ name = "video-$format-software"; kind = 'video'; software = $true; format = $format }
        $cases += @{ name = "video-$format-default"; kind = 'video'; software = $false; format = $format }
    }
    $summary = [ordered]@{
        status = 'running'; source_ref = $candidate.build.source_ref
        executable_sha256 = $candidate.payload_sha256.'pttstella-slint.exe'
        os_version = [Environment]::OSVersion.VersionString; powershell = $PSVersionTable.PSVersion.ToString()
        os_64bit = [Environment]::Is64BitOperatingSystem; process_64bit = [Environment]::Is64BitProcess
        scope = 'Windows native process/toolkit components and strict loopback only; manual IME/DPI/audio/OS input/hardware GPU, OS motion change, WebView2-free host, clean installation, rollback and performance pending'
        cases = @()
    }
    $resultFile = Join-Path $output 'native-results.json'
    foreach ($case in $cases) {
        $directory = Join-Path $output $case.name
        $arguments = switch ($case.kind) {
            'shell' { '--verify-shell' }
            'motion' { '--verify-motion' }
            default { '--probe-media --verify-media' }
        }
        if ($case.software) { $arguments = '--software-renderer ' + $arguments }
        # Double quotes cannot appear in Windows file names. One quoted path is
        # compatible with .NET Framework ProcessStartInfo (no ArgumentList API).
        $arguments += ' "' + $directory + '"'
        $info = New-Object System.Diagnostics.ProcessStartInfo
        $info.FileName = $exe; $info.Arguments = $arguments; $info.WorkingDirectory = $rootPath
        $info.UseShellExecute = $false; $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
        foreach ($key in @($info.EnvironmentVariables.Keys)) {
            if ($key -match '^(PTTSTELLA_|SLINT_|DYLD_|LD_)') { $info.EnvironmentVariables.Remove($key) }
        }
        switch ($case.kind) {
            'release' { $info.EnvironmentVariables['PTTSTELLA_SLINT_SETTINGS_PROBE'] = 'releases' }
            'search' { $info.EnvironmentVariables['PTTSTELLA_SLINT_SETTINGS_PROBE'] = 'search' }
            'capture' { $info.EnvironmentVariables['PTTSTELLA_SLINT_CAPTURE_PROBE'] = 'current' }
            'video' {
                $info.EnvironmentVariables['PTTSTELLA_VIDEO_FIXTURE'] = Join-Path $rootPath ("fixtures/quadrants." + $case.format)
                $info.EnvironmentVariables['PTTSTELLA_VIDEO_PROBE_EXIT'] = if ($case.software) { 'quit' } else { 'close' }
            }
        }
        $process = New-Object System.Diagnostics.Process
        $process.StartInfo = $info
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $row = [ordered]@{ name = $case.name; kind = $case.kind; arguments = $arguments; pid = $null
            status = 'failed'; exit = $null; timed_out = $false; process_still_running = $false; seconds = 0; error = $null }
        try {
            Require-True ($process.Start()) 'Process did not start'
            $row.pid = $process.Id
            $stdout = $process.StandardOutput.ReadToEndAsync()
            $stderr = $process.StandardError.ReadToEndAsync()
            if (-not $process.WaitForExit(90000)) {
                $row.timed_out = $true; $process.Kill()
                Require-Check ($process.WaitForExit(5000)) 'Timed-out process termination unproven; PID retained'
            }
            $row.exit = $process.ExitCode
            $reads = [Threading.Tasks.Task]::WhenAll([Threading.Tasks.Task[]]@($stdout, $stderr))
            Require-Check ($reads.Wait(5000)) 'Output pipes did not close after process exit'
            $text = $stdout.GetAwaiter().GetResult() + "`n" + $stderr.GetAwaiter().GetResult()
            [IO.File]::WriteAllText((Join-Path $output ($case.name + '.log')), $text, [Text.Encoding]::UTF8)
            Require-Check (-not $row.timed_out) 'Process timed out; normal cleanup unproven'
            $fixture = if ($case.kind -in @('shell', 'motion')) { 'slint_full_parity_grid' } else { 'slint_native_media' }
            Require-CloseAck $text $fixture
            if (($case.kind -eq 'motion') -and ($row.exit -ne 0) -and
                (Test-Path -LiteralPath (Join-Path $directory 'motion-not-ready.json'))) {
                Require-MotionUnavailable $directory
                $row.status = 'not_ready'; $row.error = 'Actual OS preference unavailable; see motion-not-ready.json'
            } else {
                Require-Check ($row.exit -eq 0) "Process exit: $($row.exit)"
                Require-CaseReceipts $directory $case.kind
                $row.status = 'passed'
            }
        } catch { $row.error = $_.Exception.Message }
        finally {
            if ($null -ne $row.pid) {
                try { $row.process_still_running = -not $process.HasExited }
                catch { $row.process_still_running = $true }
            }
            $process.Dispose(); $watch.Stop(); $row.seconds = $watch.Elapsed.TotalSeconds
        }
        $summary.cases += $row
        $summary | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $resultFile -Encoding UTF8
        Write-Host ($case.name + ': ' + $row.status + $(if ($row.error) { ' - ' + $row.error } else { '' }))
        if ($row.process_still_running) { break } # Keep PID; do not accumulate more unjoined processes.
    }
    $failed = @($summary.cases | Where-Object { $_.status -eq 'failed' })
    $unavailable = @($summary.cases | Where-Object { $_.status -eq 'not_ready' })
    $summary.status = if ($failed.Count) { 'failed' } elseif ($unavailable.Count) { 'not_ready' } else { 'component_cases_passed_manual_pending' }
    $summary | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $resultFile -Encoding UTF8
    Write-Host "Result: $resultFile"
    if ($failed.Count) { return 1 }
    if ($unavailable.Count) { return 2 }
    return 0
}

if ($MyInvocation.InvocationName -ne '.') {
    exit (Invoke-SlintNativeChecks $PSScriptRoot)
}
