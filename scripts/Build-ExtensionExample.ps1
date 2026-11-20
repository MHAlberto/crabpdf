
[CmdletBinding()]
param(
    [string]$Target = '',
    [string]$Destination = 'artifacts/extensions/selection-info',
    [switch]$Archive
)
$ErrorActionPreference='Stop'
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location -LiteralPath $workspace
try {
    if (!$Target) {
        $hostLine=(& rustc -vV | Select-String '^host: ').Line
        $Target=$hostLine.Substring(6).Trim()
    }
    if ($Target -notmatch '^(x86_64|i686|aarch64)-(pc-windows-msvc|unknown-linux-gnu|apple-darwin)$') { throw 'Plataforma de ejemplo incompatible.' }
    $folder=[IO.Path]::GetFullPath((Join-Path $workspace $Destination))
    if (!$folder.StartsWith($workspace+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'La salida debe quedar dentro del proyecto.' }
    cargo build --locked --offline --release --target $Target -p crabpdf-extension-api --example selection_info
    if ($LASTEXITCODE) { throw 'Falló la compilación de la extensión de ejemplo.' }
    New-Item -ItemType Directory -Force -Path $folder | Out-Null
    $exe=if ($Target -like '*windows*') { 'selection_info.exe' } else { 'selection_info' }
    Copy-Item -LiteralPath "target/$Target/release/examples/$exe" -Destination (Join-Path $folder $exe) -Force
    if ($env:OS -ne "Windows_NT") { chmod +x (Join-Path $folder $exe); if ($LASTEXITCODE) { throw 'No se pudo marcar el ejemplo como ejecutable.' } }
    Copy-Item -LiteralPath LICENSE -Destination (Join-Path $folder 'LICENSE') -Force
@"
api_version = 1
id = "org.crabpdf.selection-info"
name = "Información de la selección"
version = "0.1.2"
host_version = ">=0.5.0, <0.7.0"
target = "$Target"
executable = "$exe"
description = "Ejemplo Rust que informa la imagen y el texto recibidos."

[[commands]]
id = "inspect"
title = "Examinar el área seleccionada"
input = "region"
output = "text"
"@ | Set-Content -LiteralPath (Join-Path $folder 'extension.toml') -Encoding utf8NoBOM
    if ($Archive) {
        $packages = Join-Path $workspace 'artifacts/site-extensions'
        New-Item -ItemType Directory -Force -Path $packages | Out-Null
        $archivePath = Join-Path $packages "selection-info-$Target.zip"
        $stream = [IO.File]::Create($archivePath)
        try {
            $zip = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create, $true)
            try {
                foreach ($name in @('extension.toml', $exe, 'LICENSE')) {
                    [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                        $zip, (Join-Path $folder $name), $name, [IO.Compression.CompressionLevel]::Optimal
                    ) | Out-Null
                }
            } finally { $zip.Dispose() }
        } finally { $stream.Dispose() }
        Write-Output "ZIP del catálogo: $archivePath"
    }
    Write-Output "Paquete listo para instalar desde Extensiones: $folder"
} finally { Pop-Location }
