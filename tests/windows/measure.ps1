param([Parameter(Mandatory=$true)][string]$Exe,[int]$Samples=30)
$ErrorActionPreference = 'Stop'
$name=[IO.Path]::GetFileNameWithoutExtension($Exe)
$p=Get-Process -Name $name -ErrorAction Stop | Select-Object -First 1
$times=@()
for($i=0;$i -lt $Samples;$i++) {
  $p.Refresh()
  if($p.MainWindowHandle -ne 0){$null=$p.CloseMainWindow();Start-Sleep -Milliseconds 400}
  $clock=[Diagnostics.Stopwatch]::StartNew()
  Start-Process $Exe
  do {Start-Sleep -Milliseconds 15;$p.Refresh()} while($p.MainWindowHandle -eq 0 -and $clock.Elapsed.TotalSeconds -lt 10)
  if($p.MainWindowHandle -eq 0){throw 'Window did not restore'}
  $times+=$clock.Elapsed.TotalMilliseconds
}
$sorted=@($times|Sort-Object)
$p.Refresh();$cpuStart=$p.TotalProcessorTime.TotalSeconds
Start-Sleep -Seconds 60
$p.Refresh();$idleCpu=100*($p.TotalProcessorTime.TotalSeconds-$cpuStart)/60
[pscustomobject]@{samples=$Samples;restore_handle_p50_ms=$sorted[[int]($Samples*.5)];restore_handle_p95_ms=$sorted[[Math]::Min($Samples-1,[int][Math]::Ceiling($Samples*.95)-1)];restore_handle_p99_ms=$sorted[[Math]::Min($Samples-1,[int][Math]::Ceiling($Samples*.99)-1)];idle_cpu_one_core_percent=$idleCpu;working_set_bytes=$p.WorkingSet64;note='Measures visible window handle, not completed WebView paint; CPU interval must have no active import.'}|ConvertTo-Json
