# Gate E2E-2 on Windows (#108, docs/plan-windows-linux.md §7, phase 4).
#
# The Linux gate is linux/ci/e2e2.sh; this is its Windows half, and it covers
# what a Windows host must prove that a compile lane cannot: the real `claude`
# CLI, with the hook installed by the product's own installer, asks the panel
# for permission and obeys the answer.
#
# **Written but not yet exercised on a Windows host.** It was written against
# the same flow the Linux script automates and the same `morrow-app --hooks`
# commands; treat the first run on a host as the acceptance test for this file.
# What it does automate is the installer round-trip (install → status →
# settings.json carries the hook → uninstall → byte for byte), which is the half
# that can go wrong quietly. The panel half is manual and is NOT covered by the
# PASS line this script prints.
#
# Usage, in a PowerShell on the Windows host, from the repo root:
#   windows\ci\e2e2.ps1
# Env: MORROW_APP (default: target\release\morrow-app.exe), CLAUDE_BIN.
# Exit: 0 when every automated installer check passed. This script does not
#       decide whether the E2E-2 gate passed.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
Set-Location $root

$app = if ($env:MORROW_APP) { $env:MORROW_APP } else { Join-Path $root 'target\release\morrow-app.exe' }
$claude = if ($env:CLAUDE_BIN) { $env:CLAUDE_BIN } else { 'claude' }
$results = @()
$failed = 0

function Check([string]$name, [scriptblock]$test) {
    if (& $test) { $script:results += "PASS  $name" }
    else { $script:results += "FAIL  $name"; $script:failed = 1 }
}

if (-not (Test-Path $app)) { throw "no $app : build it first (cargo build --release -p morrow-app)" }

# A scratch USERPROFILE: the installer writes to %USERPROFILE%\.claude and
# %USERPROFILE%\.codeisland, and the developer's real config is not a fixture.
$work = Join-Path $env:TEMP ("e2e2-" + [guid]::NewGuid().ToString('N'))
$home = Join-Path $work 'home'
try {
    New-Item -ItemType Directory -Force -Path (Join-Path $home '.claude') | Out-Null
    $env:USERPROFILE = $home
    $env:HOME = $home
    $env:CLAUDE_CONFIG_DIR = Join-Path $home '.claude'
    $settings = Join-Path $env:CLAUDE_CONFIG_DIR 'settings.json'

    $before = if (Test-Path $settings) { (Get-FileHash $settings -Algorithm SHA256).Hash } else { 'absent' }

    $installOut = & $app --hooks install claude 2>&1 | Out-String
    Check 'install_claude' { $installOut -match '^claude: changed=' }
    Check 'settings_written' { (Test-Path $settings) -and ((Get-Content $settings -Raw) -match 'codeisland-hook\.sh') }
    Check 'status_says_installed' { (& $app --hooks status | Out-String) -match '(?m)^claude: installed$' }

    # A config the user already had: install must add our hook without touching
    # the rest, and uninstall must give the file back byte for byte.
    Remove-Item $settings -Force -ErrorAction SilentlyContinue
    '{"theme":"dark","permissions":{"allow":["Bash(ls:*)"]}}' | Set-Content -NoNewline -Path $settings
    $userBefore = (Get-FileHash $settings -Algorithm SHA256).Hash
    & $app --hooks install claude | Out-Null
    Check 'user_keys_kept' { (Get-Content $settings -Raw) -match '"theme":"dark"' }
    & $app --hooks uninstall claude | Out-Null
    Check 'user_settings_byte_for_byte' { (Get-FileHash $settings -Algorithm SHA256).Hash -eq $userBefore }

    Check 'status_says_absent' { (& $app --hooks status | Out-String) -match '(?m)^claude: absent$' }

    Write-Output ''
    Write-Output '=== Evidence: windows/ci/e2e2.ps1 (gate E2E-2, installer half) ==='
    Write-Output "date:      $(Get-Date -Format o)"
    Write-Output "commit:    $(git rev-parse --short HEAD 2>$null)"
    Write-Output "os:        $([System.Environment]::OSVersion.VersionString)"
    Write-Output "app:       $app"
    Write-Output "claude:    $((& $claude --version 2>&1 | Select-Object -First 1))"
    Write-Output "home:      $home (scratch; the developer's config was not touched)"
    Write-Output "install:   $($installOut.Trim())"
    Write-Output "settings:  before=$before after=$(if (Test-Path $settings) { (Get-FileHash $settings -Algorithm SHA256).Hash } else { 'absent' })"
    Write-Output '--- manual, and required before this gate can pass ---'
    Write-Output '1. Start the island:  $env:MORROW_SMOKE=1; & $app'
    Write-Output '2. In another window:  claude   then ask it to run:  echo e2e2-ok > "$env:TEMP\e2e2-manual.marker"'
    Write-Output '3. Click Approve on the card. The marker file must exist with that content.'
    Write-Output '4. Repeat with Deny. The marker file must be absent.'
    Write-Output '5. Paste the console output here as the Evidence for W-002/W-004/W-008.'
    $results | ForEach-Object { Write-Output $_ }
    Write-Output ("installer: " + $(if ($failed -eq 0) { "PASS ($($results.Count) automated installer checks)" } else { 'FAIL' }))
    Write-Output 'gate:      NOT RUN here — the panel half is manual and no Windows host has run it yet'
}
finally {
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
}
exit $failed
