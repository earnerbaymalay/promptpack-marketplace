Add-Type -AssemblyName System.Drawing
$icon = [System.Drawing.Icon]::ExtractAssociatedIcon('C:\Windows\System32\notepad.exe')
$fs = New-Object System.IO.FileStream('icon.ico', [System.IO.FileMode]::Create)
$icon.Save($fs)
$fs.Close()
Write-Host "Created icon.ico"
Test-Path 'icon.ico'