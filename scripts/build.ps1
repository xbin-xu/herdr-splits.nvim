# Build the Herdr-side helper and install it into scripts\bin\herdr-splits.
#
# `herdr plugin link` does not run the manifest's [[build]] commands, so local
# development needs this. The installed file deliberately has no extension:
# herdr resolves the relative command against the plugin root and Windows then
# runs that exact path without appending PATHEXT variants.
#
# Usage: pwsh -File scripts\build.ps1 [-Debug]
param(
    [switch]$Debug
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$profileArgs = if ($Debug) { @('--locked') } else { @('--release', '--locked') }

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    # rustup's default install location is often missing from PATH in a
    # non-login shell; look there once before giving up.
    $candidate = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    if (Test-Path $candidate) {
        $env:PATH = (Split-Path -Parent $candidate) + ';' + $env:PATH
    } else {
        Write-Error 'cargo not found. Install Rust from https://rustup.rs (or add %USERPROFILE%\.cargo\bin to PATH).'
        exit 1
    }
}

Write-Host 'building...'
& cargo build @profileArgs --manifest-path (Join-Path $root 'rust/Cargo.toml')
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& cargo run @profileArgs --quiet --manifest-path (Join-Path $root 'rust/Cargo.toml') -- install
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "installed $root\scripts\bin\herdr-splits"
