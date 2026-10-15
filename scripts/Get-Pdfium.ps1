[CmdletBinding()]
param(
    [ValidateSet('win', 'mac', 'linux')][string]$Platform = 'win',
    [ValidateSet('x64', 'x86', 'arm64')][string]$Architecture = 'x64',
    [string]$Destination = (Join-Path $PSScriptRoot '../native')
)

$ErrorActionPreference = 'Stop'
$build = '7881'
$assetName = "pdfium-$Platform-$Architecture.tgz"
$headers = @{ 'User-Agent' = 'Rust-Reader-setup'; 'Accept' = 'application/vnd.github+json' }
$release = Invoke-RestMethod -Uri "https://api.github.com/repos/bblanchon/pdfium-binaries/releases/tags/chromium%2F$build" -Headers $headers
$asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1
if (-not $asset) { throw "No se encontro $assetName en PDFium $build." }
if ($asset.digest -notmatch '^sha256:([a-fA-F0-9]{64})$') { throw 'GitHub no proporciono el SHA-256 del archivo; no se descargara sin verificacion.' }
$expectedHash = $Matches[1]

$destinationPath = [IO.Path]::GetFullPath($Destination)
New-Item -ItemType Directory -Path $destinationPath -Force | Out-Null
$archive = Join-Path $destinationPath $assetName
Write-Output "Descargando PDFium $build ($Platform/$Architecture)..."
Invoke-WebRequest -Uri $asset.browser_download_url -Headers $headers -OutFile $archive -UseBasicParsing
$actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash
if ($actualHash -ne $expectedHash) { throw "SHA-256 incorrecto para $archive." }

$bundle = Join-Path $destinationPath "pdfium-$build-$Platform-$Architecture"
New-Item -ItemType Directory -Path $bundle -Force | Out-Null
$entries = & tar -tzf $archive
if ($LASTEXITCODE -ne 0) { throw 'No se pudo inspeccionar el paquete.' }
foreach ($entry in $entries) {
    if ($entry -match '(^[/\\]|(^|[/\\])\.\.([/\\]|$)|^[A-Za-z]:)') { throw "Ruta no segura en el paquete: $entry" }
}
& tar -xzf $archive -C $bundle
if ($LASTEXITCODE -ne 0) { throw 'No se pudo extraer PDFium.' }
$libraryName = switch ($Platform) { 'win' { 'pdfium.dll' }; 'mac' { 'libpdfium.dylib' }; 'linux' { 'libpdfium.so' } }
$library = Get-ChildItem -LiteralPath $bundle -Recurse -File | Where-Object { $_.Name -eq $libraryName } | Select-Object -First 1
if (-not $library) { throw "El paquete no contiene $libraryName." }
Copy-Item -LiteralPath $library.FullName -Destination (Join-Path $destinationPath $libraryName) -Force
@{
    build = $build
    asset = $assetName
    url = $asset.browser_download_url
    archive_sha256 = $actualHash.ToLowerInvariant()
    license_bundle = $bundle
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $destinationPath 'pdfium-provenance.json') -Encoding UTF8
Write-Output "PDFium instalado en $destinationPath. Se conservaron las licencias en $bundle."
