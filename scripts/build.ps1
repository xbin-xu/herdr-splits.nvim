# Build the Herdr-side helper and install it into scripts\bin\herdr-splits.
#
# `herdr plugin link` does not run the manifest's [[build]] commands, so local
# development needs this. The installed file deliberately has no extension:
# herdr resolves the relative command against the plugin root and Windows then
# runs that exact path without appending PATHEXT variants. `install` also writes
# an identical .exe mirror, because shells cannot execute the extensionless one.
#
# Usage: pwsh -File scripts\build.ps1 [-Debug]
param(
    [switch]$Debug
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$profileArgs = if ($Debug) { @('--locked') } else { @('--release', '--locked') }
$manifest = Join-Path $root 'rust/Cargo.toml'

# cargo is often installed but missing from PATH: rustup puts its proxies in
# %USERPROFILE%\.cargo\bin and leaves editing PATH to the installer, while a
# process started before that edit (the herdr server, a desktop-launched editor)
# still has the old environment. Look in the standard places before failing.
function Find-Cargo {
    if ($env:CARGO) { return $env:CARGO }

    $onPath = Get-Command cargo -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }

    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    foreach ($name in 'cargo.exe', 'cargo') {
        $candidate = Join-Path (Join-Path $cargoHome 'bin') $name
        if (Test-Path -LiteralPath $candidate) { return $candidate }
    }

    # Last resort: a toolchain directory. Not a rustup proxy, but a working
    # cargo, which is all this script needs.
    $toolchains = Join-Path $env:USERPROFILE '.rustup\toolchains'
    if (Test-Path -LiteralPath $toolchains) {
        foreach ($dir in Get-ChildItem -LiteralPath $toolchains -Directory) {
            $candidate = Join-Path $dir.FullName 'bin\cargo.exe'
            if (Test-Path -LiteralPath $candidate) { return $candidate }
        }
    }
    return $null
}

$cargo = Find-Cargo
if (-not $cargo) {
    Write-Error @'
cargo not found.
  Install Rust from https://rustup.rs, or run this from a shell where
  'cargo --version' works, or set CARGO to the full path of cargo.exe.
'@
    exit 1
}

Write-Host "building with $cargo ..."
& $cargo build @profileArgs --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# `--root` is explicit on purpose: with CARGO_TARGET_DIR set, the binary lives
# outside the checkout and could not find herdr-plugin.toml by walking up.
& $cargo run @profileArgs --quiet --manifest-path $manifest -- install --root $root
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "installed $root\scripts\bin\herdr-splits"
