param(
    [Parameter(Mandatory)][string]$Destination,
    [Parameter(Mandatory)][string]$Cache,
    [Parameter(Mandatory)][string]$Scratch
)

$ErrorActionPreference = 'Stop'
$manifest = Join-Path $PSScriptRoot 'windows-runtime.txt'
New-Item -ItemType Directory -Force -Path $Cache, $Scratch | Out-Null
foreach ($row in (Get-Content -LiteralPath $manifest)) {
    $name, $sha, $url = $row.Split('|')
    $file = Join-Path $Cache $name
    if (!(Test-Path -LiteralPath $file) -or (Get-FileHash -LiteralPath $file).Hash -ne $sha) {
        & curl.exe -fLsS --retry 3 $url -o $file
        if ($LASTEXITCODE -or (Get-FileHash -LiteralPath $file).Hash -ne $sha) {
            throw "Download or checksum failed for $name."
        }
    }
}

function Expand-RuntimeArchive([string]$Archive, [string]$Directory, [string[]]$Entries) {
    if ([IO.Path]::GetExtension($Archive) -eq '.7z') {
        $sevenZip = Get-Command 7z.exe -ErrorAction SilentlyContinue
        if (!$sevenZip) { throw 'Install 7-Zip to extract the media build archives.' }
        & $sevenZip.Source x -y "-o$Directory" $Archive @Entries | Out-Null
    } else {
        & tar.exe -xf $Archive -C $Directory @Entries
    }
    if ($LASTEXITCODE) { throw "Could not extract $Archive." }
}

$mpv = Join-Path $Destination 'mpv64'
$utils = Join-Path $Destination 'utils'
$site = Join-Path $mpv 'Lib/site-packages'
$model = Join-Path $Destination 'rife/models/rife-v4.16lite'
$licenses = Join-Path $Destination 'licenses'
New-Item -ItemType Directory -Force -Path $mpv, $utils, $site, $model, $licenses | Out-Null
Expand-RuntimeArchive (Join-Path $Cache 'mpv.7z') $mpv @('mpv.exe', 'mpv.com')
Expand-RuntimeArchive (Join-Path $Cache 'ffmpeg.7z') $utils @('ffmpeg.exe')
Expand-RuntimeArchive (Join-Path $Cache 'python.zip') $mpv
Expand-RuntimeArchive (Join-Path $Cache 'vs.zip') $Scratch @('wheel/vapoursynth-79-cp312-abi3-win_amd64.whl')
Expand-RuntimeArchive (Join-Path $Scratch 'wheel/vapoursynth-79-cp312-abi3-win_amd64.whl') $site
Add-Content -LiteralPath (Join-Path $mpv 'python312._pth') -Value 'Lib\site-packages' -Encoding ascii
Get-ChildItem -LiteralPath $site -Recurse -File -Filter '*.pdb' | ForEach-Object {
    Remove-Item -LiteralPath $_.FullName
}
Copy-Item -LiteralPath (Join-Path $Cache 'rife_vs.dll') -Destination (Join-Path $Destination 'rife')
Copy-Item -LiteralPath (Join-Path $Cache 'flownet.bin'), (Join-Path $Cache 'flownet.param') -Destination $model
Copy-Item -LiteralPath (Join-Path $Cache 'yt-dlp.exe') -Destination $Destination
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$visualStudio = & $vswhere -latest -products '*' -property installationPath
if ($LASTEXITCODE -or !$visualStudio) { throw 'Visual Studio redistributables are required to build the complete installer.' }
$redist = Get-ChildItem -LiteralPath (Join-Path $visualStudio 'VC/Redist/MSVC') -Directory |
    Where-Object { $_.Name -match '^14\.\d+\.\d+$' } |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
foreach ($group in @('Microsoft.VC143.CRT', 'Microsoft.VC143.OpenMP')) {
    $files = Get-ChildItem -LiteralPath (Join-Path $redist.FullName "x64/$group") -Filter '*.dll' -File
    if (!$files) { throw "Missing Visual C++ redistributable group $group." }
    foreach ($dll in $files) {
        Copy-Item -LiteralPath $dll.FullName -Destination $mpv -Force
        Copy-Item -LiteralPath $dll.FullName -Destination $Destination -Force
    }
}
Copy-Item -LiteralPath (Join-Path $Cache 'mpv-Copyright.txt'), (Join-Path $Cache 'mpv-GPL.txt'),
    (Join-Path $Cache 'FFmpeg-COPYING.GPLv3.txt'), (Join-Path $Cache 'RIFE-LICENSE.txt'),
    (Join-Path $Cache 'yt-dlp-LICENSE.txt'), $manifest, (Join-Path $mpv 'LICENSE.txt') -Destination $licenses
