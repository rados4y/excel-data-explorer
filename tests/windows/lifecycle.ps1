param([Parameter(Mandatory=$true)][string]$Exe)
$ErrorActionPreference = 'Stop'
# Run on an interactive Windows 11 desktop with no existing explorer process.
$name = [IO.Path]::GetFileNameWithoutExtension($Exe)
if (Get-Process -Name $name -ErrorAction SilentlyContinue) { throw 'Exit existing Explorer instances before this test.' }
$p = Start-Process $Exe -ArgumentList '--background' -PassThru
try {
  Start-Sleep 6
  $p.Refresh()
  if ($p.HasExited) { throw 'Background process exited.' }
  if ($p.MainWindowHandle -ne 0) { throw 'Login launch showed a window.' }
  Start-Process $Exe
  Start-Sleep 3
  $all = @(Get-Process -Name $name)
  if ($all.Count -ne 1) { throw 'Single-instance check failed.' }
  $p.Refresh()
  if ($p.MainWindowHandle -eq 0) { throw 'Second launch did not show existing window.' }
  $null = $p.CloseMainWindow()
  Start-Sleep 2
  if (-not (Get-Process -Id $p.Id -ErrorAction SilentlyContinue)) { throw 'Close terminated the process.' }
  Write-Host 'PASS: hidden login, single-instance restore, close-to-tray. Use tray Open/Exit and verify saved filters; see docs/WINDOWS_RELEASE.md.'
} finally {
  Write-Host 'Use tray Exit to finish; this script does not force-kill the application.'
}
