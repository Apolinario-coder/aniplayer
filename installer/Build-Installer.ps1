param([string]$RuntimeRoot = (Join-Path $PSScriptRoot '..\..'))

$ErrorActionPreference = 'Stop'
$project = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$RuntimeRoot = (Resolve-Path -LiteralPath $RuntimeRoot).Path
$cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
if ($cargo) {
    $cargoPath = $cargo.Source
} else {
    $toolchain = Join-Path $env:LOCALAPPDATA 'AniPlayerRust'
    $env:CARGO_HOME = Join-Path $toolchain 'cargo'
    $env:RUSTUP_HOME = Join-Path $toolchain 'rustup'
    $cargoPath = Join-Path $env:CARGO_HOME 'bin\cargo.exe'
}
& $cargoPath build --release --locked --bins --features windows-setup --manifest-path (Join-Path $project 'Cargo.toml')
if ($LASTEXITCODE) { throw "Cargo retornou $LASTEXITCODE." }

$release = Join-Path $project 'target\release'
$work = Join-Path $project ('target\setup-' + [guid]::NewGuid().ToString('N'))
$stage = Join-Path $work 'payload'
$package = Join-Path $project 'Instalar Ani Player.exe'
New-Item -ItemType Directory -Path $stage | Out-Null
try {
    Copy-Item -LiteralPath (Join-Path $release 'ani-player.exe') -Destination (Join-Path $stage 'Ani Player.exe')
    Copy-Item -LiteralPath (Join-Path $release 'yt-dlp.exe') -Destination $stage

    function Copy-Runtime([string]$RelativePath) {
        $to = Join-Path $stage $RelativePath
        New-Item -ItemType Directory -Force -Path (Split-Path $to) | Out-Null
        Copy-Item -LiteralPath (Join-Path $RuntimeRoot $RelativePath) -Destination $to -Recurse
    }
    foreach ($file in (Get-ChildItem (Join-Path $RuntimeRoot 'mpv64') -File)) {
        if ($file.Name -match '^(mpv\.exe|mfplat\.dll|python.*\.(exe|dll|zip|_pth)|lib(crypto|ssl|ffi).*\.dll|sqlite3\.dll|vapoursynth\.dll|vsscript\.dll|vspipe\.exe|.*\.pyd)$') {
            Copy-Runtime "mpv64\$($file.Name)"
        }
    }
    foreach ($path in @(
        'mpv64\lib\site-packages\vapoursynth', 'utils\ffmpeg.exe',
        'rife\rife_vs.dll', 'rife\models\rife-v4.16lite',
        'extensions\tube\yt_dlp', 'extensions\tube\yt_dlp_ejs', 'extensions\tube\deno.exe',
        'licenses\source_codes.txt', 'licenses\gpl_for_used_libs.txt', 'licenses\lgpl_for_used_libs.txt',
        'licenses\ffmpeg-lgpl.txt', 'licenses\Python license.txt', 'licenses\RIFE_license.txt',
        'licenses\yt_dlp_license.txt', 'licenses\deno_license.txt'
    )) { Copy-Runtime $path }
    Copy-Item -LiteralPath (Join-Path $project 'assets\anime4k\LICENSE') -Destination (Join-Path $stage 'licenses\Anime4K.txt')
    Copy-Item -LiteralPath (Join-Path $project 'assets\i18n\Noto-OFL.txt') -Destination (Join-Path $stage 'licenses')
    Copy-Item -LiteralPath (Join-Path $project 'assets\i18n\messages.txt') -Destination (Join-Path $stage 'languages.txt')

    $archive = Join-Path $work 'payload.zip'
    & tar.exe -a -cf $archive -C $stage .
    if ($LASTEXITCODE) { throw "Falha ao empacotar arquivos ($LASTEXITCODE)." }

    $output = Join-Path $work 'setup.exe'
    Copy-Item -LiteralPath (Join-Path $release 'ani-setup.exe') -Destination $output
    $target = [IO.File]::Open($output, [IO.FileMode]::Append)
    try {
        $input = [IO.File]::OpenRead($archive)
        try {
            $input.CopyTo($target)
            $target.Write([BitConverter]::GetBytes([uint64]$input.Length), 0, 8)
            $target.Write([Text.Encoding]::ASCII.GetBytes('ANISETUP'), 0, 8)
        } finally {
            $input.Dispose()
        }
    } finally {
        $target.Dispose()
    }
    Move-Item -LiteralPath $output -Destination $package -Force
    Get-Item -LiteralPath $package
} finally {
    $resolved = (Resolve-Path -LiteralPath $work).Path
    if ((Split-Path $resolved) -eq (Join-Path $project 'target')) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
