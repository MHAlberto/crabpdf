# Windows observer for the portable performance_probe example.
[CmdletBinding()]
param(
    [string]$Executable='target/x86_64-pc-windows-msvc/release/examples/performance_probe.exe',
    [string]$AppExecutable='target/x86_64-pc-windows-msvc/release/crabpdf.exe',
    [string]$Pdfium='native/pdfium.dll',
    [string]$Output='artifacts/architecture-phase1/before',
    [ValidateRange(1,50)][int]$Runs=5
)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$outputPath=[IO.Path]::GetFullPath((Join-Path $root $Output))
if(-not $outputPath.StartsWith($root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'La salida debe quedar dentro del proyecto.'}
if(Test-Path -LiteralPath (Join-Path $outputPath 'report.json')){throw 'Usa otra salida para conservar la medición anterior.'}
New-Item -ItemType Directory -Path $outputPath -Force | Out-Null
$probe=(Resolve-Path -LiteralPath (Join-Path $root $Executable)).Path
$app=(Resolve-Path -LiteralPath (Join-Path $root $AppExecutable)).Path
$pdfiumPath=(Resolve-Path -LiteralPath (Join-Path $root $Pdfium)).Path
$epub=(Resolve-Path -LiteralPath (Join-Path $root 'artifacts/format-samples/book.epub')).Path
# An original, deterministic 24-page PDF. No fonts or third-party documents.
$pdf=Join-Path $outputPath 'performance.pdf'
$ascii=[Text.Encoding]::ASCII
$lf=[char]10
$objects=New-Object Collections.Generic.List[string]
$objects.Add('<< /Type /Catalog /Pages 2 0 R >>')
$kids=(0..23 | ForEach-Object { "$(4+2*$_) 0 R" }) -join ' '
$objects.Add("<< /Type /Pages /Kids [$kids] /Count 24 >>")
$objects.Add('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')
foreach($number in 1..24){
    $stream=New-Object Text.StringBuilder
    [void]$stream.Append("0.96 0.97 0.99 rg 0 0 600 800 re f$($lf)0 0 0 rg BT /F1 18 Tf 45 750 Td (CrabPDF benchmark - page $number) Tj ET$($lf)")
    [void]$stream.Append("BT /F1 12 Tf 45 715 Td$($lf)")
    foreach($line in 1..42){[void]$stream.Append("(Original reading fixture. Page $number, line $line. Text and vectors.) Tj 0 -14 Td$($lf)")}
    [void]$stream.Append("ET$($lf)")
    $content=$stream.ToString()
    $streamId=3+2*$number
    $objects.Add("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 3 0 R >> >> /Contents $streamId 0 R >>")
    $objects.Add("<< /Length $($ascii.GetByteCount($content)) >>$($lf)stream$($lf)$($content)endstream")
}
$buffer=New-Object IO.MemoryStream
function Write-Ascii([string]$Text){$bytes=$ascii.GetBytes($Text);$buffer.Write($bytes,0,$bytes.Length)}
$offsets=New-Object Collections.Generic.List[long]
try{
    Write-Ascii "%PDF-1.4$($lf)"
    for($i=0;$i -lt $objects.Count;$i++){
        $offsets.Add($buffer.Position)
        Write-Ascii "$($i+1) 0 obj$($lf)$($objects[$i])$($lf)endobj$($lf)"
    }
    $xref=$buffer.Position
    Write-Ascii "xref$($lf)0 $($objects.Count+1)$($lf)0000000000 65535 f $($lf)"
    foreach($offset in $offsets){Write-Ascii ($offset.ToString('0000000000')+" 00000 n $($lf)")}
    Write-Ascii "trailer$($lf)<< /Size $($objects.Count+1) /Root 1 0 R >>$($lf)startxref$($lf)$xref$($lf)%%EOF$($lf)"
    [IO.File]::WriteAllBytes($pdf,$buffer.ToArray())
}finally{$buffer.Dispose()}
$originalData=$env:CRABPDF_DATA_DIR
$originalPdfium=$env:RUST_READER_PDFIUM
$records=New-Object Collections.Generic.List[object]
Push-Location -LiteralPath $root
try{
    $source=@(Get-ChildItem -LiteralPath (Join-Path $root 'src') -Recurse -Filter '*.rs' | Sort-Object FullName | ForEach-Object {
        @{path=$_.FullName.Substring($root.Length+1).Replace('\','/');sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    foreach($run in 1..$Runs){
        $runPath=Join-Path $outputPath "run-$run"
        if(Test-Path -LiteralPath $runPath){throw "La carpeta ya existe: $runPath"}
        New-Item -ItemType Directory -Path $runPath | Out-Null
        $env:CRABPDF_DATA_DIR=Join-Path $runPath 'data'
        $env:RUST_READER_PDFIUM=$pdfiumPath
        $arguments=@($runPath,$pdf,$epub) | ForEach-Object { '"'+$_+'"' }
        $process=New-Object Diagnostics.Process
        $process.StartInfo.FileName=$probe
        $process.StartInfo.Arguments=$arguments -join ' '
        $process.StartInfo.WorkingDirectory=$root
        $process.StartInfo.UseShellExecute=$false
        $process.StartInfo.CreateNoWindow=$true
        $process.StartInfo.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
        $process.StartInfo.RedirectStandardOutput=$true
        $process.StartInfo.RedirectStandardError=$true
        if(-not $process.Start()){throw 'No se pudo iniciar el recorrido.'}
        $null=$process.Handle
        $stdout=$process.StandardOutput.ReadToEndAsync()
        $stderr=$process.StandardError.ReadToEndAsync()
        $watch=[Diagnostics.Stopwatch]::StartNew()
        $peak=0L
        $idle=New-Object Collections.Generic.List[object]
        try{
            while(-not $process.HasExited){
                if($watch.Elapsed.TotalSeconds -gt 100){throw "Timeout del recorrido $run"}
                try{
                    $process.Refresh()
                    $peak=[Math]::Max($peak,$process.PeakWorkingSet64)
                    $phase='0'
                    $phasePath=Join-Path $runPath 'phase.txt'
                    if(Test-Path -LiteralPath $phasePath){
                        # The probe may replace this file while it changes phase.
                        # A sharing violation only skips this resource sample.
                        try{$phase=[IO.File]::ReadAllText($phasePath)}catch [IO.IOException]{}
                    }
                    if($phase -eq '9'){$idle.Add(@{wall=$watch.Elapsed.TotalSeconds;cpu=$process.TotalProcessorTime.TotalSeconds})}
                }catch [InvalidOperationException]{if(-not $process.HasExited){throw}}
                Start-Sleep -Milliseconds 50
            }
            $process.WaitForExit()
            [IO.File]::WriteAllText((Join-Path $runPath 'stdout.log'),$stdout.Result)
            [IO.File]::WriteAllText((Join-Path $runPath 'stderr.log'),$stderr.Result)
            if($process.ExitCode -ne 0){throw "Falló el recorrido $run. Consulta $runPath/stderr.log"}
        }finally{
            if(-not $process.HasExited){Stop-Process -Id $process.Id -Force}
        }
        $timings=Get-Content -Raw -Encoding UTF8 (Join-Path $runPath 'timings.json') | ConvertFrom-Json
        if($idle.Count -lt 2){throw 'No se pudo observar el intervalo de CPU en reposo.'}
        $seconds=$idle[$idle.Count-1].wall-$idle[0].wall
        $cpu=$idle[$idle.Count-1].cpu-$idle[0].cpu
        $records.Add(@{
            run=$run;timings=$timings;app_peak_working_set_bytes=$peak
            idle_observed_seconds=$seconds;idle_cpu_seconds=$cpu;idle_cpu_percent_one_core=100.0*$cpu/$seconds
        })
        Write-Output "Recorrido $run/$Runs completado."
    }
    $summary=@(foreach($name in $records[0].timings.samples.phase){
        $values=@($records | ForEach-Object { ($_.timings.samples | Where-Object phase -eq $name).elapsed_ms } | Sort-Object)
        $middle=[int][Math]::Floor($values.Count/2)
        $median=if($values.Count%2){$values[$middle]}else{($values[$middle-1]+$values[$middle])/2}
        @{phase=$name;median_ms=$median;min_ms=$values[0];max_ms=$values[$values.Count-1];n=$values.Count}
    })
    $manifest=@(foreach($path in @($probe,$app,$pdfiumPath,$pdf,$epub,(Join-Path $root 'native/format-tools/provenance.json'))){
        $file=Get-Item -LiteralPath $path
        @{path=$file.FullName.Substring($root.Length+1).Replace('\','/');bytes=$file.Length;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    $report=@{
        schema=1;timestamp_utc=[DateTime]::UtcNow.ToString('o');git_commit=(git rev-parse HEAD).Trim();git_status=@(git status --short)
        os=[Environment]::OSVersion.VersionString;logical_processors=[Environment]::ProcessorCount
        cpu=(Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name)
        rustc=(& rustc --version);cargo=(& cargo --version);source=$source;files=$manifest
        summary=$summary;runs=@($records.ToArray())
        limits='Windows observer measures only this app process. Converter CPU/RSS, GPU allocation and compositor presentation latency are not measured. Fresh per-run app/import caches; OS disk cache uncontrolled. Fixture is synthetic; not a large real-world corpus. Probe polls at 8ms outside idle; observer samples resources at 50ms. Min/median/max only; no p95 estimate.'
    }
    $report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $outputPath 'report.json') -Encoding UTF8
}finally{
    $env:CRABPDF_DATA_DIR=$originalData
    $env:RUST_READER_PDFIUM=$originalPdfium
    Pop-Location
}
Write-Output "Informe: $outputPath/report.json"
