[CmdletBinding()]
param([string]$Destination = 'docs/dependencies.csv', [string]$LicenseDirectory)
$ErrorActionPreference = 'Stop'
$json = & cargo metadata --locked --offline --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'No se pudo leer cargo metadata. Ejecuta cargo fetch primero.' }
$metadata = $json | ConvertFrom-Json
$packages = $metadata.packages | Where-Object { $_.name -ne 'crabpdf' } | Sort-Object name, version
$parent = Split-Path -Parent $Destination
if ($parent) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
$packages | Select-Object name, version, license, repository |
    Export-Csv -LiteralPath $Destination -NoTypeInformation -Encoding UTF8
Write-Output "Inventario generado: $($packages.Count) dependencias (todos los targets, desarrollo y compilacion)."
if ($LicenseDirectory) {
    $root = [IO.Path]::GetFullPath($LicenseDirectory)
    New-Item -ItemType Directory -Path $root -Force | Out-Null
    $records = @()
    foreach ($package in $packages) {
        $source = Split-Path -Parent $package.manifest_path
        $folder = Join-Path $root "$($package.name)-$($package.version)"
        $texts = @(Get-ChildItem -LiteralPath $source -Recurse -File | Where-Object {
            $_.Name -match 'LICENSE|LICENCE|COPYING|NOTICE|(^|[-_.])(OFL|UFL)([-_.]|$)' -or
            ($package.name -eq 'epaint_default_fonts' -and $_.Extension -eq '.txt') -or
            ($package.license_file -and $_.FullName -eq (Join-Path $source $package.license_file))
        })
        foreach ($file in $texts) {
            $relative = $file.FullName.Substring($source.Length + 1)
            $output = [IO.Path]::GetFullPath((Join-Path $folder $relative))
            if (-not $output.StartsWith($root + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Ruta de licencia fuera del destino.' }
            New-Item -ItemType Directory -Path (Split-Path -Parent $output) -Force | Out-Null
            Copy-Item -LiteralPath $file.FullName -Destination $output -Force
        }
        $records += @{ name=$package.name; version=$package.version; license=$package.license; repository=$package.repository; text_count=$texts.Count }
    }
    @{ scope='Cargo.lock: all targets, build and development dependencies; a superset of each executable'; packages=$records } |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'index.json') -Encoding UTF8
    Write-Output "Textos originales conservados en $root. Revisar entradas sin texto antes de distribuir."
}
