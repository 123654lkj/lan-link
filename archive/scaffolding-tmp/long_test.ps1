$ErrorActionPreference = "Stop"
$LogFile = "C:\Users\26063\AppData\Local\Temp\long_test.log"

function Log($msg) {
  Add-Content -Path $LogFile -Value "$(Get-Date -Format 'HH:mm:ss') $msg"
}

# Cleanup any old python procs
Get-Process -Name "python" -ErrorAction SilentlyContinue | Where-Object {
  $_.Path -like "*Python311*" -and $_.MainWindowTitle -eq ""
} | ForEach-Object { try { $_.Kill() } catch {} }
Start-Sleep -Seconds 2

Log "=== Test start ==="
$proc = Start-Process -FilePath "C:\Program Files\Python311\python.exe" -ArgumentList @(
  "G:\codex-AI-tools\lan-link\client_win.py", "input"
) -PassThru -RedirectStandardError "C:\Users\26063\AppData\Local\Temp\ll_test.err" -RedirectStandardOutput "C:\Users\26063\AppData\Local\Temp\ll_test.out" -WindowStyle Hidden

Log "Started PID=$($proc.Id)"

# Wait 60s, check status every 5s
for ($i = 0; $i -lt 12; $i++) {
  Start-Sleep -Seconds 5
  $proc.Refresh()
  if ($proc.HasExited) {
    Log "T+$((($i+1)*5))s EXIT ExitCode=$($proc.ExitCode)"
    break
  } else {
    Log "T+$((($i+1)*5))s alive"
  }
}
$proc.Refresh()
if (-not $proc.HasExited) {
  Log "Killing PID=$($proc.Id) after 60s test"
  Stop-Process -Id $proc.Id -Force
  Start-Sleep -Seconds 2
}
Log "=== Test end ==="