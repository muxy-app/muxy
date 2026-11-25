# Windows template setup for the e2e harness. Run the machine part as SYSTEM (swiftvm exec --root),
# then the user part as the logged-in account (plain swiftvm exec).
param(
    [Parameter(Mandatory)][ValidateSet('machine', 'user')][string]$Part,
    [string]$User = 'muxy'
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$downloads = Join-Path $env:TEMP 'muxy-setup'
New-Item -ItemType Directory -Force -Path $downloads | Out-Null

function Get-Verified([string]$Url, [string]$Sha256) {
    $file = Join-Path $downloads ([IO.Path]::GetFileName(([Uri]$Url).AbsolutePath))
    if (-not (Test-Path $file) -or (Get-FileHash $file -Algorithm SHA256).Hash -ne $Sha256) {
        Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $file
    }
    $actual = (Get-FileHash $file -Algorithm SHA256).Hash
    if ($actual -ne $Sha256) { throw "Checksum mismatch for $Url ($actual)" }
    $file
}

function Invoke-Installer([string]$File, [string[]]$Arguments) {
    $process = Start-Process -FilePath $File -ArgumentList $Arguments -Wait -PassThru
    if ($process.ExitCode -notin 0, 1638, 3010) { throw "$File failed with exit code $($process.ExitCode)" }
}

function Set-Registry([string]$Key, [string]$Name, $Value) {
    New-Item -Path $Key -Force -ErrorAction SilentlyContinue | Out-Null
    Set-ItemProperty -Path $Key -Name $Name -Value $Value -Type DWord
}

function Add-ToPath([string]$Directory, [EnvironmentVariableTarget]$Scope) {
    $entries = @([Environment]::GetEnvironmentVariable('Path', $Scope) -split ';' | Where-Object { $_ })
    if ($entries -notcontains $Directory) {
        [Environment]::SetEnvironmentVariable('Path', (($entries + $Directory) -join ';'), $Scope)
    }
}

function Install-RootCertificates {
    # Windows fetches roots on demand, but Zig reads only the store's contents.
    $roots = Join-Path $downloads 'roots.sst'
    & certutil.exe -generateSSTFromWU $roots | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'certutil could not fetch the Windows Update root certificates' }
    Import-Certificate -FilePath $roots -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
}

function Install-Machine {
    $profileDir = (Get-CimInstance Win32_UserProfile | Where-Object { $_.LocalPath -like "*\$User" }).LocalPath
    if (-not $profileDir) { throw "No profile for $User. Sign in once first." }

    Set-Registry 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' LongPathsEnabled 1
    Set-Registry 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock' AllowDevelopmentWithoutDevLicense 1
    # Build 26300 ships with symlink evaluation unset, so rustup's proxy links don't run.
    & fsutil behavior set SymlinkEvaluation L2L:1 L2R:1 R2L:0 R2R:0 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'fsutil could not enable local symlink evaluation' }
    Set-Registry 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Personalization' NoLockScreen 1
    Set-Registry 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' NoAutoRebootWithLoggedOnUsers 1
    foreach ($setting in 'standby-timeout-ac', 'monitor-timeout-ac', 'hibernate-timeout-ac', 'disk-timeout-ac') {
        & powercfg /change $setting 0
    }
    & powercfg /setacvalueindex SCHEME_CURRENT SUB_NONE CONSOLELOCK 0
    & powercfg /setactive SCHEME_CURRENT

    Install-RootCertificates

    Add-MpPreference -ExclusionPath (Join-Path $profileDir '.cache\muxy-vm'), (Join-Path $profileDir 'src'),
        (Join-Path $profileDir 'AppData\Local\zig')

    if ((Get-WindowsCapability -Online -Name 'OpenSSH.Server~~~~0.0.1.0').State -ne 'Installed') {
        Add-WindowsCapability -Online -Name 'OpenSSH.Server~~~~0.0.1.0' | Out-Null
    }
    Set-Service sshd -StartupType Automatic
    Start-Service sshd

    $buildTools = Join-Path $downloads 'vs_BuildTools.exe'
    Invoke-WebRequest -UseBasicParsing -Uri 'https://aka.ms/vs/17/release/vs_BuildTools.exe' -OutFile $buildTools
    $signature = Get-AuthenticodeSignature $buildTools
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') {
        throw 'The Visual Studio Build Tools bootstrapper is not signed by Microsoft'
    }
    Invoke-Installer $buildTools @(
        '--quiet', '--wait', '--norestart', '--nocache',
        '--add', 'Microsoft.VisualStudio.Component.VC.Tools.ARM64',
        '--add', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
        '--add', 'Microsoft.VisualStudio.Component.Windows11SDK.26100')

    $llvm = Get-Verified 'https://github.com/llvm/llvm-project/releases/download/llvmorg-23.1.3/LLVM-23.1.3-woa64.msi' `
        '19750061816FF721966DFF175288E940889AAE2DD6E8643EAF9A14A03EE728ED'
    Invoke-Installer msiexec.exe @('/i', "`"$llvm`"", '/qn', '/norestart')
    Add-ToPath 'C:\Program Files\LLVM\bin' Machine

    $git = Get-Verified 'https://github.com/git-for-windows/git/releases/download/v2.56.0.windows.2/Git-2.56.0.2-arm64.exe' `
        'B9CED78CE371577DC966F50AE571909D6731E36D31065D35C8EFA21544C58BF0'
    Invoke-Installer $git @('/VERYSILENT', '/NORESTART', '/NOCANCEL', '/SP-', '/SUPPRESSMSGBOXES')

    $python = Get-Verified 'https://www.python.org/ftp/python/3.14.8/python-3.14.8-arm64.exe' `
        (Get-PythonSha256 '3.14.8' 'python-3.14.8-arm64.exe')
    Invoke-Installer $python @('/quiet', 'InstallAllUsers=1', 'PrependPath=1', 'Include_test=0')
    # An installer that registers only the interpreter leaves Python without its standard library.
    & 'C:\Program Files\Python314-arm64\python.exe' -I -c 'import json, http.server, unittest'
    if ($LASTEXITCODE -ne 0) { throw 'Python was installed without its standard library' }

    $pwsh = Get-Verified 'https://github.com/PowerShell/PowerShell/releases/download/v7.6.7/PowerShell-7.6.7-win-arm64.msi' `
        'DDB2F3613A0AD27817B90601A0827F7347E81DFA5BCE18D4BE8009B8CB0E0A31'
    Invoke-Installer msiexec.exe @('/i', "`"$pwsh`"", '/qn', '/norestart', 'ADD_PATH=1')
}

# python.org publishes each installer's SHA-256 in its release metadata.
function Get-PythonSha256([string]$Version, [string]$Name) {
    # Windows PowerShell passes a JSON array down the pipeline as one object; assigning it first unrolls it.
    $releases = Invoke-RestMethod "https://www.python.org/api/v2/downloads/release/?name=Python%20$Version"
    $release = $releases | Select-Object -First 1
    $slug = ($release.resource_uri -split '/' | Where-Object { $_ })[-1]
    $files = Invoke-RestMethod "https://www.python.org/api/v2/downloads/release_file/?release=$slug"
    $file = $files | Where-Object { $_.url -like "*/$Name" } | Select-Object -First 1
    if (-not $file.sha256_sum) { throw "python.org lists no SHA-256 for $Name" }
    $file.sha256_sum.ToUpperInvariant()
}

function Install-User {
    $share = Join-Path $HOME '.local\share\muxy-vm'
    New-Item -ItemType Directory -Force -Path $share, (Join-Path $HOME '.local\bin') | Out-Null
    [IO.File]::WriteAllText((Join-Path $share 'managed'), "SwiftVM Muxy sandbox`n")

    $rustupInit = Join-Path $downloads 'rustup-init.exe'
    Invoke-WebRequest -UseBasicParsing -Uri 'https://static.rust-lang.org/rustup/dist/aarch64-pc-windows-msvc/rustup-init.exe' -OutFile $rustupInit
    Invoke-WebRequest -UseBasicParsing -Uri 'https://static.rust-lang.org/rustup/dist/aarch64-pc-windows-msvc/rustup-init.exe.sha256' -OutFile "$rustupInit.sha256"
    $expected = ([IO.File]::ReadAllText("$rustupInit.sha256").Trim() -split '\s+')[0]
    if ((Get-FileHash $rustupInit -Algorithm SHA256).Hash -ne $expected.ToUpperInvariant()) { throw 'rustup-init checksum mismatch' }
    & $rustupInit -y --profile minimal --default-host aarch64-pc-windows-msvc --default-toolchain 1.92.0 --no-modify-path
    if ($LASTEXITCODE -ne 0) { throw "rustup-init failed with exit code $LASTEXITCODE" }
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    Add-ToPath $cargoBin User
    & "$cargoBin\rustup.exe" component add --toolchain 1.92.0 rustfmt clippy
    if ($LASTEXITCODE -ne 0) { throw 'rustup component add failed' }
    & "$cargoBin\rustup.exe" target add --toolchain 1.92.0 aarch64-pc-windows-msvc x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'rustup target add failed' }

    $zigIndex = Invoke-RestMethod 'https://ziglang.org/download/index.json'
    $zigRelease = $zigIndex.'0.17.0'.'aarch64-windows'
    $zigArchive = Get-Verified $zigRelease.tarball $zigRelease.shasum.ToUpperInvariant()
    $zigDir = Join-Path $HOME '.local\zig-0.17.0'
    if (-not (Test-Path (Join-Path $zigDir 'zig.exe'))) {
        $stage = Join-Path $downloads 'zig'
        Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
        Expand-Archive -Path $zigArchive -DestinationPath $stage
        New-Item -ItemType Directory -Force -Path (Split-Path $zigDir) | Out-Null
        Move-Item (Get-ChildItem $stage -Directory | Select-Object -First 1).FullName $zigDir
    }

    $git = 'C:\Program Files\Git\cmd\git.exe'
    foreach ($setting in @('core.longpaths', 'true'), @('core.autocrlf', 'false'), @('user.name', 'Muxy E2E'),
        @('user.email', 'e2e@muxy.invalid'), @('init.defaultBranch', 'main')) {
        & $git config --global $setting[0] $setting[1]
        if ($LASTEXITCODE -ne 0) { throw "git config $($setting[0]) failed" }
    }

    Set-Registry 'HKCU:\Software\Microsoft\Windows\CurrentVersion\PushNotifications' ToastEnabled 1
    Set-Registry 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings' NOC_GLOBAL_SETTING_TOASTS_ENABLED 1
    Set-Registry 'HKCU:\Control Panel\Desktop' ScreenSaveActive 0
}

if ($Part -eq 'machine') { Install-Machine } else { Install-User }
Write-Output "Windows $Part setup finished"
