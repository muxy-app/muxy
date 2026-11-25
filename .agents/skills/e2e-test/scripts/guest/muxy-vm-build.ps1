# Run inside a prepared Windows guest, as the signed-in account.
param([string]$SourceDir = (Join-Path $HOME 'src\muxy'))
$ProgressPreference = 'SilentlyContinue'

$env:Path = "$HOME\.cargo\bin;$HOME\.local\zig-0.17.0;$env:Path"
$env:LIBGHOSTTY_VT_SYS_OPTIMIZE = 'ReleaseFast'
$env:CARGO_TARGET_DIR = Join-Path $HOME '.cache\muxy-vm\target'
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '6' }
$env:CARGO_PROFILE_DEV_DEBUG = '1'
$env:CARGO_INCREMENTAL = '0'

Set-Location $SourceDir
# Cargo writes progress to stderr; going through cmd keeps PowerShell from turning it into errors.
& cmd.exe /d /c 'cargo build --locked -p muxy-app -p muxy-cli -p muxy-server 2>&1'
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# All three print the same build-info JSON when they come from one build.
$builds = foreach ($binary in 'muxy-app', 'muxy', 'muxy-server') {
    # Capturing the output makes PowerShell wait for the GUI-subsystem app.
    $info = (& (Join-Path $env:CARGO_TARGET_DIR "debug\$binary.exe") --build-info | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $info) { [Console]::Error.WriteLine("$binary --build-info failed"); exit 1 }
    "${binary}: $info" | Write-Host
    $info
}
if (@($builds | Sort-Object -Unique).Count -ne 1) {
    [Console]::Error.WriteLine('The binaries report different builds')
    exit 1
}
