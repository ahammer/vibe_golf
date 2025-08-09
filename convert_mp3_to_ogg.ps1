Param(
    [string]$FfmpegZipUrl = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
)

$ErrorActionPreference = "Stop"

$ffDir = "ffmpeg"
$zipPath = Join-Path $ffDir "ffmpeg.zip"
$ffExe = Join-Path $ffDir "ffmpeg.exe"

if (-not (Test-Path $ffDir)) {
    New-Item -ItemType Directory -Path $ffDir | Out-Null
}

if (-not (Test-Path $ffExe)) {
    Write-Host "Downloading ffmpeg..."
    Invoke-WebRequest -Uri $FfmpegZipUrl -OutFile $zipPath
    Write-Host "Extracting..."
    Expand-Archive -Path $zipPath -DestinationPath $ffDir -Force
    $inner = Get-ChildItem $ffDir -Directory | Select-Object -First 1
    if (-not $inner) {
        throw "Could not locate extracted ffmpeg folder."
    }
    $candidate = Join-Path $inner.FullName "bin\ffmpeg.exe"
    if (-not (Test-Path $candidate)) {
        throw "ffmpeg.exe not found inside extracted archive."
    }
    Copy-Item $candidate $ffExe -Force
    Write-Host "ffmpeg ready."
} else {
    Write-Host "ffmpeg already present."
}

$audioDir = "assets/audio"
if (-not (Test-Path $audioDir)) {
    throw "Audio directory '$audioDir' not found."
}

$mp3s = Get-ChildItem $audioDir -Filter *.mp3
if (-not $mp3s) {
    Write-Host "No mp3 files found to convert."
    exit 0
}

foreach ($f in $mp3s) {
    $out = [System.IO.Path]::ChangeExtension($f.FullName, ".ogg")
    Write-Host "Converting $($f.Name) -> $(Split-Path $out -Leaf)"
    & $ffExe -y -i $f.FullName $out | Out-Null
}

Write-Host "Conversion complete."
