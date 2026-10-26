[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Installer,
    [ValidateSet('x64','x86')][string]$Architecture = 'x64',
    [string]$Destination = 'artifacts/installer-tests'
)
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
function InWorkspace([string]$path) {
    $full = if ([IO.Path]::IsPathRooted($path)) { [IO.Path]::GetFullPath($path) } else { [IO.Path]::GetFullPath((Join-Path $workspace $path)) }
    if (!$full.StartsWith($workspace + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'La prueba solo utiliza rutas dentro del proyecto.' }
    return $full
}
$installerPath = InWorkspace $Installer
$destinationPath = InWorkspace $Destination
$root = Join-Path $destinationPath ("$Architecture-" + [guid]::NewGuid().ToString('N'))
$install = Join-Path $root 'Installed Reader'
$key = "HKCU:\Software\CrabPDF\Installations\$Architecture"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\CrabPDF-$Architecture"
if ((Test-Path $key) -or (Test-Path $uninstallKey)) { throw 'Ya existe una instalacion registrada. La prueba no la modificara.' }
New-Item -ItemType Directory -Force -Path $root | Out-Null
function RunInstaller([string]$exe,[string]$arguments,[int]$expected=0) {
    $process = Start-Process -FilePath $exe -ArgumentList $arguments -PassThru -WindowStyle Hidden
    if (!$process.WaitForExit(60000)) { throw 'El instalador excedio 60 segundos. No se terminaran procesos ajenos.' }
    if ($process.ExitCode -ne $expected) { throw "Salida inesperada: $($process.ExitCode), esperada $expected" }
}
function CheckPayload {
    $manifest = Get-Content -Raw -LiteralPath (Join-Path $install 'manifest.json') | ConvertFrom-Json
    foreach ($file in $manifest.files) {
        $path = Join-Path $install $file.path
        if ((Get-Item -LiteralPath $path).Length -ne $file.bytes -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $file.sha256) { throw "Archivo instalado incorrecto: $($file.path)" }
    }
    return @($manifest.files).Count
}
# An unrelated existing directory must be rejected, without altering its sentinel.
$unrelated = Join-Path $root 'Unrelated folder'
New-Item -ItemType Directory -Path $unrelated | Out-Null
[IO.File]::WriteAllText((Join-Path $unrelated 'keep.txt'), 'preserve')
RunInstaller $installerPath ("/S /NOINTEGRATION /D=$unrelated") 3
if ((Get-Content -Raw -LiteralPath (Join-Path $unrelated 'keep.txt')) -ne 'preserve' -or (Test-Path $key)) { throw 'La ruta ajena se modifico.' }
RunInstaller $installerPath ("/S /NOINTEGRATION /D=$install")
$count = CheckPayload
if ((Get-ItemProperty -LiteralPath $key).InstallLocation -ne $install) { throw 'Registro de instalacion incorrecto.' }
$signature = (Get-AuthenticodeSignature -LiteralPath (Join-Path $install 'uninstall.exe')).Status.ToString()
[IO.File]::WriteAllText((Join-Path $install 'my-document.txt'), 'preserve')
# Upgrade in place, preserving a file the application does not own.
RunInstaller $installerPath ("/S /NOINTEGRATION /D=$install")
CheckPayload | Out-Null
if ((Get-Content -Raw -LiteralPath (Join-Path $install 'my-document.txt')) -ne 'preserve') { throw 'La actualizacion modifico archivos ajenos.' }
# Keeping an open executable catches updates and uninstall while the app runs.
$lockedExe = [IO.File]::Open((Join-Path $install 'crabpdf.exe'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try { RunInstaller $installerPath ("/S /NOINTEGRATION /D=$install") 4 } finally { $lockedExe.Dispose() }
CheckPayload | Out-Null
RunInstaller (Join-Path $install 'uninstall.exe') '/S'
$deadline = (Get-Date).AddSeconds(30)
while (((Test-Path $key) -or (Test-Path -LiteralPath (Join-Path $install 'uninstall.exe'))) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 100 }
if ((Test-Path $key) -or (Test-Path $uninstallKey) -or (Test-Path -LiteralPath (Join-Path $install 'crabpdf.exe'))) { throw 'Desinstalacion incompleta.' }
if ((Get-Content -Raw -LiteralPath (Join-Path $install 'my-document.txt')) -ne 'preserve') { throw 'La desinstalacion borro archivos ajenos.' }
$report = @{
    architecture=$Architecture; payload_files=$count
    installer_sha256=(Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
    uninstaller_authenticode=$signature
    installation=$true; manifest_verified=$true; upgrade=$true
    rejects_unrelated_directory=$true; rejects_locked_executable=$true
    uninstall=$true; preserves_unrelated_files=$true; registry_removed=$true
    gui_reviewed=$false
}
$report | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'validation.json') -Encoding UTF8
Write-Output "Verificacion aprobada: $root"
