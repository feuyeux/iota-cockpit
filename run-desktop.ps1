[CmdletBinding()]
param(
    [switch]$Clean
)

$ErrorActionPreference = 'Stop'

$devPort = 15342
$portReleaseAttempts = 20

function Require-Command {
    param([Parameter(Mandatory = $true)][string]$Name)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command not found: $Name"
    }
}

function Get-ListeningProcessIds {
    param([Parameter(Mandatory = $true)][int]$Port)

    @(Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty OwningProcess -Unique)
}

function Release-DevPort {
    $processIds = Get-ListeningProcessIds -Port $devPort
    if ($processIds.Count -eq 0) {
        return
    }

    Write-Host "Stopping existing development server on port $devPort"
    foreach ($processId in $processIds) {
        Stop-Process -Id $processId -ErrorAction SilentlyContinue
    }

    for ($attempt = 1; $attempt -le $portReleaseAttempts; $attempt++) {
        if ((Get-ListeningProcessIds -Port $devPort).Count -eq 0) {
            return
        }
        Start-Sleep -Milliseconds 100
    }

    throw "Port $devPort is still in use; stop its listener and try again."
}

function Ensure-FrontendDependencies {
    if (Test-Path 'node_modules/.bin/vite.cmd' -PathType Leaf) {
        return
    }

    Write-Host 'Frontend dependencies not found, running npm install'
    & npm install
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    if (-not (Test-Path 'node_modules/.bin/vite.cmd' -PathType Leaf)) {
        throw 'npm install completed but vite is still missing from node_modules/.bin'
    }
}

function Configure-HermesBackend {
    $hermesPath = $null
    if ($env:COCKPIT_HERMES_BIN) {
        if (-not (Test-Path $env:COCKPIT_HERMES_BIN -PathType Leaf)) {
            throw "COCKPIT_HERMES_BIN does not point to a file: $env:COCKPIT_HERMES_BIN"
        }
        if ((Split-Path $env:COCKPIT_HERMES_BIN -Leaf) -ine 'hermes-agent.exe') {
            $hermesPath = $env:COCKPIT_HERMES_BIN
        } else {
            Write-Host "Ignoring Hermes Desktop Agent at $env:COCKPIT_HERMES_BIN because it is not a stdio ACP server."
            Remove-Item Env:COCKPIT_HERMES_BIN
        }
    }

    $desktopAgent = Join-Path $env:LOCALAPPDATA 'Programs\hermes-desktop\hermes-agent.exe'
    if (Test-Path $desktopAgent -PathType Leaf) {
        Write-Host "Hermes Desktop Agent found at $desktopAgent, but it is not a stdio ACP server."
    }

    if (-not $hermesPath) {
        $hermes = Get-Command hermes -ErrorAction SilentlyContinue
        if (-not $hermes) {
            throw 'Hermes ACP CLI was not found. Set COCKPIT_HERMES_BIN to the Hermes CLI executable that supports `hermes acp`.'
        }
        $hermesPath = $hermes.Source
    }
    $env:COCKPIT_HERMES_BIN = $hermesPath
    Write-Host "Using Hermes ACP CLI: $env:COCKPIT_HERMES_BIN"

    # Live simulations use a dedicated Hermes profile. Its empty skill library
    # prevents unrelated personal skills from becoming system-prompt overhead
    # on every per-human ACP session. Hermes resolves credentials from the
    # default profile as a read-only fallback.
    $profileRoot = Join-Path $env:LOCALAPPDATA 'hermes\profiles\iota-cockpit'
    if (-not (Test-Path $profileRoot -PathType Container)) {
        Write-Host 'Creating the isolated Hermes profile for Cockpit live simulations'
        & $env:COCKPIT_HERMES_BIN profile create iota-cockpit --no-skills --no-alias
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }
}

function Clean-RustWorkspaceIfRequested {
    if (-not $Clean) {
        return
    }

    Write-Host 'Cleaning Rust workspace (requested with -Clean)'
    & cargo clean
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

Set-Location $PSScriptRoot

Require-Command cargo
Require-Command npm
Configure-HermesBackend
Clean-RustWorkspaceIfRequested

Release-DevPort

Set-Location 'apps/cockpit-desktop'
Ensure-FrontendDependencies

Write-Host "Starting Cockpit Desktop on port $devPort"
& npm run tauri:dev
exit $LASTEXITCODE
