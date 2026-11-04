param([string]$Archive, [string]$Version, [switch]$Desktop, [switch]$Uninstall)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)
$marker = '.ani-player-install'
$registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\AniPlayerAnime'
$locationFile = Join-Path $env:LOCALAPPDATA 'Ani Player Anime\install-location.txt'
$shell = New-Object -ComObject WScript.Shell
$startLink = Join-Path $shell.SpecialFolders.Item('Programs') 'Ani Player.lnk'
$desktopLink = Join-Path $shell.SpecialFolders.Item('Desktop') 'Ani Player.lnk'
$mutex = New-Object Threading.Mutex($false, 'Local\AniPlayerAnimeSetup')
$locked = $false

function New-AniShortcut([string]$Link) {
    if (Test-Path -LiteralPath $Link) {
        $existing = $shell.CreateShortcut($Link)
        if ($existing.TargetPath -ne $player) { return }
    }
    $shortcut = $shell.CreateShortcut($Link)
    $shortcut.TargetPath = $player
    $shortcut.WorkingDirectory = $destination
    $shortcut.Description = 'Ani Player'
    $shortcut.IconLocation = "$player,0"
    $shortcut.Save()
}

function Remove-AniDirectory([string]$Directory) {
    $resolved = [IO.Path]::GetFullPath($Directory)
    if ([IO.Path]::GetDirectoryName($resolved) -ne $programs) {
        throw 'A pasta de instalação é inválida.'
    }
    if (Test-Path -LiteralPath $resolved) {
        if ((Get-Item -LiteralPath $resolved -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw 'A pasta de instalação é um link para outro local.'
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

function Expand-AniArchive([string]$Package, [string]$Directory) {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($Package)
    try {
        $total = [long](($zip.Entries | Measure-Object -Property Length -Sum).Sum)
        if ($total -le 0) { throw 'O pacote está vazio.' }
        [long]$copied = 0
        $last = 10
        $buffer = New-Object byte[] (1MB)
        $prefix = $Directory + [IO.Path]::DirectorySeparatorChar
        foreach ($entry in $zip.Entries) {
            $file = [IO.Path]::GetFullPath((Join-Path $Directory $entry.FullName))
            if ($file.TrimEnd('\') -eq $Directory -and !$entry.Name) { continue }
            if (!$file.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
                throw 'O pacote contém um caminho inválido.'
            }
            if (!$entry.Name) {
                [void][IO.Directory]::CreateDirectory($file)
                continue
            }
            [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($file))
            $inputStream = $entry.Open()
            try {
                $outputStream = [IO.File]::Open($file, [IO.FileMode]::CreateNew)
                try {
                    while (($count = $inputStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
                        $outputStream.Write($buffer, 0, $count)
                        $copied += $count
                        $percent = 10 + [int][Math]::Floor(79 * $copied / $total)
                        if ($percent -gt $last) {
                            [Console]::WriteLine($percent)
                            $last = $percent
                        }
                    }
                } finally { $outputStream.Dispose() }
            } finally { $inputStream.Dispose() }
        }
    } finally { $zip.Dispose() }
}

try {
    try { $locked = $mutex.WaitOne(0) } catch [Threading.AbandonedMutexException] { $locked = $true }
    if (!$locked) { throw 'Já há outra instalação em andamento.' }
    $registered = (Get-ItemProperty -LiteralPath $registry -ErrorAction SilentlyContinue).InstallLocation
    if ($Uninstall) {
        $destination = [IO.Path]::GetFullPath($PSScriptRoot)
        if ([IO.Path]::GetFileName($destination) -ne 'Ani Player' -or $registered -ne $destination) {
            throw 'A pasta não corresponde à instalação registrada no Windows.'
        }
        $programs = [IO.Path]::GetDirectoryName($destination)
    } else {
        if (!$env:ANI_SETUP_PARENT -or ![IO.Path]::IsPathRooted($env:ANI_SETUP_PARENT)) {
            throw 'Escolha uma pasta de instalação válida.'
        }
        $destination = [IO.Path]::GetFullPath((Join-Path $env:ANI_SETUP_PARENT 'Ani Player'))
        $programs = [IO.Path]::GetDirectoryName($destination)
        if ($registered -and $registered -ne $destination -and (Test-Path -LiteralPath $registered)) {
            throw 'Para mudar de local, desinstale o Ani Player em Aplicativos instalados e execute o instalador novamente.'
        }
    }
    $player = Join-Path $destination 'Ani Player.exe'
    if (Test-Path -LiteralPath $destination) {
        $item = Get-Item -LiteralPath $destination -Force
        if (!$item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            !(Test-Path -LiteralPath (Join-Path $destination $marker) -PathType Leaf)) {
            throw 'A pasta de destino já existe e pertence a outra instalação.'
        }
    }
    $running = Get-Process -Name 'Ani Player' -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $player }
    if ($running) { throw 'Feche o Ani Player e tente novamente.' }

    if ($Uninstall) {
        Remove-AniDirectory $destination
        foreach ($link in @($startLink, $desktopLink)) {
            if ((Test-Path -LiteralPath $link) -and $shell.CreateShortcut($link).TargetPath -eq $player) {
                Remove-Item -LiteralPath $link
            }
        }
        if (Test-Path -LiteralPath $registry) { Remove-Item -LiteralPath $registry }
        if (Test-Path -LiteralPath $locationFile) { Remove-Item -LiteralPath $locationFile }
        return
    }

    New-Item -ItemType Directory -Force -Path $programs | Out-Null
    $id = [guid]::NewGuid().ToString('N')
    $staging = Join-Path $programs "Ani Player.stage-$id"
    $backup = Join-Path $programs "Ani Player.backup-$id"
    try {
        Expand-AniArchive $Archive $staging
        [Console]::WriteLine(90)
        $required = @('Ani Player.exe')
        if (Test-Path -LiteralPath (Join-Path $staging '.bundled-runtime')) {
            $required += @('mpv64\mpv.exe', 'utils\ffmpeg.exe', 'rife\rife_vs.dll')
        } elseif (Test-Path -LiteralPath (Join-Path $destination 'mpv64\mpv.exe')) {
            # Keep the optional local runtime when updating an existing offline installation.
            foreach ($entry in @('mpv64', 'utils', 'rife', 'extensions', 'yt-dlp.exe', '.bundled-runtime')) {
                $source = Join-Path $destination $entry
                if (Test-Path -LiteralPath $source) {
                    Copy-Item -LiteralPath $source -Destination $staging -Recurse
                }
            }
            $oldNotices = Join-Path $destination 'licenses'
            if (Test-Path -LiteralPath $oldNotices) {
                foreach ($notice in (Get-ChildItem -LiteralPath $oldNotices -File)) {
                    $copy = Join-Path (Join-Path $staging 'licenses') $notice.Name
                    if (!(Test-Path -LiteralPath $copy)) {
                        Copy-Item -LiteralPath $notice.FullName -Destination $copy
                    }
                }
            }
        }
        foreach ($file in $required) {
            if (!(Test-Path -LiteralPath (Join-Path $staging $file) -PathType Leaf)) {
                throw 'Pacote de instalação incompleto.'
            }
        }
        Copy-Item -LiteralPath $PSCommandPath -Destination (Join-Path $staging 'Uninstall.ps1')
        New-Item -ItemType File -Path (Join-Path $staging $marker) | Out-Null
        if (Test-Path -LiteralPath $destination) {
            Move-Item -LiteralPath $destination -Destination $backup
        }
        try {
            Move-Item -LiteralPath $staging -Destination $destination
        } catch {
            if (Test-Path -LiteralPath $backup) {
                Move-Item -LiteralPath $backup -Destination $destination
            }
            throw
        }
        Remove-AniDirectory $backup
        New-AniShortcut $startLink
        if ($Desktop) { New-AniShortcut $desktopLink }
        New-Item -Path $registry -Force | Out-Null
        $powershell = Join-Path $PSHOME 'powershell.exe'
        $uninstaller = Join-Path $destination 'Uninstall.ps1'
        $values = @{
            DisplayName = 'Ani Player'
            DisplayVersion = $Version
            DisplayIcon = $player
            InstallLocation = $destination
            UninstallString = ('"{0}" -NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "{1}" -Uninstall' -f $powershell, $uninstaller)
        }
        foreach ($entry in $values.GetEnumerator()) {
            New-ItemProperty -LiteralPath $registry -Name $entry.Key -Value $entry.Value -PropertyType String -Force | Out-Null
        }
        foreach ($name in @('NoModify', 'NoRepair')) {
            New-ItemProperty -LiteralPath $registry -Name $name -Value 1 -PropertyType DWord -Force | Out-Null
        }
        [void][IO.Directory]::CreateDirectory((Split-Path $locationFile))
        [IO.File]::WriteAllText($locationFile, $destination)
        [Console]::WriteLine(99)
    } finally {
        Remove-AniDirectory $staging
    }
} catch {
    $message = $_.Exception.Message
    [Console]::Error.WriteLine($message)
    if ($Uninstall) {
        $titles = @('Uninstall Ani Player', 'Desinstalar Ani Player', 'Desinstalar Ani Player',
            'Désinstaller Ani Player', 'Ani Player deinstallieren', '卸载 Ani Player', 'Ani Player のアンインストール')
        $languages = @('en', 'pt-BR', 'es', 'fr', 'de', 'zh-Hans', 'ja')
        $preference = Join-Path (Split-Path $locationFile) 'language.txt'
        $code = Get-Content -LiteralPath $preference -Encoding UTF8 -ErrorAction SilentlyContinue | Select-Object -First 1
        $language = [Math]::Max(0, [Array]::IndexOf($languages, $code))
        $title = $titles[$language]
        foreach ($line in (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'languages.txt') -Encoding UTF8 -ErrorAction SilentlyContinue)) {
            $row = $line.Split('|')
            if ($row.Length -eq 7 -and $row[1] -eq $message) {
                $message = $row[$language]
                break
            }
        }
        Add-Type -AssemblyName System.Windows.Forms
        [void][Windows.Forms.MessageBox]::Show($message, $title)
    }
    exit 1
} finally {
    if ($locked) { $mutex.ReleaseMutex() }
    $mutex.Dispose()
}
