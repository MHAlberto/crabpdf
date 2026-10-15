[CmdletBinding()]
param([switch]$SkipBuild)
$ErrorActionPreference='Stop'
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$downloads=Join-Path $workspace 'native/format-downloads'
$tools=Join-Path $workspace 'native/format-tools'
New-Item -ItemType Directory -Path $downloads,$tools -Force | Out-Null
function Get-Verified([string]$Name,[string]$Uri,[string]$Hash) {
    $path=Join-Path $downloads $Name
    if(-not (Test-Path -LiteralPath $path)) {
        $response=Invoke-WebRequest -UseBasicParsing -Uri $Uri
        if($response.Headers['Content-Type'] -like '*text/html*') {
            $match=[regex]::Match($response.Content,'<meta http-equiv="refresh" content="5; url=([^"]+)')
            if(-not $match.Success){throw "No se recibió el archivo $Name"}
            $redirect=[Uri][Net.WebUtility]::HtmlDecode($match.Groups[1].Value)
            if(-not $redirect.Host.EndsWith('.sourceforge.net')){throw 'Redirección de descarga no autorizada.'}
            Invoke-WebRequest -UseBasicParsing -Uri $redirect -OutFile $path
        }else{[IO.File]::WriteAllBytes($path,$response.RawContentStream.ToArray())}
    }
    if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $Hash){throw "Hash incorrecto: $Name"}
    return $path
}
$mupdfSource=Get-Verified 'mupdf-1.28.5-source.tar.gz' 'https://github.com/ArtifexSoftware/mupdf-downloads/releases/download/1.28.5/mupdf-1.28.5-source.tar.gz' '98A5C10CDA20C3992CDF76FF6B2A1149C32BD79CC796D3F703230B1185B7E934'
$libmobiSource=Get-Verified 'libmobi-source.tar.gz' 'https://api.github.com/repos/bfabiszewski/libmobi/tarball/v0.12' '7ABB0EE4C401EF225BBE03B4EBF94D76C81D810FA9B6274F74B4751E10ECD4F0'
$seven=Get-Verified '7z2604.exe' 'https://github.com/ip7z/7zip/releases/download/26.04/7z2604.exe' '39BF65045153FC26C42FA9FE47AFD8CD1B11671C7F11D031E738C9A13960C009'
$sevenSource=Get-Verified '7z2604-src.tar.xz' 'https://github.com/ip7z/7zip/releases/download/26.04/7z2604-src.tar.xz' '9691944C0FE0D01BB49373A704FB983FD33BC98B1738695179DFBF99AC1734F6'
$djvu=Get-Verified 'djvulibre-real.exe' 'https://downloads.sourceforge.net/project/djvu/DjVuLibre_Windows/3.5.29%2B4.12/DjVuLibre-3.5.29_DjView-4.12_Setup.exe' '92233FBF891C63F3FB7A0B5E1CE108BAA4C29A40C89A442D1313F883AAE84670'
$djvuSource=Get-Verified 'djvulibre-3.5.29.tar.gz' 'https://downloads.sourceforge.net/project/djvu/DjVuLibre/3.5.29/djvulibre-3.5.29.tar.gz' 'D3B4B03AE2BDCA8516A36EF6EB27B777F0528C9EDA26745D9962824A3FDFECCF'
$tiff=Get-Verified 'tiff-4.0.10.tar.gz' 'https://download.osgeo.org/libtiff/tiff-4.0.10.tar.gz' '2C52D11CCAF767457DB0C46795D9C7D1A8D8F76F68B0B800A3DFE45786B996E4'
$jpeg=Get-Verified 'jpegsrc.v6b.tar.gz' 'https://www.ijg.org/files/jpegsrc.v6b.tar.gz' '75C3EC241E9996504FE02A9ED4D12F16B74ADE713972F3DB9E65CE95CD27E35D'
$zlib=Get-Verified 'zlib-1.2.11.tar.gz' 'https://zlib.net/fossils/zlib-1.2.11.tar.gz' 'C3E5E9FDD5004DCB542FEDA5EE4F0FF0744628BAF8ED2DD5D66F8CA1197CB1A1'
$bootstrap=Get-Verified '7zr.exe' 'https://www.7-zip.org/a/7zr.exe' '256FECA8E274E5DA655E2A284FABAFD9F554365EB164862089DACD4E8276D282'
# 7zr only bootstraps the verified 7-Zip archive; it is not included in the reader.
& (Join-Path $downloads '7zr.exe') e $seven "-o$tools" '7z.exe' '7z.dll' 'License.txt' -y | Out-Null
if($LASTEXITCODE -ne 0){throw 'Fallo de extracción de 7-Zip.'}
$djvuDirectory=Join-Path $downloads 'djvulibre'
& (Join-Path $tools '7z.exe') x $djvu "-o$djvuDirectory" -y | Out-Null
if($LASTEXITCODE -ne 0){throw 'Fallo de extracción de DjVuLibre.'}
foreach($name in @('ddjvu.exe','djvutoxml.exe','libdjvulibre.dll','libjpeg.dll','libtiff.dll','libz.dll')){Copy-Item -LiteralPath (Join-Path $djvuDirectory $name) -Destination $tools -Force}
Copy-Item -LiteralPath (Join-Path $djvuDirectory 'COPYING.txt') -Destination (Join-Path $tools 'DjVuLibre-COPYING.txt') -Force
$vswhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs=& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if(-not $vs){throw 'Falta Visual Studio con las herramientas C++ para preparar Windows x86.'}
$redist=Get-ChildItem -LiteralPath (Join-Path $vs 'VC/Redist/MSVC') -Directory | Where-Object Name -Match '^14\.' | Sort-Object Name -Descending | Select-Object -First 1
foreach($name in @('vcruntime140.dll','msvcp140.dll')){Copy-Item -LiteralPath (Join-Path $redist.FullName "x86/Microsoft.VC143.CRT/$name") -Destination $tools -Force}
if(-not $SkipBuild) {
    $mupdf=Join-Path $workspace 'native/format-sources/mupdf'
    $libmobi=Join-Path $workspace 'native/format-sources/libmobi'
    New-Item -ItemType Directory -Path $mupdf,$libmobi -Force | Out-Null
    if(-not(Test-Path -LiteralPath (Join-Path $mupdf 'platform/win32/mupdf.sln'))) {
        & tar -xzf $mupdfSource -C $mupdf --strip-components=1 '--exclude=*/thirdparty/freeglut/progs/test-shapes-gles1/android_toolchain.cmake' '--exclude=*/thirdparty/zxing-cpp/wrappers/python/core' '--exclude=*/thirdparty/zxing-cpp/wrappers/python/zxing.cmake' '--exclude=*/thirdparty/zxing-cpp/wrappers/rust/core'
        if($LASTEXITCODE -ne 0){throw 'Fallo de extracción de las fuentes MuPDF.'}
    }
    if(-not(Test-Path -LiteralPath (Join-Path $libmobi 'CMakeLists.txt'))) {& tar -xzf $libmobiSource -C $libmobi --strip-components=1;if($LASTEXITCODE -ne 0){throw 'Fallo de extracción de libmobi.'}}
    $cmake=Join-Path $libmobi 'tools/CMakeLists.txt'
    $text=[IO.File]::ReadAllText($cmake)
    if(-not $text.Contains('target_sources(common PRIVATE win32/getopt.c)')){
        $text=$text.Replace('add_library(common OBJECT common.c)',"add_library(common OBJECT common.c)`nif(MSVC)`n    target_sources(common PRIVATE win32/getopt.c)`nendif()")
        [IO.File]::WriteAllText($cmake,$text)
    }
    & (Join-Path $vs 'MSBuild/Current/Bin/amd64/MSBuild.exe') (Join-Path $mupdf 'platform/win32/mupdf.sln') /t:mutool /m:4 /p:Configuration=Release /p:Platform=Win32 /p:PlatformToolset=v143 "/p:ForceImportAfterCppTargets=$(Join-Path $PSScriptRoot 'FormatToolsStaticCRT.props')" /v:minimal
    if($LASTEXITCODE -ne 0){throw 'Fallo de compilación de MuPDF x86.'}
    & cmake -S $libmobi -B (Join-Path $workspace 'native/format-build/libmobi') -G 'Visual Studio 17 2022' -A Win32 -DTOOLS_STATIC=ON -DUSE_ZLIB=OFF -DUSE_LIBXML2=OFF -DUSE_ENCRYPTION=OFF -DCMAKE_POLICY_DEFAULT_CMP0091=NEW -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded
    if($LASTEXITCODE -ne 0){throw 'Fallo de configuración de libmobi.'}
    & cmake --build (Join-Path $workspace 'native/format-build/libmobi') --config Release --target mobitool --parallel 4
    if($LASTEXITCODE -ne 0){throw 'Fallo de compilación de libmobi.'}
    Copy-Item -LiteralPath (Join-Path $mupdf 'platform/win32/Release/mutool.exe') -Destination $tools -Force
    Copy-Item -LiteralPath (Join-Path $workspace 'native/format-build/libmobi/tools/Release/mobitool.exe') -Destination $tools -Force
}
Copy-Item -LiteralPath (Join-Path $workspace 'native/format-sources/mupdf/COPYING') -Destination (Join-Path $tools 'MuPDF-COPYING.txt') -Force
Copy-Item -LiteralPath (Join-Path $workspace 'native/format-sources/libmobi/COPYING') -Destination (Join-Path $tools 'Libmobi-COPYING.LESSER.txt') -Force
$gpl=Get-Verified 'GPL-3.0.txt' 'https://www.gnu.org/licenses/gpl-3.0.txt' '3972DC9744F6499F0F9B2DBF76696F2AE7AD8AF9B23DDE66D6AF86C9DFB36986'
Copy-Item -LiteralPath $gpl -Destination (Join-Path $tools 'Libmobi-COPYING.txt') -Force
$licenseSources=Join-Path $workspace 'native/format-sources/licenses'
New-Item -ItemType Directory -Path $licenseSources -Force | Out-Null
& tar -xzf $tiff -C $licenseSources 'tiff-4.0.10/COPYRIGHT'; if($LASTEXITCODE -ne 0){throw 'Falta el aviso de TIFF.'}
& tar -xzf $jpeg -C $licenseSources 'jpeg-6b/README'; if($LASTEXITCODE -ne 0){throw 'Falta el aviso de JPEG.'}
& tar -xzf $zlib -C $licenseSources 'zlib-1.2.11/README'; if($LASTEXITCODE -ne 0){throw 'Falta el aviso de zlib.'}
Copy-Item -LiteralPath (Join-Path $licenseSources 'tiff-4.0.10/COPYRIGHT') -Destination (Join-Path $tools 'TIFF-COPYRIGHT.txt') -Force
Copy-Item -LiteralPath (Join-Path $licenseSources 'jpeg-6b/README') -Destination (Join-Path $tools 'JPEG-README.txt') -Force
Copy-Item -LiteralPath (Join-Path $licenseSources 'zlib-1.2.11/README') -Destination (Join-Path $tools 'ZLIB-README.txt') -Force
$names=@('7z.exe','7z.dll','mutool.exe','mobitool.exe','ddjvu.exe','djvutoxml.exe','libdjvulibre.dll','libjpeg.dll','libtiff.dll','libz.dll','vcruntime140.dll','msvcp140.dll','License.txt','MuPDF-COPYING.txt','Libmobi-COPYING.txt','Libmobi-COPYING.LESSER.txt','DjVuLibre-COPYING.txt','TIFF-COPYRIGHT.txt','JPEG-README.txt','ZLIB-README.txt')
$files=@(foreach($name in $names){$path=Join-Path $tools $name;if(-not(Test-Path -LiteralPath $path)){throw "Falta $name"};@{path=$name;bytes=(Get-Item -LiteralPath $path).Length;sha256=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}})
@{version=1;target='i686-pc-windows-msvc';mupdf='1.28.5';libmobi='0.12';djvulibre='3.5.29';sevenzip='26.04';crt='Visual Studio 2022 redist 14.44.35112 x86 (DjVuLibre)';files=$files;sources=@('mupdf-1.28.5-source.tar.gz','libmobi-source.tar.gz','djvulibre-3.5.29.tar.gz','7z2604-src.tar.xz','tiff-4.0.10.tar.gz','jpegsrc.v6b.tar.gz','zlib-1.2.11.tar.gz')} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $tools 'provenance.json') -Encoding UTF8
Write-Output 'Decodificadores preparados en native/format-tools. Compatibles con Windows x86 y x64/WOW64.'
