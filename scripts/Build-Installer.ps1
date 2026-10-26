[CmdletBinding()]
param(
    [ValidateSet('x64','x86')][string]$Architecture = 'x64',
    [string]$Destination = 'artifacts/packages',
    [string]$PortableDirectory,
    [switch]$SkipBuild,
    [ValidatePattern('^$|^[A-Fa-f0-9]{40}$')][string]$CertificateThumbprint,
    [switch]$RequireSignature
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
function WorkspacePath([string]$relative) {
    $full = if ([IO.Path]::IsPathRooted($relative)) { [IO.Path]::GetFullPath($relative) } else { [IO.Path]::GetFullPath((Join-Path $workspace $relative)) }
    if (!$full.StartsWith($workspace + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'La ruta debe estar dentro del proyecto.' }
    return $full
}
function NsisPath([string]$value) {
    if ($value -match '["\r\n]') { throw 'Ruta incompatible con NSIS.' }
    return $value.Replace('$','$$')
}
if ($RequireSignature -and !$CertificateThumbprint) { throw 'Se requiere un certificado de firma de codigo confiable. No se fabricaran certificados de confianza.' }
$destinationPath = WorkspacePath $Destination
New-Item -ItemType Directory -Force -Path $destinationPath | Out-Null
$target = if ($Architecture -eq 'x86') { 'i686-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
Push-Location -LiteralPath $workspace
try {
    $metadata = & cargo metadata --locked --offline --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'No se pudo leer Cargo metadata.' }
    $packages = @($metadata.packages | Where-Object { $_.name -eq 'crabpdf' })
    if ($packages.Count -ne 1) { throw 'Se esperaba exactamente el paquete crabpdf.' }
    $version = $packages[0].version
    if (!$SkipBuild) {
        & cargo build --locked --offline -p crabpdf --bin crabpdf --release --target $target
        if ($LASTEXITCODE -ne 0) { throw 'Fallo de compilacion.' }
    }
    $binary = Join-Path $workspace "target/$target/release/crabpdf.exe"
    if (!$PortableDirectory) {
        & (Join-Path $PSScriptRoot 'Pack-Portable.ps1') -Architecture $Architecture -Destination $Destination -SkipBuild
        $PortableDirectory = Join-Path $destinationPath "crabpdf-$version-win-$Architecture"
    }
    $portable = WorkspacePath $PortableDirectory
    $manifest = Get-Content -Raw -LiteralPath (Join-Path $portable 'manifest.json') | ConvertFrom-Json
    if ($manifest.version -ne $version -or $manifest.target -ne $target) { throw 'Version o arquitectura del paquete incorrectas.' }
    if ((Get-FileHash -LiteralPath $binary).Hash -ne (Get-FileHash -LiteralPath (Join-Path $portable 'crabpdf.exe')).Hash) { throw 'El paquete no contiene el ejecutable actual.' }
    $work = Join-Path $destinationPath "installer-build-$Architecture"
    $payload = Join-Path $work 'payload'
    New-Item -ItemType Directory -Force -Path $payload | Out-Null
    $paths = [Collections.Generic.List[string]]::new()
    $bytesTotal = 0L
    foreach ($file in $manifest.files) {
        $relative = [string]$file.path
        if ($relative -match '(^[/\\]|(^|[/\\])\.\.([/\\]|$)|[:*?"\r\n])' -or $paths.Contains($relative)) { throw 'Ruta de manifiesto insegura o duplicada.' }
        $source = [IO.Path]::GetFullPath((Join-Path $portable $relative))
        if (!$source.StartsWith($portable + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Ruta fuera del paquete.' }
        $item = Get-Item -LiteralPath $source
        if ($item.Length -ne $file.bytes -or (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $file.sha256) { throw "Integridad incorrecta: $relative" }
        $copy = Join-Path $payload $relative
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $copy) | Out-Null
        Copy-Item -LiteralPath $source -Destination $copy -Force
        $paths.Add($relative)
        $bytesTotal += $item.Length
    }
    foreach ($required in @('crabpdf.exe','pdfium.dll','LICENSE','format-tools/provenance.json')) {
        if (!$paths.Contains($required)) { throw "Falta $required en el manifiesto." }
    }
    $compiler = & (Join-Path $PSScriptRoot 'Get-NSIS.ps1')
    $notices = Join-Path $payload 'installer-licenses'
    New-Item -ItemType Directory -Force -Path $notices | Out-Null
    Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $compiler) 'COPYING') -Destination (Join-Path $notices 'NSIS-COPYING.txt') -Force
    Copy-Item -LiteralPath (Join-Path $workspace 'installer/windows/NSIS-NOTICE.txt') -Destination (Join-Path $notices 'NSIS-NOTICE.txt') -Force
    foreach ($notice in @('installer-licenses/NSIS-COPYING.txt','installer-licenses/NSIS-NOTICE.txt')) {
        $paths.Add($notice)
        $bytesTotal += (Get-Item -LiteralPath (Join-Path $payload $notice)).Length
    }
    $signScript = Join-Path $PSScriptRoot 'Sign-Windows.ps1'
    if ($CertificateThumbprint) {
        & $signScript -Path (Join-Path $payload 'crabpdf.exe') -CertificateThumbprint $CertificateThumbprint
    }
    $payloadFiles = @(foreach ($relative in $paths) {
        $file = Get-Item -LiteralPath (Join-Path $payload $relative)
        @{ path=$relative; bytes=$file.Length; sha256=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
    })
    $utf8 = [Text.UTF8Encoding]::new($false)
    $payloadManifest = @{ version=$version; target=$target; files=$payloadFiles } | ConvertTo-Json -Depth 6
    [IO.File]::WriteAllText((Join-Path $payload 'manifest.json'), $payloadManifest, $utf8)
    $paths.Add('manifest.json')
    $installLines = [Collections.Generic.List[string]]::new()
    $uninstallLines = [Collections.Generic.List[string]]::new()
    $directories = @{}
    foreach ($relative in $paths) {
        $windowsPath = $relative.Replace('/','\')
        $directory = [IO.Path]::GetDirectoryName($windowsPath)
        $targetPath = if ($directory) { '$INSTDIR\' + (NsisPath $directory) } else { '$INSTDIR' }
        $installLines.Add('SetOutPath "' + $targetPath + '"')
        $installLines.Add('File "' + (NsisPath (Join-Path $payload $windowsPath)) + '"')
        $uninstallLines.Add('Delete "$INSTDIR\' + (NsisPath $windowsPath) + '"')
        while ($directory) {
            $directories[$directory] = $true
            $directory = [IO.Path]::GetDirectoryName($directory)
        }
    }
    foreach ($directory in ($directories.Keys | Sort-Object Length -Descending)) {
        $uninstallLines.Add('RMDir "$INSTDIR\' + (NsisPath $directory) + '"')
    }
    [IO.File]::WriteAllLines((Join-Path $work 'install-files.nsh'), [string[]]$installLines, $utf8)
    [IO.File]::WriteAllLines((Join-Path $work 'uninstall-files.nsh'), [string[]]$uninstallLines, $utf8)
    $buildDirectory = Join-Path $workspace "target/$target/release/build"
    $branding = Get-ChildItem -LiteralPath $buildDirectory -Directory -Filter 'crabpdf-*' |
        ForEach-Object { Join-Path $_.FullName 'out' } |
        Where-Object { Test-Path -LiteralPath (Join-Path $_ 'installer-sidebar.bmp') } |
        Sort-Object { (Get-Item -LiteralPath (Join-Path $_ 'installer-sidebar.bmp')).LastWriteTimeUtc } -Descending |
        Select-Object -First 1
    if (!$branding) { throw 'Faltan los recursos del instalador. Compila la aplicacion actual sin -SkipBuild.' }
    $output = Join-Path $destinationPath "crabpdf-$version-win-$Architecture-setup.exe"
    $label = if ($CertificateThumbprint) { 'Authenticode' } else { 'Sin firma digital / Unsigned' }
    $arguments = @('/INPUTCHARSET','UTF8','/V2','/WX',
        "/DARCH=$Architecture", "/DVERSION=$version", "/DOUTPUT=$output",
        "/DBRANDING=$branding", "/DINCLUDE_DIR=$work",
        "/DLICENSE=$(Join-Path $workspace 'LICENSE')",
        "/DINSTALLED_KIB=$([math]::Ceiling($bytesTotal / 1024))", "/DSIGNATURE_LABEL=$label")
    if ($CertificateThumbprint) {
        $arguments += @("/DSIGN_THUMBPRINT=$CertificateThumbprint", "/DSIGN_SCRIPT=$signScript", "/DPOWERSHELL=$PSHOME\powershell.exe")
    }
    & $compiler @arguments (Join-Path $workspace 'installer/windows/crabpdf.nsi')
    if ($LASTEXITCODE -ne 0) { throw 'NSIS no pudo construir el instalador.' }
    if ($CertificateThumbprint) { & $signScript -Path $output -CertificateThumbprint $CertificateThumbprint }
    $signature = Get-AuthenticodeSignature -LiteralPath $output
    if ($RequireSignature -and $signature.Status -ne 'Valid') { throw 'La firma del instalador no es valida.' }
    $report = @{
        version=$version; target=$target; architecture=$Architecture
        source_commit=(& git rev-parse HEAD).Trim()
        source_dirty=[bool](& git status --porcelain)
        compiler='NSIS 3.12'
        compiler_archive_sha256='56581f90db321581c5381193d796fffcf2d24b2f8fed2160a6c6a3baa67f2c4f'
        installer=(Split-Path -Leaf $output)
        bytes=(Get-Item -LiteralPath $output).Length
        sha256=(Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
        payload_files=$paths.Count
        authenticode_status=$signature.Status.ToString()
        signer=if ($signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
        security_certification=$false
        requires_admin=$false
    }
    [IO.File]::WriteAllText("$output.json", ($report | ConvertTo-Json -Depth 5), $utf8)
    [IO.File]::WriteAllText("$output.sha256", ($report.sha256 + '  ' + (Split-Path -Leaf $output) + [Environment]::NewLine), [Text.Encoding]::ASCII)
    Write-Output "Instalador: $output ($($report.bytes) bytes, $($report.authenticode_status))"
} finally { Pop-Location }
