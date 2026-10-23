[CmdletBinding()]
param([ValidateSet('x64','x86','both')][string]$Architecture='both', [switch]$SkipVisual, [string]$ReportRoot='')
$ErrorActionPreference='Stop'
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$reportRoot=if($ReportRoot){[IO.Path]::GetFullPath($ReportRoot)}else{Join-Path $workspace 'artifacts/validation'}
New-Item -ItemType Directory -Path $reportRoot -Force | Out-Null
$records=New-Object Collections.Generic.List[object]
$originalPdfium=$env:RUST_READER_PDFIUM
$originalData=$env:CRABPDF_DATA_DIR
$env:CRABPDF_DATA_DIR=Join-Path $reportRoot 'reader-data'
function Invoke-CargoCheck([string]$Name,[string[]]$Arguments) {
    Write-Output "Comprobando: $Name"
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $previousAction=$ErrorActionPreference
    $ErrorActionPreference='Continue'
    try { $output=& cargo @Arguments 2>&1; $code=$LASTEXITCODE }
    finally { $ErrorActionPreference=$previousAction }
    $watch.Stop()
    $output | ForEach-Object { $_.ToString() } | Out-File -LiteralPath (Join-Path $reportRoot "$Name.log") -Encoding UTF8
    $records.Add(@{ check=$Name; command='cargo '+($Arguments -join ' '); exit_code=$code; seconds=$watch.Elapsed.TotalSeconds })
    if ($code -ne 0) { throw "Fallo de $Name. Consulta $(Join-Path $reportRoot "$Name.log")." }
}
Push-Location -LiteralPath $workspace
try {
    if(-not(Test-Path -LiteralPath (Join-Path $workspace 'native/format-tools/provenance.json'))){throw 'Faltan los decodificadores. Ejecuta scripts/Get-FormatTools.ps1.'}
    & (Join-Path $workspace 'scripts/New-FormatSamples.ps1')
    Invoke-CargoCheck 'format' @('fmt','--all','--','--check')
    $architectures=if($Architecture -eq 'both') { @('x64','x86') } else { @($Architecture) }
    foreach ($arch in $architectures) {
        $target=if($arch -eq 'x86') {'i686-pc-windows-msvc'} else {'x86_64-pc-windows-msvc'}
        $native=if($arch -eq 'x86') {Join-Path $workspace 'native/win-x86/pdfium.dll'} else {Join-Path $workspace 'native/pdfium.dll'}
        if (-not (Test-Path -LiteralPath $native)) {throw "Falta PDFium para $arch."}
        $env:RUST_READER_PDFIUM=$native
        Invoke-CargoCheck "$arch-clippy" @('clippy','--locked','--offline','--all-targets','--target',$target,'--','-D','warnings')
        Invoke-CargoCheck "$arch-tests" @('test','--locked','--offline','--release','--target',$target,'--','--include-ignored','--test-threads=1')
        $buildArguments=@('build','--locked','--offline','--release','--target',$target,'--bins')
        if (-not $SkipVisual) { $buildArguments+='--examples' }
        Invoke-CargoCheck "$arch-build" $buildArguments
        if (-not $SkipVisual) {
            $watch=[Diagnostics.Stopwatch]::StartNew()
            & (Join-Path $workspace "target/$target/release/examples/visual_smoke.exe") (Join-Path $reportRoot "$arch-ui")
            $code=$LASTEXITCODE; $watch.Stop()
            $records.Add(@{check="$arch-ui";exit_code=$code;seconds=$watch.Elapsed.TotalSeconds;captures=@(Get-ChildItem -LiteralPath (Join-Path $reportRoot "$arch-ui") -Filter '*.png').Count})
            if ($code -ne 0) {throw "Fallo del recorrido de interfaz $arch."}
            & (Join-Path $workspace "target/$target/release/examples/cli_smoke.exe")
            $code=$LASTEXITCODE
            $records.Add(@{check="$arch-cli-startup";exit_code=$code})
            if ($code -ne 0) {throw "Fallo del arranque CLI $arch."}
            & (Join-Path $workspace "target/$target/release/examples/search_smoke.exe")
            $code=$LASTEXITCODE
            $records.Add(@{check="$arch-search-viewport";exit_code=$code})
            if ($code -ne 0) {throw "Fallo al revelar una búsqueda en una página larga ($arch)."}
            & (Join-Path $workspace "target/$target/release/examples/reading_session_smoke.exe") (Join-Path $reportRoot "$arch-reading-session")
            $code=$LASTEXITCODE
            $records.Add(@{check="$arch-reading-session";exit_code=$code;captures=@(Get-ChildItem -LiteralPath (Join-Path $reportRoot "$arch-reading-session") -Filter '*.png').Count})
            if ($code -ne 0) {throw "Fallo de restauración de lectura entre procesos ($arch)."}
            foreach ($mode in @('normal','reduced','dark')) {
                $transitionArgs=@((Join-Path $reportRoot "$arch-transitions-$mode"))
                if ($mode -eq 'reduced') { $transitionArgs+='--reduced' }
                if ($mode -eq 'dark') { $transitionArgs+='--dark' }
                & (Join-Path $workspace "target/$target/release/examples/transition_smoke.exe") @transitionArgs
                $code=$LASTEXITCODE
                $records.Add(@{check="$arch-transitions-$mode";exit_code=$code;captures=@(Get-ChildItem -LiteralPath $transitionArgs[0] -Filter '*.png').Count})
                if ($code -ne 0) {throw "Fallo de transiciones $arch ($mode)."}
            }
            & (Join-Path $workspace "target/$target/release/examples/refinement_smoke.exe") (Join-Path $reportRoot "$arch-refinement")
            $code=$LASTEXITCODE
            $records.Add(@{check="$arch-refinement";exit_code=$code;captures=@(Get-ChildItem -LiteralPath (Join-Path $reportRoot "$arch-refinement") -Filter '*.png').Count})
            if ($code -ne 0) {throw "Fallo del refinamiento de interfaz $arch."}
            & (Join-Path $workspace "target/$target/release/examples/formats_smoke.exe") (Join-Path $reportRoot "$arch-formats")
            $code=$LASTEXITCODE
            $records.Add(@{check="$arch-formats-and-marks";exit_code=$code;captures=@(Get-ChildItem -LiteralPath (Join-Path $reportRoot "$arch-formats") -Filter '*.png').Count})
            if ($code -ne 0) {throw "Fallo de formatos y anotaciones $arch."}
        }
    }
} finally {
    @{timestamp_utc=[DateTime]::UtcNow.ToString('o');os=[Environment]::OSVersion.VersionString;host_64bit=[Environment]::Is64BitOperatingSystem;checks=@($records.ToArray())} |
        ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $reportRoot 'results.json') -Encoding UTF8
    $env:RUST_READER_PDFIUM=$originalPdfium
    $env:CRABPDF_DATA_DIR=$originalData
    Pop-Location
}
Write-Output "Resultados: $(Join-Path $reportRoot 'results.json')"
