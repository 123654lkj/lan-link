$LogFile = "C:\Users\26063\AppData\Local\Temp\exe_test.log"
Remove-Item $LogFile -ErrorAction SilentlyContinue
function Log($msg) { Add-Content -Path $LogFile -Value "$(Get-Date -Format 'HH:mm:ss') $msg" }
Log "=== EXE test start ==="
$proc = Start-Process -FilePath "G:\codex-AI-tools\lan-link\dist\lan-link-input.exe" -ArgumentList "--daemon","input" -PassThru -WindowStyle Hidden
Log "Started PID=$($proc.Id)"
for ($i=0; $i -lt 14; $i++) {
  Start-Sleep -Seconds 5
  $proc.Refresh()
  if ($proc.HasExited) { Log "T+$((($i+1)*5))s EXIT ExitCode=$($proc.ExitCode)"; break }
  else { Log "T+$((($i+1)*5))s alive" }
}
$proc.Refresh()
if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force; Log "Killed after 70s" }
Log "=== EXE test end ==="