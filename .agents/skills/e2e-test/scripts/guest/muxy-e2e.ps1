# Guest-side helper for Muxy E2E runs on Windows. Installed by `e2e up` as
# ~\.local\bin\muxy-e2e.ps1. Targets Windows PowerShell 5.1.
$ProgressPreference = 'SilentlyContinue'
# The host reads this helper's output as UTF-8; Windows PowerShell writes the OEM code page otherwise.
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$OutputEncoding = [Console]::OutputEncoding

$env:MUXY_DIR = Join-Path $HOME '.local\state\muxy-vm'
$Bin = Join-Path $HOME '.cache\muxy-vm\target\debug'
$env:MUXY_SERVER_BIN = Join-Path $Bin 'muxy-server.exe'
$env:Path = "$Bin;$HOME\.cargo\bin;$HOME\.local\bin;$env:Path"
$Share = Join-Path $HOME '.local\share\muxy-vm'
$AppLog = Join-Path $Share 'app.log'
$Fixtures = Join-Path $HOME 'e2e'
$HttpPort = 8765

function Fail([string]$Message) { [Console]::Error.WriteLine("muxy-e2e: $Message"); exit 1 }

# Steps single-quote paths like '~/e2e/beta' so the host shell keeps them; expand them here.
function Expand-Home([string]$Path) {
    if ($Path -eq '~') { return $HOME }
    if ($Path -like '~/*' -or $Path -like '~\*') { return Join-Path $HOME $Path.Substring(2) }
    $Path
}

function Invoke-Muxy { & (Join-Path $Bin 'muxy.exe') @args }

function Get-AppProcess { Get-Process -Name muxy-app -ErrorAction SilentlyContinue | Select-Object -First 1 }

function Get-Sessions {
    $json = Invoke-Muxy session list --json 2>$null
    if ($LASTEXITCODE -ne 0 -or -not $json) { return @() }
    @(($json | Out-String | ConvertFrom-Json) | Where-Object { $_.status -eq 'Live' })
}

# read-screen returns the last 50 rows by default, which misses the top of a taller window.
function Get-Screen([string]$Id) {
    @(Invoke-Muxy session read-screen $Id --lines 500 2>$null) | Where-Object { $_.Trim() }
}

function Get-ProjectName([string]$Id) {
    $json = Invoke-Muxy project list --json 2>$null
    if (-not $json) { return '' }
    (($json | Out-String | ConvertFrom-Json) | Where-Object { $_.id -eq $Id } | Select-Object -First 1).name
}

function Test-Server { Invoke-Muxy server status *> $null; $LASTEXITCODE -eq 0 }

function Invoke-Launch {
    $app = Get-AppProcess
    if ($app) { "app already running: pid $($app.Id)"; return }
    New-Item -ItemType Directory -Force -Path $Share, $env:MUXY_DIR | Out-Null
    Add-Content -Path $AppLog -Value "`n==== launch $(Get-Date)"
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $HOME '.local\bin\muxy-vm-run.ps1') | Out-Null
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Milliseconds 500
        $app = Get-AppProcess
        if ($app -and (Test-Server)) {
            Start-Sleep -Seconds 3
            "app running: pid $($app.Id); server: $(@(Invoke-Muxy server status 2>&1)[0])"
            return
        }
    }
    Get-Content $AppLog -Tail 30 -ErrorAction SilentlyContinue | ForEach-Object { [Console]::Error.WriteLine($_) }
    Fail 'the app did not start within 30 seconds (log above)'
}

# Windows has no SIGTERM. taskkill without /F asks the app's windows to close, as
# a quit from the window would, then the app is ended if it is still there.
function Invoke-Quit {
    $app = Get-AppProcess
    if (-not $app) { 'app not running'; return }
    & taskkill.exe /PID $app.Id *> $null
    for ($i = 0; $i -lt 20; $i++) {
        Start-Sleep -Milliseconds 500
        if (-not (Get-AppProcess)) { 'app quit'; return }
    }
    & taskkill.exe /F /PID $app.Id *> $null
    'app did not close its window; killed'
}

function Invoke-StopAll {
    Invoke-Quit | Out-Null
    Invoke-Muxy server stop --force *> $null
    Get-Process -Name muxy-server -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Invoke-HttpStop | Out-Null
    'app, server and helpers stopped'
}

function Write-Text([string]$Path, [string]$Text) {
    New-Item -ItemType Directory -Force -Path (Split-Path $Path) | Out-Null
    [IO.File]::WriteAllText($Path, $Text.Replace("`r`n", "`n"))
}

function Invoke-Git([string]$Directory) {
    & git.exe -C $Directory @args 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { Fail "git $($args -join ' ') failed in $Directory" }
}

function Invoke-Fixtures {
    if (Test-Path $Fixtures) { Remove-Item -Recurse -Force $Fixtures }
    New-Item -ItemType Directory -Force -Path $Fixtures | Out-Null
    $gamma = Join-Path $Fixtures 'gamma'
    Write-Text "$gamma\README.md" "# gamma`nFixture repository for Muxy's worktree tests.`n"
    Write-Text "$gamma\.muxy\worktree.json" @'
{
  "setup": ["echo setup-$env:MUXY_WORKTREE_BRANCH >> ~/e2e/hook-log.txt"],
  "teardown": ["echo teardown-$env:MUXY_WORKTREE_BRANCH >> ~/e2e/hook-log.txt"]
}
'@
    Invoke-Git $gamma init -q -b main
    Invoke-Git $gamma add -A
    Invoke-Git $gamma commit -qm 'Initial fixture'
    Invoke-Git $gamma checkout -q -b topic
    Write-Text "$gamma\topic.txt" "only on topic`n"
    Invoke-Git $gamma add -A
    Invoke-Git $gamma commit -qm 'Topic work'
    Invoke-Git $gamma checkout -q main
    $alpha = Join-Path $Fixtures 'alpha'
    Write-Text "$alpha\README.md" "# alpha`nFixture project for Muxy E2E tests.`n"
    Write-Text "$alpha\.muxy\layouts\trio.yaml" @'
layout: horizontal
panes:
  - tab:
      name: left
      command: echo layout-left-$(40+2)
  - layout: vertical
    panes:
      - tab: echo layout-top-$(40+2)
      - tab:
          name: bottom
          command: echo layout-bottom-$(40+2)
'@
    Invoke-Git $alpha init -q -b main
    Invoke-Git $alpha add -A
    Invoke-Git $alpha commit -qm 'Initial fixture'
    Write-Text "$Fixtures\beta\notes.txt" "beta notes`n"
    Write-Text "$Fixtures\site\index.html" @'
<!doctype html>
<html><head><meta charset="utf-8"><title>E2E Home</title></head>
<body style="font-family: sans-serif; background: #fff7d6">
<h1 id="greeting">Hello from Muxy E2E</h1>
<p>Fixture page served from the guest.</p>
<input id="name" placeholder="Your name">
<button id="go" onclick="document.getElementById('out').textContent = 'Saved ' + document.getElementById('name').value">Save</button>
<p id="out"></p>
<a id="next" href="next.html">Go to the second page</a>
</body></html>
'@
    Write-Text "$Fixtures\site\next.html" @'
<!doctype html>
<html><head><meta charset="utf-8"><title>E2E Second</title></head>
<body style="font-family: sans-serif; background: #d6f0ff"><h1>Second page reached</h1></body></html>
'@
    "fixtures ready in ${Fixtures}: alpha (git repo with .muxy\layouts\trio.yaml), beta, gamma (git repo with worktree hooks and a topic branch), site"
}

function Invoke-Reset {
    Invoke-StopAll | Out-Null
    if (-not (Test-Path (Join-Path $Share 'managed'))) { Fail 'not a prepared Muxy test guest' }
    foreach ($path in $env:MUXY_DIR, (Join-Path $HOME '.muxy\worktrees')) {
        if (Test-Path $path) { Remove-Item -Recurse -Force $path }
    }
    # A fresh profile would open the welcome tour over everything; suites start past it.
    Write-Text (Join-Path $env:MUXY_DIR 'settings.toml') "[appearance]`nonboarding_completed = true`n"
    Set-Clipboard -Value $null -ErrorAction SilentlyContinue
    Invoke-Fixtures
    Invoke-Launch
}

function Invoke-Screens {
    foreach ($session in Get-Sessions) {
        "=== session $($session.id) (project: $(Get-ProjectName $session.project_id))"
        Get-Screen $session.id
    }
}

function Find-Text([string]$Text) {
    foreach ($session in Get-Sessions) {
        $line = Get-Screen $session.id | Where-Object { $_.Contains($Text) } | Select-Object -Last 1
        if ($line) { "$($session.id): $line" }
    }
}

function Invoke-Find([string]$Text) {
    $lines = @(Find-Text $Text)
    if (-not $lines) { exit 1 }
    $lines
}

function Invoke-Wait([string]$Text, [int]$Seconds) {
    for ($i = 0; $i -lt $Seconds * 2; $i++) {
        $lines = @(Find-Text $Text)
        if ($lines) { $lines; return }
        Start-Sleep -Milliseconds 500
    }
    Fail "not found on any live terminal screen after ${Seconds}s: $Text"
}

function Invoke-Gone([string]$Text, [int]$Seconds) {
    for ($i = 0; $i -lt $Seconds * 2; $i++) {
        if (-not @(Find-Text $Text)) { "gone: $Text"; return }
        Start-Sleep -Milliseconds 500
    }
    Fail "still on screen after ${Seconds}s: $Text"
}

function Invoke-HttpStart {
    Invoke-HttpStop | Out-Null
    if (-not (Test-Path "$Fixtures\site")) { Invoke-Fixtures | Out-Null }
    New-Item -ItemType Directory -Force -Path $Share | Out-Null
    $server = Start-Process python.exe -ArgumentList '-m', 'http.server', $HttpPort, '--bind', '127.0.0.1', '--directory', "`"$Fixtures\site`"" `
        -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $Share 'http.log')
    [IO.File]::WriteAllText((Join-Path $Share 'http.pid'), "$($server.Id)")
    Start-Sleep -Seconds 1
    $page = try { (Invoke-WebRequest -UseBasicParsing "http://127.0.0.1:$HttpPort/").Content } catch { '' }
    if ($page -notmatch 'Hello from Muxy E2E') { Fail 'fixture web server did not answer' }
    "serving http://127.0.0.1:$HttpPort/"
}

function Invoke-HttpStop {
    $pidFile = Join-Path $Share 'http.pid'
    if (Test-Path $pidFile) {
        Stop-Process -Id ([int](Get-Content $pidFile)) -Force -ErrorAction SilentlyContinue
        Remove-Item $pidFile
    }
    'web server stopped'
}

function Invoke-Info {
    $os = Get-CimInstance Win32_OperatingSystem
    # PROCESSOR_ARCHITECTURE reads AMD64 under x64 emulation, so ask the processor.
    $arch = if ((Get-CimInstance Win32_Processor | Select-Object -First 1).Architecture -eq 12) { 'ARM64' } else { 'x64' }
    "os: $($os.Caption) $($os.Version) (build $($os.BuildNumber)) $arch"
    'session: windows'
    "shell: $((Get-Command pwsh.exe -ErrorAction SilentlyContinue).Source)"
    foreach ($binary in 'muxy-app', 'muxy', 'muxy-server') {
        # Capturing the output makes PowerShell wait for the GUI-subsystem app.
        "${binary}: $(& (Join-Path $Bin "$binary.exe") --build-info 2>&1 | Out-String)".Trim()
    }
}

function Invoke-Logs {
    '### app.log (last 80 lines)'; Get-Content $AppLog -Tail 80 -ErrorAction SilentlyContinue
    '### app.err.log (last 40 lines)'; Get-Content (Join-Path $Share 'app.err.log') -Tail 40 -ErrorAction SilentlyContinue
    '### server.log (last 80 lines)'; Get-Content (Join-Path $env:MUXY_DIR 'server.log') -Tail 80 -ErrorAction SilentlyContinue
    foreach ($file in Get-ChildItem (Join-Path $env:MUXY_DIR 'app-diagnostics-*.log') -ErrorAction SilentlyContinue) {
        "### $($file.Name) (last 40 lines)"; Get-Content $file.FullName -Tail 40
    }
    '### processes'
    Get-Process | Where-Object { $_.Name -match 'muxy|pwsh|powershell|conhost|python' } |
        Format-Table Id, Name, StartTime -AutoSize | Out-String -Width 200
}

function Invoke-Running([string]$Text) {
    $found = @(Get-CimInstance Win32_Process | Where-Object {
            # Windows names programs in any case, such as PING.EXE.
            $_.CommandLine -and $_.CommandLine.IndexOf($Text, [StringComparison]::OrdinalIgnoreCase) -ge 0 -and
            $_.CommandLine -notmatch 'muxy-e2e'
        } | ForEach-Object { "$($_.ProcessId) $($_.CommandLine)" })
    if (-not $found) { "no process matches: $Text"; exit 1 }
    $found
}

function Invoke-RmTree([string]$Path) {
    $full = [IO.Path]::GetFullPath((Expand-Home $Path))
    $allowed = @($Fixtures, (Join-Path $HOME '.muxy\worktrees')) | ForEach-Object { [IO.Path]::GetFullPath($_) + '\' }
    if (-not ($allowed | Where-Object { $full.StartsWith($_, [StringComparison]::OrdinalIgnoreCase) })) {
        Fail 'rm-tree only deletes inside ~/e2e or ~/.muxy/worktrees'
    }
    Remove-Item -Recurse -Force $full
    "deleted $full"
}

function Show-Usage {
    @'
Usage: muxy-e2e.ps1 <command> [arguments]   (guest side; on the host use: e2e g <vm> <command>)

  reset                  Quit the app, stop the server, wipe the test profile and fixtures, recreate fixtures, launch
  launch                 Start the app if it isn't running and wait until it is connected
  quit                   Close the app (the server and terminals keep running)
  stop-all               Stop the app, the server and the fixture web server
  fixtures               Recreate ~/e2e: alpha (git repo with a layout), beta, gamma (git repo with hooks), site
  muxy <args>            Run the CLI against the test profile, e.g. muxy session list
  git <folder> <args>    Run Git in a folder, outside Muxy
  file <path>            Print a file (says so if it doesn't exist)
  exists <path>          Print yes or no
  rm-tree <path>         Delete a folder inside ~/e2e or ~/.muxy/worktrees, outside Muxy
  screens                Print every live terminal's screen
  find <text>            Print live terminals whose screen shows <text>; exit 1 if none
  wait <text> [seconds]  Wait until some live terminal shows <text> (default 15s)
  gone <text> [seconds]  Wait until no live terminal shows <text>
  clip-set <text>        Put text on the guest clipboard
  clip-get               Print the guest clipboard
  http-start | http-stop Serve ~/e2e/site on http://127.0.0.1:8765/
  info                   OS, architecture and build info
  logs                   Tails of the app/server logs and the running processes
  running <text>         Print processes whose command line contains <text>; exit 1 if none
  config <file>          Print a file from the test profile, e.g. settings.toml or terminal.toml
  lines <path>           Count the lines of a file (0 if it doesn't exist)
  app-pid                Print the app's pid (empty if not running)
  link-launch            Restart the app; it registers muxy-beta:// links for itself when it starts
  open-link <url>        Open a URL as a browser would (start), e.g. a muxy-beta:// link
  open-session <id|text> [right|down]  Print muxy session link for that terminal (by id, or the one live terminal showing text) and open it

Other muxy-e2e commands (TUI, agent, extension, image and IME fixtures) are not available on Windows.
'@
}

function Invoke-OpenLink([string]$Url) {
    Start-Process cmd.exe -ArgumentList '/d', '/c', "start `"`" `"$Url`"" -WindowStyle Hidden -WorkingDirectory $HOME
    Start-Sleep -Seconds 3
    "opened $Url"
}

function Invoke-OpenSession([string[]]$Arguments) {
    if ($Arguments.Count -lt 1) { Fail 'usage: open-session <id|text> [right|down]' }
    $id = $Arguments[0]
    if ($id -notmatch '^[0-9]+$') {
        $found = @(Get-Sessions | Where-Object { @(Get-Screen $_.id | Where-Object { $_.Contains($Arguments[0]) }).Count -gt 0 })
        if ($found.Count -ne 1) { Fail "expected one live terminal showing '$($Arguments[0])', found $($found.Count)" }
        $id = "$($found[0].id)"
        "session: $id"
    }
    $link = if ($Arguments.Count -gt 1) { Invoke-Muxy session link $id --split $Arguments[1] } else { Invoke-Muxy session link $id }
    if ($LASTEXITCODE -ne 0 -or -not $link) { Fail 'muxy session link failed' }
    "link: $link"
    Invoke-OpenLink "$link"
}

$command = if ($args.Count) { $args[0] } else { 'help' }
$rest = @($args | Select-Object -Skip 1)
switch ($command) {
    'reset' { Invoke-Reset }
    'launch' { Invoke-Launch }
    'quit' { Invoke-Quit }
    'stop-all' { Invoke-StopAll }
    'fixtures' { Invoke-Fixtures }
    'muxy' { Invoke-Muxy @($rest | ForEach-Object { Expand-Home $_ }); exit $LASTEXITCODE }
    'git' {
        if ($rest.Count -lt 1) { Fail 'usage: git <folder> <args>' }
        & git.exe -C (Expand-Home $rest[0]) @($rest | Select-Object -Skip 1 | ForEach-Object { Expand-Home $_ })
        exit $LASTEXITCODE
    }
    'file' {
        $file = Expand-Home $rest[0]
        if (Test-Path $file -PathType Leaf) { Get-Content -Raw $file } else { "no such file: $($rest[0])" }
    }
    'exists' { if (Test-Path (Expand-Home $rest[0])) { 'yes' } else { 'no' } }
    'rm-tree' { Invoke-RmTree $rest[0] }
    'screens' { Invoke-Screens }
    'find' { if ($rest.Count -lt 1) { Fail 'usage: find <text>' }; Invoke-Find $rest[0] }
    'wait' { if ($rest.Count -lt 1) { Fail 'usage: wait <text> [seconds]' }; Invoke-Wait $rest[0] $(if ($rest.Count -gt 1) { $rest[1] } else { 15 }) }
    'gone' { if ($rest.Count -lt 1) { Fail 'usage: gone <text> [seconds]' }; Invoke-Gone $rest[0] $(if ($rest.Count -gt 1) { $rest[1] } else { 15 }) }
    'clip-set' { if ($rest.Count -and $rest[0]) { Set-Clipboard -Value $rest[0] } else { Set-Clipboard -Value $null } }
    'clip-get' { Get-Clipboard -Raw }
    'http-start' { Invoke-HttpStart }
    'http-stop' { Invoke-HttpStop }
    'info' { Invoke-Info }
    'logs' { Invoke-Logs }
    'running' { Invoke-Running $rest[0] }
    'config' {
        $file = Join-Path $env:MUXY_DIR $rest[0]
        if (-not (Test-Path $file)) { Fail "no $($rest[0]) in the test profile" }
        Get-Content -Raw $file
    }
    'lines' {
        $file = Expand-Home $rest[0]
        if (Test-Path $file -PathType Leaf) { @(Get-Content $file).Count } else { 0 }
    }
    'app-pid' { $app = Get-AppProcess; if ($app) { $app.Id } }
    'link-launch' { Invoke-Quit | Out-Null; Invoke-Launch; "muxy-beta:// links open the app, which registers itself when it starts" }
    'open-link' { if ($rest.Count -lt 1) { Fail 'usage: open-link <url>' }; Invoke-OpenLink $rest[0] }
    'open-session' { Invoke-OpenSession $rest }
    { $_ -in 'help', '-h', '--help' } { Show-Usage }
    default { Show-Usage | ForEach-Object { [Console]::Error.WriteLine($_) }; Fail "'$command' is not available on Windows" }
}
