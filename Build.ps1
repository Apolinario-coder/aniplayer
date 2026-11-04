$ErrorActionPreference = 'Stop'
$cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
if ($cargo) {
    $cargoPath = $cargo.Source
} else {
    $toolchain = Join-Path $env:LOCALAPPDATA 'AniPlayerRust'
    $env:CARGO_HOME = Join-Path $toolchain 'cargo'
    $env:RUSTUP_HOME = Join-Path $toolchain 'rustup'
    $cargoPath = Join-Path $env:CARGO_HOME 'bin\cargo.exe'
    if (!(Test-Path -LiteralPath $cargoPath)) {
        throw 'Instale Rust e coloque cargo.exe no PATH.'
    }
}

& $cargoPath build --release --locked --features windows-setup --manifest-path (Join-Path $PSScriptRoot 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw "Falha ao compilar o Ani Player ($LASTEXITCODE)." }

$release = Join-Path $PSScriptRoot 'target\release'
Copy-Item -LiteralPath (Join-Path $release 'ani-player.exe') -Destination (Join-Path $PSScriptRoot 'Ani Player.exe') -Force
Copy-Item -LiteralPath (Join-Path $release 'yt-dlp.exe') -Destination (Join-Path $PSScriptRoot 'yt-dlp.exe') -Force
