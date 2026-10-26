[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][ValidatePattern('^[A-Fa-f0-9]{40}$')][string]$CertificateThumbprint,
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    [string]$SignTool
)
$ErrorActionPreference = 'Stop'
$certificate = Get-Item -LiteralPath "Cert:/CurrentUser/My/$CertificateThumbprint"
if (!$certificate.HasPrivateKey -or $certificate.NotAfter -le (Get-Date) -or $certificate.NotBefore -gt (Get-Date)) { throw 'El certificado no tiene una clave privada utilizable o no esta vigente.' }
$codeSigning = $certificate.Extensions | Where-Object { $_.Oid.Value -eq '2.5.29.37' } |
    ForEach-Object { $_.EnhancedKeyUsages } | Where-Object { $_.Value -eq '1.3.6.1.5.5.7.3.3' }
if (!$codeSigning) { throw 'El certificado no permite firma de codigo.' }
if (!$SignTool) {
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) { $SignTool = $command.Source }
    else {
        $sdk = Join-Path "${env:ProgramFiles(x86)}" 'Windows Kits/10/bin'
        $SignTool = Get-ChildItem -LiteralPath $sdk -Directory | Sort-Object Name -Descending |
            ForEach-Object { Join-Path $_.FullName 'x64/signtool.exe' } |
            Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    }
}
if (!$SignTool) { throw 'Falta SignTool del Windows SDK.' }
& $SignTool sign /sha1 $CertificateThumbprint /s My /fd SHA256 /tr $TimestampUrl /td SHA256 /d CrabPDF $Path
if ($LASTEXITCODE -ne 0) { throw 'No se pudo firmar el ejecutable.' }
& $SignTool verify /pa /all $Path
if ($LASTEXITCODE -ne 0) { throw 'SignTool no valida la firma.' }
$signature = Get-AuthenticodeSignature -LiteralPath $Path
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Thumbprint -ne $CertificateThumbprint -or !$signature.TimeStamperCertificate) {
    throw 'La firma no es confiable, no corresponde al certificado elegido o no lleva sello de tiempo.'
}
