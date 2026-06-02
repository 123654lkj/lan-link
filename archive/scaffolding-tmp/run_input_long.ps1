$ErrorActionPreference = "Stop"
$pid_proc = Start-Process -FilePath "C:\Program Files\Python311\python.exe" -ArgumentList "G:\codex-AI-tools\lan-link\client_win.py", "input" -PassThru -RedirectStandardError "C:\Users\26063\AppData\Local\Temp\ll_long.err" -RedirectStandardOutput "C:\Users\26063\AppData\Local\Temp\ll_long.out"
Write-Host "Started PID=$($pid_proc.Id) at $(Get-Date -Format 'HH:mm:ss')"
$pid_proc | Out-File -FilePath "C:\Users\26063\AppData\Local\Temp\ll_long.pid" -Encoding ASCII