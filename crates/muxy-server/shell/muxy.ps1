# Runs after the user's profile, so it wraps the prompt and PSReadLine they chose.
if ($env:MUXY_SHELL_INTEGRATION -ne '1' -or $Global:__MuxyInstalled) { return }
$Global:__MuxyInstalled = $true
$Global:__MuxyRunning = $false
$Global:__MuxyPrompt = $function:prompt

function Global:__MuxyMark([string] $Text) {
    "$([char]27)]$Text$([char]7)"
}

# OSC 7 for drive paths only, percent-encoded as UTF-8.
function Global:__MuxyDirectory {
    if ($PWD.Provider.Name -ne 'FileSystem') { return '' }
    $path = $PWD.ProviderPath -replace '\\', '/'
    if ($path -notmatch '^[A-Za-z]:/') { return '' }
    $encoded = New-Object System.Text.StringBuilder
    foreach ($byte in [System.Text.Encoding]::UTF8.GetBytes($path)) {
        $character = [char]$byte
        if ($byte -lt 128 -and $character -match '[A-Za-z0-9/._~:-]') {
            [void]$encoded.Append($character)
        } else {
            [void]$encoded.Append('%{0:X2}' -f $byte)
        }
    }
    __MuxyMark "7;file:///$encoded"
}

function Global:prompt {
    $succeeded = $global:?
    $code = $global:LASTEXITCODE
    $marks = ''
    if ($Global:__MuxyRunning) {
        $result = 0
        if (-not $succeeded) {
            $result = 1
            if ($code -is [int] -and $code -ne 0) { $result = $code }
        }
        $marks += __MuxyMark "133;D;$result"
    }
    $Global:__MuxyRunning = $false
    $marks += __MuxyDirectory
    $marks += __MuxyMark '133;A'
    # Let the user's prompt see the command's failure in $?.
    if (-not $succeeded) { Write-Error 'failed' -ErrorAction Ignore }
    $text = (& $Global:__MuxyPrompt) -join ''
    $global:LASTEXITCODE = $code
    "$marks$text$(__MuxyMark '133;B')"
}

# PSReadLine returns the line just before it runs, which marks the command's start.
# Only the session is searched: looking through every installed module can take minutes.
if (Test-Path Function:\PSConsoleHostReadLine) {
    $Global:__MuxyReadLine = $function:PSConsoleHostReadLine
    function Global:PSConsoleHostReadLine {
        $line = $Global:__MuxyReadLine.Invoke()
        $Global:__MuxyRunning = $true
        [Console]::Write((__MuxyMark '133;C'))
        $line
    }
}
