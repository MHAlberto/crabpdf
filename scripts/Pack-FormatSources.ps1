[CmdletBinding()]
param([string]$Destination='artifacts/packages')
$ErrorActionPreference='Stop'
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$destinationPath=[IO.Path]::GetFullPath((Join-Path $workspace $Destination))
if(-not $destinationPath.StartsWith($workspace+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'El destino debe quedar dentro del proyecto.'}
$metadata = & cargo metadata --locked --offline --no-deps --format-version 1 | ConvertFrom-Json
$packages = @($metadata.packages | Where-Object name -eq 'crabpdf')
if ($LASTEXITCODE -ne 0 -or $packages.Count -ne 1) { throw 'No se encontro exactamente el paquete crabpdf.' }
$version = $packages[0].version
$folder=Join-Path $destinationPath "crabpdf-$version-format-tools-sources"
New-Item -ItemType Directory -Path $folder -Force | Out-Null
$manifest=Get-Content -Raw -LiteralPath (Join-Path $workspace 'native/format-tools/provenance.json') | ConvertFrom-Json
foreach($name in $manifest.sources){Copy-Item -LiteralPath (Join-Path $workspace "native/format-downloads/$name") -Destination $folder -Force}
foreach($name in @('Get-FormatTools.ps1','FormatToolsStaticCRT.props','Build-LinuxFormatTools.sh')){Copy-Item -LiteralPath (Join-Path $workspace "scripts/$name") -Destination $folder -Force}
Copy-Item -LiteralPath (Join-Path $workspace 'docs/THIRD_PARTY_NOTICES.md') -Destination $folder -Force
Copy-Item -LiteralPath (Join-Path $workspace 'native/format-tools/provenance.json') -Destination $folder -Force
@"
Fuentes de los decodificadores de CrabPDF $version.
Conservar este archivo junto a los paquetes Windows al compartirlos.
Los archivos tar.gz/tar.xz contienen las fuentes originales completas.
Get-FormatTools.ps1 fija URLs y hashes y muestra las opciones de compilacion.
MuPDF: Win32 Release, VS 2022/v143, CRT estatico mediante FormatToolsStaticCRT.props.
libmobi: CMake, Win32, TOOLS_STATIC=ON, USE_ZLIB=OFF, USE_LIBXML2=OFF,
USE_ENCRYPTION=OFF, CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded.
Parche CMake de libmobi: enlazar tools/win32/getopt.c al target common en MSVC.
DjVuLibre/7-Zip: programas sin modificar de sus distribuciones oficiales.
JPEG/TIFF/zlib: fuentes exactas de las DLL auxiliares de DjVuLibre.
Las DLL Microsoft no tienen fuente incluida; proceden del SDK redistribuible.
La aplicacion permite sustituir las herramientas por otra compilacion compatible.
Build-LinuxFormatTools.sh reproduce las compilaciones Linux de MuPDF y libmobi.
El codigo Rust y el instalador se publican en https://github.com/MHAlberto/crabpdf
La etiqueta v$version identifica la entrega correspondiente.
Los avisos y fuentes de NSIS se identifican en installer-licenses/ dentro de la instalacion Windows.
"@ | Set-Content -LiteralPath (Join-Path $folder 'LEEME.txt') -Encoding UTF8
$records=Get-ChildItem -LiteralPath $folder -File | Where-Object Name -ne 'manifest.json' | ForEach-Object {@{path=$_.Name;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant()}}
@{version=$version;files=@($records)} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $folder 'manifest.json') -Encoding UTF8
# ZIP paths use '/' so the same source bundle extracts correctly on Linux.
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$stream = [IO.File]::Create("$folder.zip")
$zip = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create, $false)
try {
    foreach ($file in (Get-ChildItem -LiteralPath $folder -File | Sort-Object Name)) {
        $entryName = [IO.Path]::GetFileName($folder) + '/' + $file.Name
        [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $file.FullName, $entryName, [IO.Compression.CompressionLevel]::Optimal) | Out-Null
    }
} finally { $zip.Dispose(); $stream.Dispose() }
Write-Output "Fuentes: $folder.zip"
