[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$directory = Join-Path $workspace 'native/installer-tools'
$archive = Join-Path $directory 'nsis-3.12.zip'
$expected = '56581f90db321581c5381193d796fffcf2d24b2f8fed2160a6c6a3baa67f2c4f'
# Published upstream ZIP, checksum and size independently recorded by MacPorts:
# https://raw.githubusercontent.com/macports/macports-ports/master/devel/nsis/Portfile
New-Item -ItemType Directory -Force -Path $directory | Out-Null
if (!(Test-Path -LiteralPath $archive)) {
    $downloaded = $false
    foreach ($url in @(
        'https://downloads.sourceforge.net/project/nsis/NSIS%203/3.12/nsis-3.12.zip',
        'https://distfiles.macports.org/nsis/nsis-3.12.zip'
    )) {
        try {
            Invoke-WebRequest -Uri $url -OutFile $archive -UseBasicParsing -TimeoutSec 60
            if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) { throw 'Checksum NSIS incorrecto.' }
            $downloaded = $true
            break
        } catch { Write-Warning $_.Exception.Message }
    }
    if (!$downloaded) { throw 'No se pudo descargar el compilador NSIS verificado.' }
}
if ((Get-Item -LiteralPath $archive).Length -ne 2362938 -or (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
    throw 'El ZIP de NSIS no coincide con la version verificada. No se ejecutara.'
}
# Always restore tools from the verified archive; never trust an old extracted executable.
Expand-Archive -LiteralPath $archive -DestinationPath $directory -Force
$compiler = Join-Path $directory 'nsis-3.12/makensis.exe'
$version = & $compiler /VERSION
if ($LASTEXITCODE -ne 0 -or $version -ne 'v3.12') { throw 'Version de NSIS inesperada.' }
Write-Output $compiler
