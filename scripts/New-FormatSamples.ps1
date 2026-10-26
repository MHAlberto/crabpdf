[CmdletBinding()]
param([string]$Destination='artifacts/format-samples')
$ErrorActionPreference='Stop'
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$sampleRoot=[IO.Path]::GetFullPath((Join-Path $workspace $Destination))
if (-not $sampleRoot.StartsWith($workspace+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Los ejemplos deben quedar dentro del proyecto.' }
$archiveTool=Join-Path $workspace 'native/format-tools/7z.exe'
$utf8=New-Object Text.UTF8Encoding($false)
function Write-SampleText([string]$Relative,[string]$Text) {
    $path=Join-Path $sampleRoot $Relative
    New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($path)) -Force | Out-Null
    [IO.File]::WriteAllText($path,$Text,$utf8)
}
function New-SampleArchive([string]$Name,[string]$Type,[string]$Folder,[string[]]$Files) {
    Push-Location -LiteralPath (Join-Path $sampleRoot $Folder)
    try { & $archiveTool a "-t$Type" '-mx=0' (Join-Path $sampleRoot $Name) @Files | Out-Null; if($LASTEXITCODE -ne 0){throw "Fallo de $Name"} }
    finally { Pop-Location }
}
New-Item -ItemType Directory -Path $sampleRoot -Force | Out-Null
Write-SampleText 'text.txt' "CrabPDF texto seleccionable.`nSegunda línea: lectura y colores."
Write-SampleText 'markdown.md' "# CrabPDF Markdown`n`nTexto seleccionable y **negrita**.`n`n- Primer elemento`n- Segundo elemento"
Write-SampleText 'book.fb2' '<?xml version="1.0" encoding="UTF-8"?><FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0"><description><title-info><genre>science</genre><author><first-name>CrabPDF</first-name><last-name>Tests</last-name></author><book-title>CrabPDF FictionBook</book-title><lang>es</lang></title-info></description><body><section><title><p>Primer capítulo</p></title><p>CrabPDF texto seleccionable en FictionBook.</p></section><section><title><p>Segundo capítulo</p></title><p>Segunda página del libro.</p></section></body></FictionBook>'
Write-SampleText 'epub/mimetype' 'application/epub+zip'
Write-SampleText 'epub/META-INF/container.xml' '<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>'
Write-SampleText 'epub/OEBPS/content.opf' '<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>CrabPDF EPUB</dc:title><dc:identifier id="id">crabpdf-test</dc:identifier><dc:language>es</dc:language></metadata><manifest><item id="p1" href="one.xhtml" media-type="application/xhtml+xml"/><item id="p2" href="two.xhtml" media-type="application/xhtml+xml"/><item id="toc" href="toc.ncx" media-type="application/x-dtbncx+xml"/></manifest><spine toc="toc"><itemref idref="p1"/><itemref idref="p2"/></spine></package>'
Write-SampleText 'epub/OEBPS/one.xhtml' '<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Primer capítulo</title></head><body><h1>CrabPDF EPUB</h1><p>Texto seleccionable del primer capítulo.</p><a href="two.xhtml">Ir al segundo capítulo</a></body></html>'
Write-SampleText 'epub/OEBPS/two.xhtml' '<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Segundo capítulo</title></head><body><h1>Segundo capítulo</h1><p>CrabPDF segunda página.</p></body></html>'
Write-SampleText 'epub/OEBPS/toc.ncx' '<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="crabpdf-test"/></head><docTitle><text>CrabPDF EPUB</text></docTitle><navMap><navPoint id="one" playOrder="1"><navLabel><text>Primer capítulo</text></navLabel><content src="one.xhtml"/></navPoint><navPoint id="two" playOrder="2"><navLabel><text>Segundo capítulo</text></navLabel><content src="two.xhtml"/></navPoint></navMap></ncx>'
New-SampleArchive 'book.epub' 'zip' 'epub' @('mimetype','META-INF','OEBPS')
New-SampleArchive 'book.fb2z' 'zip' '.' @('book.fb2')

Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Path (Join-Path $sampleRoot 'images') -Force | Out-Null
foreach($index in @(1,2,10)) {
    $bitmap=New-Object Drawing.Bitmap(240,320)
    $graphics=[Drawing.Graphics]::FromImage($bitmap)
    try {
        $color=if($index -eq 1){[Drawing.Color]::LightBlue}elseif($index -eq 2){[Drawing.Color]::LightGreen}else{[Drawing.Color]::LightPink}
        $graphics.Clear($color)
        $graphics.FillRectangle([Drawing.Brushes]::Black,20,20,80,20)
        $bitmap.Save((Join-Path $sampleRoot "images/page$index.png"),[Drawing.Imaging.ImageFormat]::Png)
        if($index -eq 1) {
            $bitmap.Save((Join-Path $sampleRoot 'image.jpg'),[Drawing.Imaging.ImageFormat]::Jpeg)
            $bitmap.Save((Join-Path $sampleRoot 'image.bmp'),[Drawing.Imaging.ImageFormat]::Bmp)
            $bitmap.Save((Join-Path $sampleRoot 'image.gif'),[Drawing.Imaging.ImageFormat]::Gif)
            $bitmap.Save((Join-Path $sampleRoot 'image.tif'),[Drawing.Imaging.ImageFormat]::Tiff)
        }
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
New-SampleArchive 'comic.cbz' 'zip' 'images' @('page10.png','page2.png','page1.png')
$firstImage=[Drawing.Image]::FromFile((Join-Path $sampleRoot 'images/page1.png'))
$secondImage=[Drawing.Image]::FromFile((Join-Path $sampleRoot 'images/page2.png'))
$encoder=[Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object MimeType -eq 'image/tiff'
$parameters=New-Object Drawing.Imaging.EncoderParameters(1)
try {
    $parameters.Param[0]=New-Object Drawing.Imaging.EncoderParameter([Drawing.Imaging.Encoder]::SaveFlag,[long][Drawing.Imaging.EncoderValue]::MultiFrame)
    $firstImage.Save((Join-Path $sampleRoot 'multi.tiff'),$encoder,$parameters)
    $parameters.Param[0].Dispose()
    $parameters.Param[0]=New-Object Drawing.Imaging.EncoderParameter([Drawing.Imaging.Encoder]::SaveFlag,[long][Drawing.Imaging.EncoderValue]::FrameDimensionPage)
    $firstImage.SaveAdd($secondImage,$parameters)
    $parameters.Param[0].Dispose()
    $parameters.Param[0]=New-Object Drawing.Imaging.EncoderParameter([Drawing.Imaging.Encoder]::SaveFlag,[long][Drawing.Imaging.EncoderValue]::Flush)
    $firstImage.SaveAdd($parameters)
} finally {$parameters.Dispose();$firstImage.Dispose();$secondImage.Dispose()}
New-SampleArchive 'comic.cb7' '7z' 'images' @('page10.png','page2.png','page1.png')
New-SampleArchive 'comic.cbt' 'tar' 'images' @('page10.png','page2.png','page1.png')
function Get-CRC32([byte[]]$Data) {
    [uint32]$crc=4294967295
    foreach($byte in $Data) {
        $crc=$crc -bxor $byte
        for($bit=0;$bit -lt 8;$bit++) { if($crc -band 1){$crc=($crc -shr 1) -bxor 3988292384L}else{$crc=$crc -shr 1} }
    }
    return [uint32]($crc -bxor 4294967295L)
}
function Write-RarHeader([IO.BinaryWriter]$Writer,[byte[]]$Header) {
    $Writer.Write([uint16]((Get-CRC32 $Header) -band 65535L)); $Writer.Write($Header)
}
$rarStream=New-Object IO.MemoryStream
$rarWriter=New-Object IO.BinaryWriter($rarStream)
try {
    $rarWriter.Write([byte[]]@(82,97,114,33,26,7,0))
    Write-RarHeader $rarWriter ([byte[]]@(115,0,0,13,0,0,0,0,0,0,0))
    foreach($index in @(10,2,1)) {
        $data=[IO.File]::ReadAllBytes((Join-Path $sampleRoot "images/page$index.png"))
        $name=[Text.Encoding]::ASCII.GetBytes("page$index.png")
        $headerStream=New-Object IO.MemoryStream; $header=New-Object IO.BinaryWriter($headerStream)
        try {
            $header.Write([byte]116); $header.Write([uint16]32768); $header.Write([uint16](32+$name.Length))
            $header.Write([uint32]$data.Length); $header.Write([uint32]$data.Length); $header.Write([byte]2)
            $header.Write([uint32](Get-CRC32 $data)); $header.Write([uint32]0); $header.Write([byte]20)
            $header.Write([byte]48); $header.Write([uint16]$name.Length); $header.Write([uint32]32); $header.Write($name)
            Write-RarHeader $rarWriter $headerStream.ToArray(); $rarWriter.Write($data)
        } finally { $header.Dispose(); $headerStream.Dispose() }
    }
    Write-RarHeader $rarWriter ([byte[]]@(123,0,0,7,0))
    [IO.File]::WriteAllBytes((Join-Path $sampleRoot 'comic.cbr'),$rarStream.ToArray())
} finally { $rarWriter.Dispose(); $rarStream.Dispose() }
Copy-Item -LiteralPath (Join-Path $sampleRoot 'comic.cbr') -Destination (Join-Path $sampleRoot 'comic.rar') -Force
foreach($pair in @(@('comic.cbz','comic.zip'),@('comic.cb7','comic.7z'),@('comic.cbt','comic.tar'),@('book.fb2z','book.fb2.zip'),@('book.fb2z','book.fbz'),@('book.fb2z','book.zfb2'),@('text.txt','text.log'),@('text.txt','text.nfo'))) {
    Copy-Item -LiteralPath (Join-Path $sampleRoot $pair[0]) -Destination (Join-Path $sampleRoot $pair[1]) -Force
}
$sevenInstaller=Join-Path $workspace 'native/format-downloads/7z2604.exe'
if((Get-FileHash -LiteralPath $sevenInstaller -Algorithm SHA256).Hash -ne '39BF65045153FC26C42FA9FE47AFD8CD1B11671C7F11D031E738C9A13960C009') {
    throw 'Falta el instalador verificado de 7-Zip 26.04; ejecuta Get-FormatTools.ps1.'
}
$chmDirectory=Join-Path $sampleRoot 'chm-source'
& $archiveTool e $sevenInstaller "-o$chmDirectory" '7-zip.chm' -y | Out-Null
if($LASTEXITCODE -ne 0){throw 'Fallo al extraer la muestra CHM de 7-Zip.'}
Copy-Item -LiteralPath (Join-Path $chmDirectory '7-zip.chm') -Destination (Join-Path $sampleRoot 'help.chm') -Force
Copy-Item -LiteralPath (Join-Path $workspace 'native/format-downloads/djvulibre/doc/lizard2007.djvu') -Destination (Join-Path $sampleRoot 'book.djvu') -Force
Copy-Item -LiteralPath (Join-Path $sampleRoot 'book.djvu') -Destination (Join-Path $sampleRoot 'book.djv') -Force
Write-SampleText 'image.svg' '<svg xmlns="http://www.w3.org/2000/svg" width="240" height="320"><rect width="240" height="320" fill="#90c0ff"/><rect x="20" y="20" width="80" height="20" fill="black"/></svg>'

$fonts=Join-Path $env:USERPROFILE '.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/epaint_default_fonts-0.36.2/fonts'
$font=Get-ChildItem -LiteralPath $fonts -Recurse -File -Filter '*.ttf' | Where-Object Name -NotMatch 'emoji' | Select-Object -First 1
if(-not $font){throw 'Falta la fuente de epaint para generar XPS.'}
New-Item -ItemType Directory -Path (Join-Path $sampleRoot 'xps/Resources') -Force | Out-Null
Copy-Item -LiteralPath $font.FullName -Destination (Join-Path $sampleRoot 'xps/Resources/font.ttf') -Force
Write-SampleText 'xps/[Content_Types].xml' '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="fpage" ContentType="application/vnd.ms-package.xps-fixedpage+xml"/><Default Extension="fdoc" ContentType="application/vnd.ms-package.xps-fixeddocument+xml"/><Default Extension="fdseq" ContentType="application/vnd.ms-package.xps-fixeddocumentsequence+xml"/><Default Extension="ttf" ContentType="application/vnd.ms-opentype"/></Types>'
Write-SampleText 'xps/_rels/.rels' '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="R1" Type="http://schemas.microsoft.com/xps/2005/06/fixedrepresentation" Target="/FixedDocSeq.fdseq"/></Relationships>'
Write-SampleText 'xps/FixedDocSeq.fdseq' '<FixedDocumentSequence xmlns="http://schemas.microsoft.com/xps/2005/06"><DocumentReference Source="/Documents/1/FixedDoc.fdoc"/></FixedDocumentSequence>'
Write-SampleText 'xps/Documents/1/FixedDoc.fdoc' '<FixedDocument xmlns="http://schemas.microsoft.com/xps/2005/06"><PageContent Source="/Documents/1/Pages/1.fpage"/><PageContent Source="/Documents/1/Pages/2.fpage"/></FixedDocument>'
foreach($page in @(1,2)) {
    Write-SampleText "xps/Documents/1/Pages/$page.fpage" "<FixedPage Width=`"816`" Height=`"1056`" xmlns=`"http://schemas.microsoft.com/xps/2005/06`" xml:lang=`"es`"><Glyphs FontUri=`"/Resources/font.ttf`" FontRenderingEmSize=`"20`" OriginX=`"40`" OriginY=`"80`" UnicodeString=`"CrabPDF XPS texto pagina $page`" Fill=`"#000000`"/></FixedPage>"
}
New-SampleArchive 'book.xps' 'zip' 'xps' @('[Content_Types].xml','_rels','FixedDocSeq.fdseq','Documents','Resources')
Copy-Item -LiteralPath (Join-Path $sampleRoot 'book.xps') -Destination (Join-Path $sampleRoot 'book.oxps') -Force
Write-Output "Ejemplos: $sampleRoot"
