[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^(x86_64|i686|aarch64)-(pc-windows-msvc|unknown-linux-gnu|apple-darwin)$')][string]$Target,
    [string]$Python = 'python',
    [switch]$Ocr
)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location -LiteralPath $root
try {
    $packageDir = Join-Path $root 'artifacts/site-extensions'
    New-Item -ItemType Directory -Force $packageDir | Out-Null
    if ($Ocr) {
        & $Python 'extensions/math-ocr/package.py' $Target
        if ($LASTEXITCODE) { throw 'No se pudo empaquetar OCR.' }
        return
    }
    $name = 'pomodoro'
    $folder = Join-Path $root "artifacts/extension-build/$name-$Target"
    New-Item -ItemType Directory -Force $folder | Out-Null
    $manifest = [IO.File]::ReadAllText((Join-Path $root "extensions/$name/extension.toml"))
    $manifest.Replace('@TARGET@', $Target).Replace('@EXE@', '') | Set-Content -LiteralPath (Join-Path $folder 'extension.toml') -Encoding utf8NoBOM
    Copy-Item -LiteralPath 'LICENSE' -Destination (Join-Path $folder 'LICENSE') -Force
    $archive = Join-Path $packageDir "$name-$Target.zip"
    & $Python -c 'import pathlib,sys,zipfile; root=pathlib.Path(sys.argv[1]); files=sorted(p for p in root.rglob("*") if p.is_file()); archive=zipfile.ZipFile(sys.argv[2],"w",compression=zipfile.ZIP_DEFLATED); [archive.write(p,p.relative_to(root).as_posix()) for p in files]; archive.close()' $folder $archive
    if ($LASTEXITCODE) { throw 'No se pudo crear el ZIP.' }
    Write-Output $archive
} finally { Pop-Location }
