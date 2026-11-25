# Builds an optimized Muxy from ~\src\muxy and installs it for the signed-in
# user, with Desktop and Start menu shortcuts and `muxy` on the PATH.
$ProgressPreference = 'SilentlyContinue'

$env:Path = "$HOME\.cargo\bin;$HOME\.local\zig-0.17.0;$env:Path"
$env:LIBGHOSTTY_VT_SYS_OPTIMIZE = 'ReleaseFast'
$env:CARGO_TARGET_DIR = Join-Path $HOME '.cache\muxy-release'
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '8' }

Set-Location (Join-Path $HOME 'src\muxy')
& cmd.exe /d /c 'cargo build --locked --release -p muxy-app -p muxy-cli -p muxy-server 2>&1'
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$install = Join-Path $env:LOCALAPPDATA 'Programs\Muxy'
# Windows can't replace a running program, so the app and server stop first.
Get-Process muxy-app -ErrorAction SilentlyContinue | Stop-Process -Force
if (Test-Path "$install\muxy.exe") { & "$install\muxy.exe" server stop --force *> $null }
Get-Process muxy-server -ErrorAction SilentlyContinue | Stop-Process -Force
New-Item -ItemType Directory -Force -Path $install | Out-Null
foreach ($binary in 'muxy-app', 'muxy', 'muxy-server') {
    Copy-Item (Join-Path $env:CARGO_TARGET_DIR "release\$binary.exe") $install -Force
}

$path = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $_ })
if ($path -notcontains $install) {
    [Environment]::SetEnvironmentVariable('Path', (($path + $install) -join ';'), 'User')
}

$shell = New-Object -ComObject WScript.Shell
$startMenu = Join-Path ([Environment]::GetFolderPath('Programs')) 'Muxy.lnk'
foreach ($link in (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Muxy.lnk'), $startMenu) {
    $shortcut = $shell.CreateShortcut($link)
    $shortcut.TargetPath = Join-Path $install 'muxy-app.exe'
    $shortcut.WorkingDirectory = $HOME
    $shortcut.Save()
}

foreach ($binary in 'muxy-app', 'muxy', 'muxy-server') {
    "${binary}: $((& (Join-Path $install "$binary.exe") --build-info | Out-String).Trim())"
}
"Installed in $install"
