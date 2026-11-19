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
    $name = if ($Ocr) { 'math-ocr' } else { 'pomodoro' }
    $tmp = Join-Path $root "artifacts/extension-build/$name-$Target"
    New-Item -ItemType Directory -Force $tmp | Out-Null
    $exe = ''
    $folder = $tmp
    if ($Ocr) {
        if ($Target -notin @('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu')) { throw 'Plataforma OCR no disponible.' }
        $exe = if ($Target -like '*windows*') { 'ocr.exe' } else { 'ocr' }
        & $Python -m PyInstaller --noconfirm --onedir --name ocr --distpath "$tmp/dist" --workpath "$tmp/work" --specpath $tmp --collect-data rapid_latex_ocr --collect-binaries onnxruntime --exclude-module tkinter 'extensions/math-ocr/ocr.py'
        if ($LASTEXITCODE) { throw 'No se pudo empaquetar OCR.' }
        $folder = Join-Path $tmp 'dist/ocr'
        & $Python -m pip list --format=freeze | Set-Content -LiteralPath (Join-Path $folder 'DEPENDENCIAS.txt') -Encoding utf8NoBOM
        if ($LASTEXITCODE) { throw 'No se pudo registrar el entorno OCR.' }
        Copy-Item -LiteralPath 'extensions/math-ocr/SOURCES.txt' -Destination $folder -Force
        & $Python 'extensions/math-ocr/licenses.py' (Join-Path $folder 'LICENSES')
        if ($LASTEXITCODE) { throw 'No se pudo registrar las licencias del OCR.' }
    }
    $manifest = [IO.File]::ReadAllText((Join-Path $root "extensions/$name/extension.toml"))
    $manifest.Replace('@TARGET@', $Target).Replace('@EXE@', $exe) | Set-Content -LiteralPath (Join-Path $folder 'extension.toml') -Encoding utf8NoBOM
    Copy-Item -LiteralPath 'LICENSE' -Destination (Join-Path $folder 'LICENSE') -Force
    $archive = Join-Path $packageDir "$name-$Target.zip"
    # ZipFile follows file symlinks in PyInstaller's Linux distribution and stores the file data.
    & $Python -c 'import pathlib,sys,zipfile; root=pathlib.Path(sys.argv[1]); files=sorted(p for p in root.rglob("*") if p.is_file()); archive=zipfile.ZipFile(sys.argv[2],"w",compression=zipfile.ZIP_DEFLATED); [archive.write(p,p.relative_to(root).as_posix()) for p in files]; archive.close()' $folder $archive
    if ($LASTEXITCODE) { throw 'No se pudo crear el ZIP.' }
    Write-Output $archive
} finally { Pop-Location }
