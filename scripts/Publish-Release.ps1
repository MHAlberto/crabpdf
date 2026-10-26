[CmdletBinding()]
param([string]$Version = '0.3.6', [switch]$RequireSignedInstaller)
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$repo = 'MHAlberto/crabpdf'
$tag = "v$Version"
$notesPath = Join-Path $workspace "docs/releases/$tag.md"
$assets = @(
    "artifacts/packages/crabpdf-$Version-win-x64.zip",
    "artifacts/packages/crabpdf-$Version-win-x86.zip",
    "artifacts/packages/crabpdf-$Version-win-x64-setup.exe",
    "artifacts/packages/crabpdf-$Version-win-x64-setup.exe.json",
    "artifacts/packages/crabpdf-$Version-win-x86-setup.exe",
    "artifacts/packages/crabpdf-$Version-win-x86-setup.exe.json",
    "artifacts/packages/crabpdf-$Version-ubuntu24.04-amd64.deb",
    "artifacts/packages/crabpdf-$Version-linux-x64.tar.gz",
    'artifacts/packages/crabpdf-0.2.0-format-tools-sources.zip'
)
Push-Location -LiteralPath $workspace
try {
    $origin = (git remote get-url origin).Trim().TrimEnd('/') -replace '\.git$', ''
    if ((git status --porcelain) -or $origin -ne "https://github.com/$repo") {
        throw 'El árbol debe estar limpio y origin debe apuntar al repositorio esperado.'
    }
    $releaseCommit = (git rev-list -n 1 $tag).Trim()
    if ($LASTEXITCODE -ne 0) { throw "No existe la etiqueta $tag." }
    # A later correction to this upload helper may publish the existing tag, but
    # the application and build/packaging inputs must still match that tag.
    $sourceChanges = @(git diff --name-only $releaseCommit HEAD -- Cargo.toml Cargo.lock rust-toolchain.toml build.rs crabpdf-logo.svg src scripts/Build-LinuxFormatTools.sh scripts/Pack-Linux.sh scripts/Export-RustLicenses.py scripts/Pack-Portable.ps1 scripts/Export-Licenses.ps1 scripts/Build-Installer.ps1 scripts/Get-NSIS.ps1 scripts/Sign-Windows.ps1 scripts/Get-FormatTools.ps1 installer)
    if ($LASTEXITCODE -ne 0 -or $sourceChanges.Count -gt 0) { throw "Las fuentes o entradas de empaquetado difieren de $tag." }
    if ((git ls-remote --tags origin "refs/tags/$tag") -notmatch 'refs/tags/') {
        throw "La etiqueta $tag aún no está en GitHub."
    }
    if (-not (Test-Path -LiteralPath $notesPath)) { throw "Faltan las notas $notesPath" }
    foreach ($relative in $assets) {
        if (-not (Test-Path -LiteralPath (Join-Path $workspace $relative))) {
            throw "Falta $relative"
        }
    }
    foreach ($architecture in @('x64','x86')) {
        $installer = Join-Path $workspace "artifacts/packages/crabpdf-$Version-win-$architecture-setup.exe"
        $report = Get-Content -Raw -LiteralPath "$installer.json" | ConvertFrom-Json
        if ($report.version -ne $Version -or $report.architecture -ne $architecture -or
            (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash -ne $report.sha256) { throw 'El instalador y su informe no coinciden.' }
        if ($RequireSignedInstaller -and (Get-AuthenticodeSignature -LiteralPath $installer).Status -ne 'Valid') {
            throw 'La publicacion requiere instaladores con firma confiable.'
        }
        if ($report.authenticode_status -ne 'Valid') {
            if ([IO.File]::ReadAllText($notesPath) -notmatch '(?i)sin firma|unsigned') { throw 'Las notas deben indicar que el instalador no lleva firma.' }
        }
    }
    $checksumLines = foreach ($relative in $assets) {
        $file = Get-Item -LiteralPath (Join-Path $workspace $relative)
        $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $($file.Name)"
    }
    $checksumRelative = "artifacts/packages/SHA256SUMS-$tag.txt"
    [IO.File]::WriteAllLines((Join-Path $workspace $checksumRelative), [string[]]$checksumLines, [Text.Encoding]::ASCII)
    $assets += $checksumRelative

    $env:GCM_INTERACTIVE = 'Never'
    $env:GIT_TERMINAL_PROMPT = '0'
    $credentialInput = 'protocol=https' + [char]10 + 'host=github.com' + [char]10 + [char]10
    $credential = @($credentialInput | git credential fill)
    if ($LASTEXITCODE -ne 0) { throw 'Git Credential Manager no entregó credenciales.' }
    $passwordLine = $credential | Where-Object { $_ -like 'password=*' } | Select-Object -First 1
    if (-not $passwordLine) { throw 'No hay credencial GitHub disponible.' }
    $token = $passwordLine.Substring(9)
    $headers = @{
        Authorization = "Bearer $token"
        Accept = 'application/vnd.github+json'
        'X-GitHub-Api-Version' = '2022-11-28'
        'User-Agent' = 'CrabPDF-release-script'
    }
    $api = "https://api.github.com/repos/$repo"
    $existing = @(Invoke-RestMethod -Uri "$api/releases?per_page=100" -Headers $headers) |
        Where-Object { $_.tag_name -eq $tag } | Select-Object -First 1
    if ($existing) {
        $release = $existing
        if (-not $release.draft) { throw "La publicación $tag ya es pública; no se modificará." }
    } else {
        $body = @{
            tag_name = $tag
            target_commitish = $releaseCommit
            name = "CrabPDF $Version (desarrollo)"
            body = [IO.File]::ReadAllText($notesPath, [Text.Encoding]::UTF8)
            draft = $true
            prerelease = $true
            generate_release_notes = $false
        } | ConvertTo-Json -Depth 4
        $release = Invoke-RestMethod -Uri "$api/releases" -Method Post -Headers $headers -Body ([Text.Encoding]::UTF8.GetBytes($body)) -ContentType 'application/json'
        Write-Output "Borrador creado: $($release.html_url)"
    }
    $uploadBase = $release.upload_url -replace '\{\?name,label\}$', ''
    foreach ($relative in $assets) {
        $file = Get-Item -LiteralPath (Join-Path $workspace $relative)
        $digest = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        $existingAsset = @($release.assets | Where-Object { $_.name -eq $file.Name }) | Select-Object -First 1
        if ($existingAsset) {
            if ($existingAsset.digest -ne "sha256:$digest") {
                throw "El asset existente $($file.Name) tiene un hash distinto."
            }
            Write-Output "Ya verificado: $($file.Name)"
            continue
        }
        $type = if ($file.Extension -eq '.zip') { 'application/zip' }
            elseif ($file.Extension -eq '.deb') { 'application/vnd.debian.binary-package' }
            elseif ($file.Extension -eq '.txt') { 'text/plain' }
            elseif ($file.Extension -eq '.json') { 'application/json' }
            elseif ($file.Extension -eq '.exe') { 'application/vnd.microsoft.portable-executable' }
            else { 'application/gzip' }
        $uploaded = Invoke-RestMethod -Uri "$uploadBase`?name=$([uri]::EscapeDataString($file.Name))" -Method Post -Headers $headers -InFile $file.FullName -ContentType $type -TimeoutSec 1800
        if ($uploaded.state -ne 'uploaded' -or $uploaded.size -ne $file.Length -or
            $uploaded.digest -ne "sha256:$digest") {
            throw "GitHub no confirmó el hash, estado y tamaño de $($file.Name)."
        }
        Write-Output "Subido: $($uploaded.name), $($uploaded.size) bytes, $($uploaded.digest)"
        $release.assets += $uploaded
    }
    $published = Invoke-RestMethod -Uri "$api/releases/$($release.id)" -Method Patch -Headers $headers -Body '{"draft":false,"prerelease":true}' -ContentType 'application/json'
    Write-Output "Publicación: $($published.html_url)"
} finally {
    Pop-Location
}
