# Guest account; input must come from export-source.py. Keeps the old source tree.
param([Parameter(Mandatory)][string]$Archive)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

if (-not (Test-Path (Join-Path $HOME '.local\share\muxy-vm\managed'))) { throw 'Not a prepared Muxy test guest' }
$srcDir = Join-Path $HOME 'src'
$target = Join-Path $srcDir 'muxy'
New-Item -ItemType Directory -Force -Path $srcDir | Out-Null
$stage = Join-Path $srcDir ('.muxy-import.' + [IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $stage | Out-Null
try {
    & tar.exe -xmf $Archive -C $stage
    if ($LASTEXITCODE -ne 0) { throw "tar failed with exit code $LASTEXITCODE" }
    foreach ($required in 'Cargo.toml', 'Cargo.lock', 'crates\muxy-app\Cargo.toml') {
        if (-not (Test-Path (Join-Path $stage $required))) { throw "The archive has no $required" }
    }
    if (Test-Path $target) {
        # Extracted files carry the import time. Unchanged ones keep the old time,
        # so Cargo reuses their build.
        $old = (Resolve-Path $target).Path
        foreach ($file in Get-ChildItem -LiteralPath $stage -Recurse -File -Force) {
            $previousFile = Join-Path $old $file.FullName.Substring($stage.Length + 1)
            if ((Test-Path -LiteralPath $previousFile -PathType Leaf) -and
                (Get-Item -LiteralPath $previousFile).Length -eq $file.Length -and
                (Get-FileHash -LiteralPath $previousFile).Hash -eq (Get-FileHash -LiteralPath $file.FullName).Hash) {
                $file.LastWriteTimeUtc = (Get-Item -LiteralPath $previousFile).LastWriteTimeUtc
            }
        }
        $previous = Join-Path $srcDir ('muxy-previous.' + [IO.Path]::GetRandomFileName())
        Move-Item $target $previous
        "Previous source: $previous"
    }
    Move-Item $stage $target
    $stage = $null
} finally {
    if ($stage) { Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue }
}
"Source ready: $target"
