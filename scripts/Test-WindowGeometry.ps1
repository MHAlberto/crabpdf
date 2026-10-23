[CmdletBinding()]
param([string]$Executable='target/debug/examples/document_chrome_smoke.exe',[string]$ReportRoot='artifacts/validation-chrome-20261023/normal',[switch]$Native)
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT'){throw 'Este control requiere Win32.'}
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$report=if([IO.Path]::IsPathRooted($ReportRoot)){[IO.Path]::GetFullPath($ReportRoot)}else{[IO.Path]::GetFullPath((Join-Path $workspace $ReportRoot))}
if(-not $report.StartsWith($workspace+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'El informe debe quedar dentro del proyecto.'}
$exe=[IO.Path]::GetFullPath((Join-Path $workspace $Executable))
if(-not(Test-Path -LiteralPath $exe)){throw 'Compila primero document_chrome_smoke.'}
New-Item -ItemType Directory -Path $report -Force|Out-Null
$processName=[IO.Path]::GetFileNameWithoutExtension($exe)
$observer=Start-Job -ArgumentList @($processName,$report,$PID) -ScriptBlock {
 param($processName,$report,$parentId)
 $ErrorActionPreference='Stop'
 Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class CrabGeometry {
 [StructLayout(LayoutKind.Sequential)] public struct R { public int Left,Top,Right,Bottom; }
 [StructLayout(LayoutKind.Sequential)] public struct P { public int X,Y; }
 [StructLayout(LayoutKind.Sequential)] public struct M { public int cbSize; public R Monitor,Work; public uint Flags; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out R r);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h,out R r);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h,ref P p);
 [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr h,uint f);
 [DllImport("user32.dll")] public static extern bool GetMonitorInfo(IntPtr h,ref M m);
}
'@
 $watch=[Diagnostics.Stopwatch]::StartNew()
 $rows=New-Object Collections.Generic.List[object]
 $seen=$false;$lastPhase=-1
 while($watch.Elapsed.TotalSeconds -lt 70){
  $candidate=Get-CimInstance Win32_Process -Filter "Name='$processName.exe' AND ParentProcessId=$parentId"|Select-Object -First 1
  if(-not $candidate){if($seen){break};Start-Sleep -Milliseconds 80;continue}
  $seen=$true
  $p=Get-Process -Id $candidate.ProcessId -ErrorAction SilentlyContinue
  if($p -and $p.MainWindowHandle -ne [IntPtr]::Zero -and (Test-Path -LiteralPath (Join-Path $report 'ready-phase.json'))){
   try{$phase=Get-Content -Raw -LiteralPath (Join-Path $report 'ready-phase.json')|ConvertFrom-Json}catch{continue}
   if($phase.phase -ne $lastPhase){
    $h=$p.MainWindowHandle
    $outer=New-Object CrabGeometry+R;$client=New-Object CrabGeometry+R;$origin=New-Object CrabGeometry+P;$monitor=New-Object CrabGeometry+M
    $monitor.cbSize=[Runtime.InteropServices.Marshal]::SizeOf($monitor)
    if([CrabGeometry]::GetWindowRect($h,[ref]$outer) -and [CrabGeometry]::GetClientRect($h,[ref]$client) -and [CrabGeometry]::ClientToScreen($h,[ref]$origin) -and [CrabGeometry]::GetMonitorInfo([CrabGeometry]::MonitorFromWindow($h,2),[ref]$monitor)){
     $rows.Add(@{phase=$phase.phase;fullscreen=$phase.fullscreen;native=$phase.native;outer=$outer;client=$client;origin=$origin;monitor=$monitor.Monitor})
     $lastPhase=$phase.phase
    }
   }
  }
  Start-Sleep -Milliseconds 60
 }
 $rows.ToArray()|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $report 'native-geometry.json') -Encoding UTF8
 $full=@($rows.ToArray()|Where-Object fullscreen)
 if(-not($full|Where-Object phase -eq 18) -or -not($full|Where-Object phase -eq 22)){throw 'Faltan mediciones desde ventana normal o maximizada.'}
 foreach($r in $full){
  if($r.origin.X -ne $r.monitor.Left -or $r.origin.Y -ne $r.monitor.Top -or $r.client.Right -ne $r.monitor.Right-$r.monitor.Left -or $r.client.Bottom -ne $r.monitor.Bottom-$r.monitor.Top){
   throw ("Cliente incompleto en fase {0}: origen {1},{2}; cliente {3}x{4}" -f $r.phase,$r.origin.X,$r.origin.Y,$r.client.Right,$r.client.Bottom)
  }
 }
 "Geometria nativa aprobada: $($full.Count) mediciones de pantalla completa."
}
try{
 Push-Location -LiteralPath $workspace
 $arguments=@($report)
 if($Native){$arguments+='--native'}
 & $exe @arguments
 $code=$LASTEXITCODE
 $null=Wait-Job $observer -Timeout 75
 Receive-Job $observer
 if($observer.State -ne 'Completed'){throw 'Fallo del observador Win32.'}
 if($code -ne 0){throw "Fallo del recorrido grafico: $code"}
}finally{
 Pop-Location
 if($observer.State -eq 'Running'){Stop-Job $observer}
 Remove-Job $observer
}
