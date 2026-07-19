# Build the cockpit-runner and stage it as a Tauri sidecar binary.
#
# Tauri resolves `externalBin` entries by appending the host target triple, so
# the runner is copied to `binaries/cockpit-runner-<triple><ext>`. Run this
# before `npm run tauri:build` (or `tauri:dev`) to package the runner alongside
# the desktop app.

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$WorkspaceRoot = Resolve-Path "$ScriptDir\..\..\.."
$BinDir = "$ScriptDir\binaries"

# Get the host target triple
$TripleLine = rustc -vV | Select-String "host:"
if (-not $TripleLine) {
    Write-Error "could not determine host target triple"
    exit 1
}
$Triple = $TripleLine.ToString() -replace "host:\s*", ""

$Ext = if ($Triple -match "windows") { ".exe" } else { "" }

Write-Host "Building cockpit-runner and cockpit-evaluator (release) for $Triple"
Push-Location $WorkspaceRoot
try {
    cargo build --release -p cockpit-runner -p cockpit-evaluator --features cockpit-runner/live-acp
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
foreach ($Name in @("cockpit-runner", "cockpit-evaluator")) {
    $Src = "$WorkspaceRoot\target\release\$Name$Ext"
    $Dst = "$BinDir\$Name-$Triple$Ext"
    Copy-Item $Src $Dst -Force
    Write-Host "Staged sidecar: $Dst"
}
