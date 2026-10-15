[CmdletBinding()]
param(
    [ValidateSet('x64','x86')][string]$Architecture = 'x64',
    [string]$Destination = 'artifacts/packages',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$target = if ($Architecture -eq 'x86') { 'i686-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
$nativeDir = if ($Architecture -eq 'x86') { Join-Path $workspace 'native/win-x86' } else { Join-Path $workspace 'native' }
$destinationPath = [IO.Path]::GetFullPath((Join-Path $workspace $Destination))
if (-not $destinationPath.StartsWith($workspace + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {
    throw 'El destino del paquete debe estar dentro del proyecto.'
}
Push-Location -LiteralPath $workspace
try {
    if (-not $SkipBuild) {
        & cargo build --locked --offline --release --target $target
        if ($LASTEXITCODE -ne 0) { throw 'Fallo de compilacion.' }
    }
    $exe = Join-Path $workspace "target/$target/release/crabpdf.exe"
    $dll = Join-Path $nativeDir 'pdfium.dll'
    if (-not (Test-Path -LiteralPath $exe) -or -not (Test-Path -LiteralPath $dll)) { throw 'Falta el ejecutable o PDFium de esta arquitectura.' }
    $expectedMachine=if($Architecture -eq 'x86') {0x014c} else {0x8664}
    foreach ($binary in @($exe,$dll)) {
        $bytes=[IO.File]::ReadAllBytes($binary)
        if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {throw "Archivo PE invalido: $binary"}
        $header=[BitConverter]::ToInt32($bytes,0x3c)
        if ($header -lt 0 -or $header + 6 -gt $bytes.Length -or [BitConverter]::ToUInt32($bytes,$header) -ne 0x00004550 -or [BitConverter]::ToUInt16($bytes,$header + 4) -ne $expectedMachine) {throw "Arquitectura PE incorrecta: $binary"}
    }
    $provenance = Get-Content -Raw -LiteralPath (Join-Path $nativeDir 'pdfium-provenance.json') | ConvertFrom-Json
    $expectedAsset = "pdfium-win-$Architecture.tgz"
    if ($provenance.build -ne '7881' -or $provenance.asset -ne $expectedAsset) { throw 'PDFium no coincide con el target y build requeridos.' }
    $archive = Join-Path $nativeDir $expectedAsset
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $provenance.archive_sha256) { throw 'Se modifico el archivo de PDFium verificado.' }
    $bundle = Join-Path $nativeDir "pdfium-7881-win-$Architecture"
    $bundledDll = Join-Path $bundle 'bin/pdfium.dll'
    if ((Get-FileHash -LiteralPath $bundledDll -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $dll -Algorithm SHA256).Hash) { throw 'La DLL no coincide con el paquete verificado.' }
    $version = (& cargo metadata --locked --offline --no-deps --format-version 1 | ConvertFrom-Json).packages[0].version
    $package = Join-Path $destinationPath "crabpdf-$version-win-$Architecture"
    New-Item -ItemType Directory -Path $package -Force | Out-Null
    Copy-Item -LiteralPath $exe -Destination (Join-Path $package 'crabpdf.exe') -Force
    Copy-Item -LiteralPath $dll -Destination (Join-Path $package 'pdfium.dll') -Force
    $formatTools=Join-Path $workspace 'native/format-tools'
    $formatManifest=Get-Content -Raw -LiteralPath (Join-Path $formatTools 'provenance.json') | ConvertFrom-Json
    $formatDestination=Join-Path $package 'format-tools'
    New-Item -ItemType Directory -Path $formatDestination -Force | Out-Null
    foreach($file in $formatManifest.files) {
        $path=Join-Path $formatTools $file.path
        if((Get-FileHash -LiteralPath $path).Hash -ne $file.sha256){throw "Se modificó el decodificador $($file.path)."}
        if([IO.Path]::GetExtension($path) -in @('.exe','.dll')){
            $bytes=[IO.File]::ReadAllBytes($path);$offset=[BitConverter]::ToInt32($bytes,0x3c)
            if([BitConverter]::ToUInt16($bytes,$offset+4) -ne 0x014c){throw 'Los decodificadores compartidos deben ser x86 para funcionar en ambos Windows.'}
        }
        Copy-Item -LiteralPath $path -Destination $formatDestination -Force
    }
    Copy-Item -LiteralPath (Join-Path $formatTools 'provenance.json') -Destination $formatDestination -Force
    Copy-Item -LiteralPath (Join-Path $workspace 'docs/FORMATS_AND_ANNOTATIONS.md') -Destination $package -Force
    Copy-Item -LiteralPath (Join-Path $workspace 'LICENSE') -Destination (Join-Path $package 'LICENSE') -Force
    Copy-Item -LiteralPath (Join-Path $workspace 'docs/THIRD_PARTY_NOTICES.md') -Destination (Join-Path $package 'THIRD_PARTY_NOTICES.md') -Force
    Copy-Item -LiteralPath (Join-Path $workspace 'docs/dependencies.csv') -Destination (Join-Path $package 'dependencies.csv') -Force
    & (Join-Path $workspace 'scripts/Export-Licenses.ps1') -Destination (Join-Path $package 'dependencies.csv') -LicenseDirectory (Join-Path $package 'rust-licenses')
    Copy-Item -LiteralPath (Join-Path $workspace 'docs/licenses-upstream') -Destination $package -Recurse -Force
    $licenses = Join-Path $package 'pdfium-licenses'
    New-Item -ItemType Directory -Path $licenses -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $bundle 'LICENSE') -Destination (Join-Path $licenses 'LICENSE') -Force
    Copy-Item -LiteralPath (Join-Path $bundle 'licenses') -Destination $licenses -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $nativeDir 'pdfium-provenance.json') -Destination (Join-Path $licenses 'provenance.json') -Force
    @"
CrabPDF $version - Windows $Architecture
Ejecutar crabpdf.exe. Conservar pdfium.dll y la carpeta format-tools.
F11: pantalla completa sin barras; mover el puntero al borde superior muestra controles.
F11 o Escape: salir. F12: panel lateral. Flechas izquierda/derecha: paginas o desplazar con zoom.
PDF, EPUB, Kindle sin DRM, FB2, XPS, DjVu, CHM, comics e imagenes comunes.
Seleccionar palabras con doble clic o arrastrando. Marcas: resaltar, subrayar y elegir color.
Ctrl+Mayus+L resalta, Ctrl+Mayus+U subraya, Ctrl+Z deshace la ultima marca.
Las marcas se conservan localmente. Ctrl+S exporta una copia PDF con las anotaciones.
Version en desarrollo hacia la paridad de SumatraPDF; no es la version 1.0.
No incluye instalador ni firma. Las preferencias se guardan en AppData.
Para elegir su ubicacion, establecer RUST_READER_CONFIG antes de ejecutar.
"@ | Set-Content -LiteralPath (Join-Path $package 'LEEME.txt') -Encoding UTF8
    $files = Get-ChildItem -LiteralPath $package -Recurse -File | Where-Object { $_.FullName -ne (Join-Path $package 'manifest.json') } | ForEach-Object {
        @{ path = $_.FullName.Substring($package.Length + 1).Replace('\','/'); bytes=$_.Length; sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
    @{ version=$version; target=$target; files=@($files) } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $package 'manifest.json') -Encoding UTF8
    # Cargo archives may use Unix epoch timestamps, before ZIP's 1980 minimum.
    # Normalize the copied files only; keep every license's original bytes.
    Get-ChildItem -LiteralPath $package -Recurse -File | Where-Object { $_.LastWriteTime.Year -lt 1980 } | ForEach-Object { $_.LastWriteTime = [DateTime]'1980-01-02' }
    Compress-Archive -LiteralPath $package -DestinationPath "$package.zip" -Force
    Write-Output "Paquete: $package.zip"
} finally { Pop-Location }
