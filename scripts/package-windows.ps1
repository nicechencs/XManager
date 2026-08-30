# Package XManager Windows zip: desktop exe + CLI + .env.example + docs.
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Ui,
    [Parameter(Mandatory = $true)][string]$Cli,
    [string]$Out = "dist"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-ExistingFile([string]$Path, [string]$Label) {
    $full = [System.IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) {
        throw "$Label not found: $full"
    }
    return $full
}

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$uiPath = Resolve-ExistingFile $Ui "UI binary"
$cliPath = Resolve-ExistingFile $Cli "CLI binary"
$envExample = Resolve-ExistingFile (Join-Path $root ".env.example") ".env.example"
$license = Resolve-ExistingFile (Join-Path $root "LICENSE") "LICENSE"
$readmeSrc = Resolve-ExistingFile (Join-Path $root "packaging\README.txt") "packaging README"

$stageName = "XManager-$Version-windows-x64"
$outDir = Join-Path $root $Out
$stage = Join-Path $outDir $stageName
$zipPath = Join-Path $outDir "$stageName.zip"

if (Test-Path -LiteralPath $stage) {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $stage | Out-Null
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

Copy-Item -LiteralPath $uiPath -Destination (Join-Path $stage "XManager.exe")
Copy-Item -LiteralPath $cliPath -Destination (Join-Path $stage "xmanager-cli.exe")
Copy-Item -LiteralPath $envExample -Destination (Join-Path $stage ".env.example")
Copy-Item -LiteralPath $license -Destination (Join-Path $stage "LICENSE")

$readme = "XManager $Version`r`n`r`n" + [System.IO.File]::ReadAllText($readmeSrc)
[System.IO.File]::WriteAllText(
    (Join-Path $stage "README.txt"),
    $readme,
    (New-Object System.Text.UTF8Encoding $false)
)

if (Test-Path -LiteralPath $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory(
    $stage,
    $zipPath,
    [System.IO.Compression.CompressionLevel]::Optimal,
    $true
)

Write-Host "Wrote $zipPath"
Get-ChildItem -LiteralPath $stage | ForEach-Object { Write-Host ("  {0}`t{1}" -f $_.Name, $_.Length) }
