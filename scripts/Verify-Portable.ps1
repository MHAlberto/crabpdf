[CmdletBinding()]
param([string]$PackageDirectory='artifacts/packages')
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$packages=Get-ChildItem -LiteralPath (Join-Path $workspace $PackageDirectory) -File | Where-Object { $_.Name -like 'crabpdf-*-win-*.zip' -or $_.Name -like 'crabpdf-*-format-tools-sources.zip' }
if (-not $packages) {throw 'No hay paquetes para comprobar.'}
foreach ($package in $packages) {
    $zip=[IO.Compression.ZipFile]::OpenRead($package.FullName)
    $hash=[Security.Cryptography.SHA256]::Create()
    try {
        $entries=@{}
        foreach ($entry in $zip.Entries) {
            $name=$entry.FullName.Replace('\','/')
            if ($entries.ContainsKey($name)) {throw "Entrada duplicada: $name"}
            $entries[$name]=$entry
        }
        $prefix=$package.BaseName+'/'
        $manifestEntry=$entries[$prefix+'manifest.json']
        if (-not $manifestEntry) {throw 'Falta el manifiesto.'}
        $reader=[IO.StreamReader]::new($manifestEntry.Open())
        try {$manifest=$reader.ReadToEnd() | ConvertFrom-Json} finally {$reader.Dispose()}
        foreach ($file in $manifest.files) {
            $entry=$entries[$prefix+$file.path]
            if (-not $entry -or $entry.Length -ne $file.bytes) {throw "Falta el archivo o su longitud es incorrecta: $($file.path)"}
            $stream=$entry.Open()
            try {$digest=[BitConverter]::ToString($hash.ComputeHash($stream)).Replace('-','').ToLowerInvariant()} finally {$stream.Dispose()}
            if ($digest -ne $file.sha256) {throw "SHA-256 incorrecto: $($file.path)"}
        }
        $currentVersion=(& cargo metadata --locked --offline --no-deps --format-version 1 | ConvertFrom-Json).packages[0].version
        if($manifest.target -and $manifest.version -eq $currentVersion){
            $exeHash=(Get-FileHash -LiteralPath (Join-Path $workspace "target/$($manifest.target)/release/crabpdf.exe") -Algorithm SHA256).Hash
            if ($exeHash -ne ($manifest.files | Where-Object path -eq 'crabpdf.exe').sha256) {throw 'El paquete no contiene el ultimo ejecutable compilado.'}
        }
        Write-Output "$($package.Name): $($manifest.files.Count) archivos verificados, $($package.Length) bytes, target $($manifest.target)."
    } finally {$hash.Dispose();$zip.Dispose()}
}
