# Generates original, redistributable PDF fixtures without third-party tooling.
# The encrypted fixture uses PDF Standard Security revision 2 only for testing.
[CmdletBinding()]
param([string]$Destination = (Join-Path $PSScriptRoot '../examples'))
$ErrorActionPreference = 'Stop'
$ascii = [Text.Encoding]::ASCII
New-Item -ItemType Directory -Path $Destination -Force | Out-Null

function Get-Md5([byte[]]$Data) {
    $md5 = [Security.Cryptography.MD5]::Create()
    try { return ,($md5.ComputeHash($Data)) } finally { $md5.Dispose() }
}

function Invoke-Rc4([byte[]]$Key, [byte[]]$Data) {
    $s = 0..255
    $j = 0
    for ($i = 0; $i -lt 256; $i++) {
        $j = ($j + $s[$i] + $Key[$i % $Key.Length]) % 256
        $temp = $s[$i]; $s[$i] = $s[$j]; $s[$j] = $temp
    }
    $i = 0; $j = 0
    $output = New-Object byte[] $Data.Length
    for ($n = 0; $n -lt $Data.Length; $n++) {
        $i = ($i + 1) % 256; $j = ($j + $s[$i]) % 256
        $temp = $s[$i]; $s[$i] = $s[$j]; $s[$j] = $temp
        $output[$n] = $Data[$n] -bxor $s[($s[$i] + $s[$j]) % 256]
    }
    return ,$output
}

function ConvertTo-Hex([byte[]]$Data) { return ([BitConverter]::ToString($Data)).Replace('-', '') }

function Write-Fixture([string]$Name, [bool]$Encrypted, [string]$Variant = 'Plain') {
    $streams = @(
@'
q
0.965 0.97 0.98 rg 0 0 600 800 re f
1 1 1 rg 32 32 536 736 re f
0.21 0.36 0.61 rg 62 703 34 4 re f
BT /F1 11 Tf 0.42 0.46 0.52 rg 62 727 Td (RUST READER / READING NOTES) Tj ET
BT /F1 32 Tf 0.12 0.15 0.20 rg 62 644 Td (A little room to read.) Tj ET
BT /F1 13 Tf 0.42 0.46 0.52 rg 62 611 Td (An original PDF for testing the reader.) Tj ET
0.86 0.88 0.91 RG 0.7 w 62 577 m 538 577 l S
BT /F1 16 Tf 0.12 0.15 0.20 rg 62 535 Td (Make space for the page) Tj ET
BT /F1 12 Tf 0.30 0.34 0.40 rg 62 502 Td (Good reading begins with fewer distractions.) Tj 0 -23 Td (This page includes real text, vector paths and color.) Tj 0 -23 Td (Use the toolbar to adjust the width or change the zoom.) Tj ET
0.93 0.95 0.98 rg 62 299 476 117 re f
BT /F1 14 Tf 0.21 0.36 0.61 rg 82 381 Td (One page at a time.) Tj ET
BT /F1 12 Tf 0.30 0.34 0.40 rg 82 351 Td (Go forward to see a landscape page.) Tj 0 -21 Td (Its blue panel also verifies pixel color conversion.) Tj ET
BT /F1 10 Tf 0.42 0.46 0.52 rg 62 66 Td (RUST READER 0.1) Tj 418 0 Td (01) Tj ET
Q
'@,
@'
q
1 1 1 rg 0 0 800 500 re f
BT /F1 11 Tf 0.42 0.46 0.52 rg 60 443 Td (RUST READER / A DIFFERENT PERSPECTIVE) Tj ET
BT /F1 28 Tf 0.12 0.15 0.20 rg 60 385 Td (A wider view.) Tj ET
BT /F1 13 Tf 0.42 0.46 0.52 rg 60 349 Td (The reader adjusts each page to its own proportions.) Tj ET
0.20 0.40 0.80 rg 60 120 310 175 re f
BT /F1 22 Tf 1 1 1 rg 83 249 Td (Space to focus.) Tj ET
BT /F1 12 Tf 1 1 1 rg 83 208 Td (A real vector panel.) Tj 0 -23 Td (Blue stays blue at every zoom.) Tj ET
BT /F1 14 Tf 0.12 0.15 0.20 rg 408 265 Td (Read at your pace) Tj ET
BT /F1 12 Tf 0.30 0.34 0.40 rg 408 231 Td (Try zooming in and out.) Tj 0 -24 Td (Switch the interface to dark mode.) Tj 0 -24 Td (The document keeps its original colors.) Tj ET
BT /F1 10 Tf 0.42 0.46 0.52 rg 60 50 Td (RUST READER 0.1) Tj 662 0 Td (02) Tj ET
Q
'@
    )
    $objects = @{
        1 = $ascii.GetBytes('<< /Type /Catalog /Pages 2 0 R >>')
        2 = $ascii.GetBytes('<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>')
        3 = $ascii.GetBytes('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 7 0 R >> >> /Contents 5 0 R >>')
        4 = $ascii.GetBytes('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 800 500] /Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>')
        7 = $ascii.GetBytes('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')
    }
    $trailerExtra = ''
    $count = 7
    if ($Variant -eq 'Interactive') {
        $objects[1] = $ascii.GetBytes('<< /Type /Catalog /Pages 2 0 R /Outlines 8 0 R >>')
        $objects[3] = $ascii.GetBytes('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 7 0 R >> >> /Contents 5 0 R /Annots [11 0 R 12 0 R] >>')
        $objects[8] = $ascii.GetBytes('<< /Type /Outlines /First 9 0 R /Last 10 0 R /Count 2 >>')
        $objects[9] = $ascii.GetBytes('<< /Title (Reading notes) /Parent 8 0 R /Next 10 0 R /Dest [3 0 R /Fit] >>')
        $objects[10] = $ascii.GetBytes('<< /Title (A wider view) /Parent 8 0 R /Prev 9 0 R /Dest [4 0 R /Fit] >>')
        $objects[11] = $ascii.GetBytes('<< /Type /Annot /Subtype /Link /Rect [62 609 430 627] /Border [0 0 0] /A << /S /URI /URI (https://example.org/reader) >> >>')
        $objects[12] = $ascii.GetBytes('<< /Type /Annot /Subtype /Link /Rect [82 347 430 365] /Border [0 0 0] /Dest [4 0 R /Fit] >>')
        $objects[13] = $ascii.GetBytes('<< /Title (Rust Reader interactive fixture) /Author (Rust Reader project) >>')
        $count = 13
        $trailerExtra = ' /Info 13 0 R'
    }
    if ($Variant -eq 'Geometry') {
        $objects[3] = $ascii.GetBytes('<< /Type /Page /Parent 2 0 R /MediaBox [-20 -30 600 800] /CropBox [40 50 580 770] /Rotate 90 /Resources << /Font << /F1 7 0 R >> >> /Contents 5 0 R >>')
    }
    if ($Variant -eq 'Tall') {
        $objects[4] = $ascii.GetBytes('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 3000] /Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>')
    }
    if ($Encrypted) {
        [byte[]]$padding = @(0x28,0xbf,0x4e,0x5e,0x4e,0x75,0x8a,0x41,0x64,0x00,0x4e,0x56,0xff,0xfa,0x01,0x08,0x2e,0x2e,0x00,0xb6,0xd0,0x68,0x3e,0x80,0x2f,0x0c,0xa9,0xfe,0x64,0x53,0x69,0x7a)
        [byte[]]$user = ($ascii.GetBytes('reader-test') + $padding)[0..31]
        [byte[]]$owner = ($ascii.GetBytes('fixture-owner') + $padding)[0..31]
        [byte[]]$ownerKey = (Get-Md5 $owner)[0..4]
        [byte[]]$ownerEntry = Invoke-Rc4 $ownerKey $user
        [byte[]]$identifier = Get-Md5 $ascii.GetBytes('Rust Reader test fixture')
        [byte[]]$fileKey = (Get-Md5 ($user + $ownerEntry + [BitConverter]::GetBytes([int]-4) + $identifier))[0..4]
        [byte[]]$userEntry = Invoke-Rc4 $fileKey $padding
        $objects[8] = $ascii.GetBytes("<< /Filter /Standard /V 1 /R 2 /O <$(ConvertTo-Hex $ownerEntry)> /U <$(ConvertTo-Hex $userEntry)> /P -4 >>")
        $trailerExtra = " /Encrypt 8 0 R /ID [<$(ConvertTo-Hex $identifier)><$(ConvertTo-Hex $identifier)>]"
        $count = 8
    }
    for ($n = 0; $n -lt 2; $n++) {
        $objectNumber = 5 + $n
        [byte[]]$content = $ascii.GetBytes($streams[$n])
        if ($Encrypted) {
            [byte[]]$objectKey = (Get-Md5 ($fileKey + [byte[]]@($objectNumber, 0, 0, 0, 0)))[0..9]
            $content = Invoke-Rc4 $objectKey $content
        }
        $objects[$objectNumber] = $ascii.GetBytes("<< /Length $($content.Length) >>`nstream`n") + $content + $ascii.GetBytes("`nendstream")
    }
    $output = New-Object IO.MemoryStream
    try {
        $header = $ascii.GetBytes("%PDF-1.4`n"); $output.Write($header, 0, $header.Length)
        $offsets = @(0)
        for ($n = 1; $n -le $count; $n++) {
            $offsets += $output.Position
            $bytes = $ascii.GetBytes("$n 0 obj`n") + $objects[$n] + $ascii.GetBytes("`nendobj`n")
            $output.Write($bytes, 0, $bytes.Length)
        }
        $xrefOffset = $output.Position
        $xref = "xref`n0 $($count + 1)`n0000000000 65535 f `n"
        for ($n = 1; $n -le $count; $n++) { $xref += ('{0:D10} 00000 n ' -f [long]$offsets[$n]) + "`n" }
        $xref += "trailer`n<< /Size $($count + 1) /Root 1 0 R$trailerExtra >>`nstartxref`n$xrefOffset`n%%EOF`n"
        $bytes = $ascii.GetBytes($xref); $output.Write($bytes, 0, $bytes.Length)
        [IO.File]::WriteAllBytes((Join-Path $Destination $Name), $output.ToArray())
    } finally { $output.Dispose() }
}
Write-Fixture 'reading-sample.pdf' $false
Write-Fixture 'protected-sample.pdf' $true
Write-Fixture 'features-sample.pdf' $false 'Interactive'
Write-Fixture 'geometry-sample.pdf' $false 'Geometry'
Write-Fixture 'tall-sample.pdf' $false 'Tall'
Write-Output "PDFs originales generados en $Destination."
