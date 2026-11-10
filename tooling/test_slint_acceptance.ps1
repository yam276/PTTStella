# Self-authored receipts only. Does not launch an App or claim Windows acceptance.
. (Join-Path $PSScriptRoot 'windows/slint_acceptance.ps1')

function Reject([scriptblock]$Action, [string]$Label) {
    $rejected = $false
    try { & $Action } catch { $rejected = $true }
    if (-not $rejected) { throw "Invalid evidence accepted: $Label" }
}

function Save([string]$Name, $Value) {
    $Value | ConvertTo-Json -Depth 15 | Set-Content -LiteralPath (Join-Path $temp $Name) -Encoding UTF8
}

$temp = Join-Path ([IO.Path]::GetTempPath()) ('pttstella-receipt-fixtures-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temp | Out-Null
try {
    $ack = '{"close":"acknowledged","expected_close":"acknowledged","fixture":"slint_native_media","status":"passed"}'
    Require-CloseAck ("ordinary diagnostic`n" + $ack) 'slint_native_media'
    Reject { Require-CloseAck '' 'slint_native_media' } 'empty trace'
    Reject { Require-CloseAck ($ack + "`n" + $ack) 'slint_native_media' } 'duplicate Close'
    Reject { Require-CloseAck $ack 'wrong_fixture' } 'wrong peer'
    Reject { Require-CloseAck ($ack.Replace('"status":"passed"', '"status":"failed"')) 'slint_native_media' } 'failed peer'
    Reject { Require-True 'true' 'bad type' } 'string boolean'
    Reject { Read-Receipt $temp 'absent.json' } 'missing receipt'
    Save 'null.json' $null
    Reject { Read-Receipt $temp 'null.json' } 'JSON null'

    $motion = @{
        status = 'passed_pending_exit_audit'
        initial = @{state = 'known'; source = 'windows_uisettings'; reduced = $false}
        final = @{state = 'known'; source = 'windows_uisettings'; reduced = $false}
        actual_os_change_and_restore = $false; notice_policy_verified = $true; zero_bbs_input = $true
    }
    $motionClosed = @{status = 'passed'; ownership = @{
        source = @{listener_removed = $true; worker_joined = $null}; ui_listener_joined = $true
    }}
    Save 'motion-result.json' $motion; Save 'motion-closed.json' $motionClosed
    Require-CaseReceipts $temp 'motion'
    $motion.initial.source = 'controlled_synthetic_probe'; Save 'motion-result.json' $motion
    Reject { Require-CaseReceipts $temp 'motion' } 'synthetic source instead of Windows'
    $motion.initial.source = 'windows_uisettings'; Save 'motion-result.json' $motion
    $motionClosed.ownership.ui_listener_joined = $false; Save 'motion-closed.json' $motionClosed
    Reject { Require-CaseReceipts $temp 'motion' } 'unjoined UI listener'
    Save 'motion-not-ready.json' @{status = 'not_ready'; reading = @{state = 'unavailable'; stage = 'subscribe'; detail = 'fixture'}}
    Save 'motion-closed.json' @{status = 'passed'; ownership = @{source = $null; ui_listener_joined = $null}}
    Require-MotionUnavailable $temp
    Save 'motion-closed.json' $motionClosed
    Reject { Require-MotionUnavailable $temp } 'not_ready with unjoined listener'
    Save 'motion-not-ready.json' @{status = 'not_ready'; reading = @{state = 'known'}}
    Reject { Require-MotionUnavailable $temp } 'known source falsely marked unavailable'

    $video = @{
        status = 'prepared'; stage = 'playing'; playing = $true; renderer_owned = $true; session_owned = $true
        samples = @(@{passed = $true; frame_serial = 7
            expected = @(@(255,0,0), @(0,0,255), @(255,255,0), @(0,255,255))
            pixels = @(@(254,0,0,255), @(0,0,254,255), @(255,255,0,255), @(1,255,255,255))})
    }
    $videoClosed = @{status = 'passed'; normal_exit = $true; renderer_retired = $true
        session_joined = $true; video_task_empty = $true; image_worker_empty = $true
        offer_empty = $true; pending_empty = $true}
    Save 'video-exit-ready.json' $video; Save 'video-exit-closed.json' $videoClosed
    Require-CaseReceipts $temp 'video'
    $video.samples[0].pixels[0] = @(0,0,0,255); Save 'video-exit-ready.json' $video
    Reject { Require-CaseReceipts $temp 'video' } 'black pixels with dishonest passed flag'
    $video.samples = @(); Save 'video-exit-ready.json' $video
    Reject { Require-CaseReceipts $temp 'video' } 'empty pixel samples'

    $shell = @{status = 'passed'}
    $notice = @{status = 'passed_pending_exit_audit'; hover_on_standard_child = $true
        toolkit_tab_focus = $true; fade_pause = $true; reduced_motion_change = $true
        replacement_deadline = $true; error_priority = $true; persistent_unknown = $true; expiry_preserves_focus = $true}
    $noticeClosed = @{status = 'passed'; stopped = $true; timer_running = $false; model_deadline = $false}
    Save 'shell-result.json' $shell; Save 'notice-result.json' $notice; Save 'notice-closed.json' $noticeClosed
    Require-CaseReceipts $temp 'shell'
    $noticeClosed.timer_running = $true; Save 'notice-closed.json' $noticeClosed
    Reject { Require-CaseReceipts $temp 'shell' } 'live notification timer'

    $actions = @{status = 'passed_pending_exit_audit'; synthetic_default_disabled = $true
        remote_url_ignored = $true; failed_retry_retained_offer = $true; manual_retry_toolkit_space_key = $true
        off_page_and_closed_actions_rejected = $true; pending_http_at_return = $true}
    $releaseClosed = @{status = 'passed'; request_count = 3; requests = @('first','second','third')
        settings_unchanged = $true; stalled_http_closed = $true; status_task_retired = $true; worker_joined = $true}
    Save 'release-actions.json' $actions; Save 'release-closed.json' $releaseClosed
    Require-CaseReceipts $temp 'release'
    $releaseClosed.worker_joined = $false; Save 'release-closed.json' $releaseClosed
    Reject { Require-CaseReceipts $temp 'release' } 'unjoined release worker'

    $destinations = @(
        foreach ($language in @('en-US', 'zh-TW')) {
            foreach ($field in 1..24) { @{language = $language; focused_field = $field} }
        }
    )
    $search = @{status = 'passed'; cases = $destinations; incomplete_draft_preserved = $true
        private_name_not_indexed = $true; stale_focus_cancelled = $true; saved_settings_restored = $true}
    Save 'settings-search.json' $search; Save 'sites-closed.json' @{status = 'passed'; stopped = $true}
    Require-CaseReceipts $temp 'search'
    $search.cases = @($destinations[0]) * 48; Save 'settings-search.json' $search
    Reject { Require-CaseReceipts $temp 'search' } '48 duplicated destinations'

    Save 'capture-result.json' @{case = 'current'; saved = $true; strict_station_peer = $true}
    $captureClosed = @{capture_retired = $true; timer_stopped = $true; file_worker_joined = $true
        result_collected = $true; window_released = $true}
    Save 'capture-closed.json' $captureClosed
    Require-CaseReceipts $temp 'capture'
    $captureClosed.file_worker_joined = $false; Save 'capture-closed.json' $captureClosed
    Reject { Require-CaseReceipts $temp 'capture' } 'unjoined capture file worker'

    # Parser compatibility is checked as well as the executed validator functions.
    $tokens = $null; $errors = $null
    [Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'windows/slint_acceptance.ps1'),
        [ref]$tokens, [ref]$errors) | Out-Null
    if ($errors.Count) { throw ($errors | Out-String) }
    Write-Host 'Receipt helper fixtures passed; no product or native Windows execution'
} finally { Remove-Item -LiteralPath $temp -Recurse -Force }
