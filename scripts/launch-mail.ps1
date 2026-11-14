param(
    [string]$Exe = ""
)

$ErrorActionPreference = "Stop"

$source = if ($Exe -ne "") {
    (Resolve-Path $Exe).Path
} else {
    (Resolve-Path (Join-Path $PSScriptRoot "..\target\release\mail.exe")).Path
}

# Windows only allows a Low-Integrity process to run without a tray icon.
# Launching from the workspace (which DSH may label Low) can therefore hide the
# tray icon; copy the binary to a normal user location before launching.
$targetDir = Join-Path $env:LOCALAPPDATA "mail-app"
New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
$target = Join-Path $targetDir "mail.exe"
# Remove first: overwriting a Low-Integrity file can preserve its label.
Remove-Item $target -Force -ErrorAction SilentlyContinue
Copy-Item $source $target -Force

Start-Process -FilePath $target
Write-Output ("launched " + $target)
