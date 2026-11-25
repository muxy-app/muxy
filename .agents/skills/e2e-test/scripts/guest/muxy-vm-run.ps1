# Starts the app on the signed-in account's desktop and returns once it runs.
# Set MUXY_E2E_HIDE_CLI to start it with no muxy on its PATH.
$ErrorActionPreference = 'Stop'

$debugDir = Join-Path $HOME '.cache\muxy-vm\target\debug'
$share = Join-Path $HOME '.local\share\muxy-vm'
$env:Path = if ($env:MUXY_E2E_HIDE_CLI) {
    (($env:Path -split ';') | Where-Object { $_ -and $_ -ne $debugDir }) -join ';'
} else { "$debugDir;$HOME\.cargo\bin;$HOME\.local\bin;$env:Path" }
$env:MUXY_DIR = Join-Path $HOME '.local\state\muxy-vm'
$env:MUXY_SERVER_BIN = Join-Path $debugDir 'muxy-server.exe'
New-Item -ItemType Directory -Force -Path $env:MUXY_DIR, $share | Out-Null

# Redirecting through Start-Process would let the app inherit swiftvm exec's
# pipes, and exec would then wait for the app to quit. cmd.exe, started
# through the shell, inherits nothing and writes the logs instead.
$arguments = ($args | ForEach-Object { "`"$_`"" }) -join ' '
$command = "`"$debugDir\muxy-app.exe`" $arguments >>`"$share\app.log`" 2>>`"$share\app.err.log`""
$shell = Start-Process cmd.exe -ArgumentList '/d', '/c', "`"$command`"" -PassThru -WindowStyle Hidden `
    -WorkingDirectory $HOME

$deadline = (Get-Date).AddSeconds(10)
do {
    $app = Get-CimInstance Win32_Process -Filter "ParentProcessId = $($shell.Id) AND Name = 'muxy-app.exe'"
    if ($app) { break }
    Start-Sleep -Milliseconds 200
} while ((Get-Date) -lt $deadline -and -not $shell.HasExited)
if (-not $app) { throw "muxy-app did not start; see $share\app.err.log" }

[IO.File]::WriteAllText((Join-Path $share 'app.pid'), "$($app.ProcessId)")
"muxy-app started with pid $($app.ProcessId)"
