# Peak RSS of the built Morrow app, measured on the host that runs it (#64).
#
# Windows half of scripts/ci/bench-rss.sh. It starts an already-built binary,
# samples its resident set (WorkingSet64) at a fixed interval for a fixed
# window, then stops it. A cross-build cannot produce RSS for Windows, so run
# this on the Windows desktop being measured.
#
# Written against the same flow as the Unix script, but NOT yet exercised on a
# Windows host: the first run on one is the acceptance test for this file.
#
# Host prerequisites
#   Windows 10+, PowerShell 5.1+, WebView2 runtime, and a build:
#     cd shared\desktop
#     bun install
#     bun run build
#     cargo build --release -p morrow-app
#   No admin rights and no changes to the user's config.
#
# Usage, in PowerShell from the repo root:
#   windows\ci\bench-rss.ps1
#   $env:MORROW_APP = 'C:\path\to\morrow-app.exe'; windows\ci\bench-rss.ps1
#   $env:BENCH_SECONDS = 20; windows\ci\bench-rss.ps1
#
# Env: MORROW_APP (binary, auto-detected when unset), BENCH_SECONDS (default
#      10), BENCH_INTERVAL (default 0.2).
# Exit: 0 when samples were collected on this host, non-zero otherwise.
# Scope: the app's main process only. WebView2 children are not summed; see
#        the README section "Measure peak RAM usage" before comparing OSes.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
Set-Location $root

$window = if ($env:BENCH_SECONDS) { [double]$env:BENCH_SECONDS } else { 10 }
$intervalSec = if ($env:BENCH_INTERVAL) { [double]$env:BENCH_INTERVAL } else { 0.2 }
if ($window -le 0 -or $intervalSec -le 0 -or $intervalSec -ge $window) {
    throw 'BENCH_SECONDS and BENCH_INTERVAL must be positive and interval < window'
}

$app = $env:MORROW_APP
if (-not $app) {
    foreach ($candidate in @('target\release\morrow-app.exe', 'target\debug\morrow-app.exe')) {
        $path = Join-Path $root $candidate
        if (Test-Path $path) { $app = $path; break }
    }
}
if (-not $app -or -not (Test-Path $app)) {
    throw 'no built app found; build it first or set MORROW_APP (see file header)'
}

$work = Join-Path $env:TEMP ('bench-rss-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$outLog = Join-Path $work 'app.out'
$errLog = Join-Path $work 'app.err'
$proc = $null
$peak = [int64]0
$samples = 0

try {
    $proc = Start-Process -FilePath $app -RedirectStandardOutput $outLog -RedirectStandardError $errLog -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds($window)
    while ([DateTime]::UtcNow -lt $deadline) {
        $proc.Refresh()
        if ($proc.HasExited) {
            throw "app exited before the window ended; see $errLog"
        }
        $rss = [int64]$proc.WorkingSet64
        if ($rss -gt $peak) { $peak = $rss }
        $samples++
        Start-Sleep -Milliseconds ([int]($intervalSec * 1000))
    }
    if ($samples -eq 0) { throw "no samples collected; see $errLog" }

    $peakKb = [math]::Round($peak / 1KB)
    $peakMib = [math]::Round($peak / 1MB, 1)
    $commit = (& git rev-parse --short HEAD 2>$null) -join ''
    if (-not $commit) { $commit = 'unknown' }

    Write-Output ''
    Write-Output '=== Evidence: windows/ci/bench-rss.ps1 (#64) ==='
    Write-Output "date:      $(Get-Date -Format o)"
    Write-Output "commit:    $commit"
    Write-Output "os:        $([System.Environment]::OSVersion.VersionString)"
    Write-Output "app:       $app"
    Write-Output "window:    ${window}s, interval ${intervalSec}s, $samples samples"
    Write-Output 'scope:     main process RSS only (child/webview processes excluded)'
    Write-Output "peak_rss:  $peakKb kB ($peakMib MiB)"
    Write-Output "app_out:   $outLog"
    Write-Output "app_err:   $errLog"
    Write-Output 'verdict:   MEASURED on Windows; not evidence for any other host'
}
finally {
    if ($proc) {
        $proc.Refresh()
        if (-not $proc.HasExited) {
            try { $null = $proc.CloseMainWindow() } catch { }
            if (-not $proc.WaitForExit(3000)) { $proc.Kill() }
        }
    }
}
