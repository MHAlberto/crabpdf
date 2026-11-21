[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^(x86_64|i686|aarch64)-(pc-windows-msvc|unknown-linux-gnu|apple-darwin)$')][string]$Target,
    [string]$Python = 'python',
    [switch]$Ocr,
    [switch]$Statistics,
    [switch]$ReadingGuide
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
    $name = if ($Statistics) { 'reading-stats' } elseif ($ReadingGuide) { 'reading-guide' } else { 'pomodoro' }
    $folder = Join-Path $root "artifacts/extension-build/$name-$Target"
    New-Item -ItemType Directory -Force $folder | Out-Null
    $manifest = [IO.File]::ReadAllText((Join-Path $root "extensions/$name/extension.toml"))
    $manifest.Replace('@TARGET@', $Target).Replace('@EXE@', '') | Set-Content -LiteralPath (Join-Path $folder 'extension.toml') -Encoding utf8NoBOM
    Copy-Item -LiteralPath 'LICENSE' -Destination (Join-Path $folder 'LICENSE') -Force
    $archive = Join-Path $packageDir "$name-$Target.zip"
    $stream = [IO.File]::Create($archive)
    try {
        $zip = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create, $true)
        try {
            foreach ($file in @('extension.toml', 'LICENSE')) {
                [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                    $zip, (Join-Path $folder $file), $file, [IO.Compression.CompressionLevel]::Optimal
                ) | Out-Null
            }
        } finally { $zip.Dispose() }
    } finally { $stream.Dispose() }
    Write-Output $archive
} finally { Pop-Location }
