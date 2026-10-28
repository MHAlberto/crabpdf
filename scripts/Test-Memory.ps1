[CmdletBinding()]
param(
    [string]$Executable='target/x86_64-pc-windows-msvc/release/examples/memory_probe.exe',
    [string]$Pdfium='native/pdfium.dll',
    [string]$Fixture='tests/fixtures/reading-sample.pdf',
    [string]$ReportRoot='artifacts/memory',
    [ValidateRange(1,20)][int]$Runs=3
)
$ErrorActionPreference='Stop'
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$exe=[IO.Path]::GetFullPath((Join-Path $workspace $Executable))
$pdf=[IO.Path]::GetFullPath((Join-Path $workspace $Fixture))
$dll=(Resolve-Path -LiteralPath (Join-Path $workspace $Pdfium)).Path
$root=[IO.Path]::GetFullPath((Join-Path $workspace $ReportRoot))
if (-not $root.StartsWith($workspace+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {throw 'ReportRoot must be inside the workspace.'}
New-Item -ItemType Directory -Path $root -Force | Out-Null
$previousData=$env:CRABPDF_DATA_DIR
$previousPdfium=$env:RUST_READER_PDFIUM
$reports=New-Object Collections.Generic.List[object]
try {
    $env:RUST_READER_PDFIUM=$dll
    for ($run=1; $run -le $Runs; $run++) {
        $folder=Join-Path $root "run-$run"
        if (Test-Path -LiteralPath $folder) {throw "Use a fresh report directory: $folder"}
        New-Item -ItemType Directory -Path $folder | Out-Null
        $env:CRABPDF_DATA_DIR=Join-Path $folder 'data'
        $samples=New-Object Collections.Generic.List[object]
        $watch=[Diagnostics.Stopwatch]::StartNew()
        $process=Start-Process -FilePath $exe -ArgumentList @('"'+$folder+'"','"'+$pdf+'"') -WorkingDirectory $workspace -PassThru -WindowStyle Hidden -RedirectStandardError (Join-Path $folder 'stderr.log')
        # Retain the native handle before the process exits (Windows PowerShell).
        $null=$process.Handle
        try {
            while (-not $process.HasExited) {
                if ($watch.Elapsed.TotalSeconds -gt 180) {throw 'Memory probe exceeded 180 seconds.'}
                try {
                    $process.Refresh()
                    if ($process.HasExited) {break}
                    $phaseFile=Join-Path $folder 'phase.txt'
                    $phase='startup'
                    if (Test-Path -LiteralPath $phaseFile) {
                        # Reading the marker must not deny the probe's next write.
                        try {
                            $phaseStream=[IO.File]::Open($phaseFile,[IO.FileMode]::Open,[IO.FileAccess]::Read,([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
                            $phaseReader=[IO.StreamReader]::new($phaseStream)
                            try {$phase=$phaseReader.ReadToEnd()} finally {$phaseReader.Dispose()}
                        } catch [IO.IOException] {continue}
                    }
                    $samples.Add(@{seconds=$watch.Elapsed.TotalSeconds;phase=$phase;working_set=$process.WorkingSet64;private_bytes=$process.PrivateMemorySize64;cpu_seconds=$process.TotalProcessorTime.TotalSeconds})
                } catch [InvalidOperationException] {if (-not $process.HasExited) {throw}}
                Start-Sleep -Milliseconds 25
            }
            $process.WaitForExit()
            if ($process.ExitCode -ne 0) {throw "Memory probe failed: $(Get-Content -LiteralPath (Join-Path $folder 'stderr.log') -Raw)"}
        } finally {
            if (-not $process.HasExited) {$process.Kill(); $process.WaitForExit()}
            $process.Dispose()
        }
        $timings=Get-Content -LiteralPath (Join-Path $folder 'timings.json') -Raw | ConvertFrom-Json
        if ($samples.Count -eq 0) {throw 'No process memory samples were recorded.'}
        $reports.Add(@{run=$run;timings=$timings;samples=$samples.ToArray();peak_working_set=($samples | ForEach-Object {$_['working_set']} | Measure-Object -Maximum).Maximum;peak_private_bytes=($samples | ForEach-Object {$_['private_bytes']} | Measure-Object -Maximum).Maximum})
        Write-Output "Memory run $run/$Runs complete."
    }
} finally {
    $env:CRABPDF_DATA_DIR=$previousData
    $env:RUST_READER_PDFIUM=$previousPdfium
    @{timestamp_utc=[DateTime]::UtcNow.ToString('o');os=[Environment]::OSVersion.VersionString;executable=$exe;executable_sha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash;pdfium_sha256=(Get-FileHash -LiteralPath $dll -Algorithm SHA256).Hash;fixture_sha256=(Get-FileHash -LiteralPath $pdf -Algorithm SHA256).Hash;runs=$reports.ToArray();sample_interval_ms=25;notes='Observed reader process only. Working set and private committed bytes differ; GPU and converter processes excluded. Other desktop workloads and OS caches are uncontrolled.'} |
        ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $root 'report.json') -Encoding UTF8
}
Write-Output "Report: $(Join-Path $root 'report.json')"
