param(
    [ValidateSet("install", "configure", "uninstall", "")]
    [string]$Action = ""
)

$ErrorActionPreference = "Stop"

$Repo = "noxygalaxy/rl-discord-rpc"
$InstallDir = Join-Path $env:LOCALAPPDATA "rl-discord-rpc"
$ConfigPath = Join-Path $InstallDir "config.json"
$DiscordClientId = "1540801470503329932"

function Stop-RunningInstances {
    Get-Process -Name "rl_discord_rpc" -ErrorAction SilentlyContinue | Stop-Process -Force
    Get-Process -Name "rl_rpc_wrapper" -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 1
}

function Get-LatestTag {
    $releaseInfo = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    return $releaseInfo.tag_name
}

function Enable-StatsApi {
    param([int]$Port)

    Write-Host ""
    $ans = Read-Host "Turn on the Rocket League Stats API now via this script? [Y/n]"
    if ($ans -match '^[Nn]') {
        Write-Host "Skipped. You can enable it manually later -- see the README."
        return
    }

    $configDir = Join-Path $env:USERPROFILE "Documents\My Games\Rocket League\TAGame\Config"

    if (-not (Test-Path $configDir)) {
        Write-Host "WARNING: expected config folder not found at:"
        Write-Host "  $configDir"
        Write-Host "Rocket League may need to be launched at least once first."
        Write-Host "Skipping auto-configure -- see the README to do this manually."
        return
    }

    $iniPath = Join-Path $configDir "TAStatsAPI.ini"
    if (-not (Test-Path $iniPath)) {
        $iniPath = Join-Path $configDir "DefaultStatsAPI.ini"
    }

    Write-Host "> Writing Stats API config to: $iniPath"
    $webPort = $Port + 1
    $iniContent = @"
[TAGame.MatchStatsExporter_TA]
Port=$Port
WebPort=$webPort
PacketSendRate=30

[IniVersion]
0=1786011205.000000
"@
    Set-Content -Path $iniPath -Value $iniContent -Encoding ascii

    Write-Host "> Done. Fully quit Rocket League if it's running, then relaunch for this to take effect."
}

function Write-Config {
    Write-Host "> Configuring rl_discord_rpc ..."
    $trackerPlatform = Read-Host "Tracker Network platform [epic/steam/psn/xbl] (default: epic)"
    if ([string]::IsNullOrWhiteSpace($trackerPlatform)) { $trackerPlatform = "epic" }
    $trackerUsername = Read-Host "Tracker Network username (e.g. your Epic display name)"
    $statsPortInput = Read-Host "Stats API port (default: 49123)"
    if ([string]::IsNullOrWhiteSpace($statsPortInput)) { $statsPortInput = "49123" }
    $script:LastStatsPort = [int]$statsPortInput

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

    $configJson = @"
{
  "discord_client_id": "$DiscordClientId",
  "tracker_platform": "$trackerPlatform",
  "tracker_username": "$trackerUsername",
  "stats_api_port": $($script:LastStatsPort)
}
"@
    Set-Content -Path $ConfigPath -Value $configJson -Encoding utf8
    Write-Host "> Wrote $ConfigPath"
}

function Invoke-Install {
    Write-Host "> Stopping any running instances ..."
    Stop-RunningInstances

    Write-Host "> Detecting latest release for $Repo ..."
    $tag = Get-LatestTag
    if (-not $tag) {
        Write-Error "Could not determine latest release tag. Check the `$Repo` variable in this script."
        return
    }
    Write-Host "> Latest release: $tag"

    $assetName = "rl_discord_rpc-windows-x86_64.zip"
    $downloadUrl = "https://github.com/$Repo/releases/download/$tag/$assetName"
    $tmpDir = Join-Path $env:TEMP ("rl_discord_rpc_install_" + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

    try {
        $zipPath = Join-Path $tmpDir $assetName
        Write-Host "> Downloading $assetName ..."
        Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath

        Write-Host "> Extracting ..."
        Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force

        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

        Write-Host "> Installing binaries to $InstallDir ..."
        Copy-Item (Join-Path $tmpDir "rl_discord_rpc.exe") (Join-Path $InstallDir "rl_discord_rpc.exe") -Force
        Copy-Item (Join-Path $tmpDir "rl_rpc_wrapper.exe") (Join-Path $InstallDir "rl_rpc_wrapper.exe") -Force

        $script:LastStatsPort = 49123
        if (Test-Path $ConfigPath) {
            Write-Host "> Existing config.json found at $ConfigPath, leaving it untouched."
        }
        else {
            Write-Config
        }

        Enable-StatsApi -Port $script:LastStatsPort

        Write-Host ""
        Write-Host "Done. Installed:"
        Write-Host "  $InstallDir\rl_discord_rpc.exe"
        Write-Host "  $InstallDir\rl_rpc_wrapper.exe"
        Write-Host "  $ConfigPath"
        Write-Host ""
        Write-Host "Next step: in Heroic, Rocket League -> Settings -> Advanced -> Wrapper command ->"
        Write-Host "  $InstallDir\rl_rpc_wrapper.exe"
    }
    finally {
        Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
    }
}

function Invoke-Configure {
    if (-not (Test-Path $InstallDir)) {
        Write-Host "rl_discord_rpc doesn't appear to be installed yet ($InstallDir not found)."
        $ans = Read-Host "Run install first? [Y/n]"
        if ([string]::IsNullOrWhiteSpace($ans) -or $ans -match '^[Yy]') {
            Invoke-Install
            return
        }
        else {
            return
        }
    }

    if (Test-Path $ConfigPath) {
        Write-Host "> Current config.json:"
        Get-Content $ConfigPath | Write-Host
        Write-Host ""
        $ans = Read-Host "Overwrite with new values? [y/N]"
        if ($ans -notmatch '^[Yy]') {
            Write-Host "Cancelled."
            return
        }
    }

    Stop-RunningInstances
    Write-Config
    Enable-StatsApi -Port $script:LastStatsPort
    Write-Host "Done. Restart rl_discord_rpc (or relaunch via Heroic) for changes to take effect."
}

function Invoke-Uninstall {
    Write-Host "This will remove: $InstallDir"
    $ans = Read-Host "Are you sure? [y/N]"
    if ($ans -notmatch '^[Yy]') {
        Write-Host "Cancelled."
        return
    }

    Write-Host "> Stopping any running instances ..."
    Stop-RunningInstances

    $ans2 = Read-Host "Also delete your config.json (Discord ID / Tracker username)? [y/N]"
    if ($ans2 -match '^[Yy]') {
        Write-Host "> Removing $InstallDir entirely ..."
        Remove-Item -Recurse -Force $InstallDir -ErrorAction SilentlyContinue
    }
    else {
        Write-Host "> Removing binaries but keeping config.json ..."
        Remove-Item -Force (Join-Path $InstallDir "rl_discord_rpc.exe") -ErrorAction SilentlyContinue
        Remove-Item -Force (Join-Path $InstallDir "rl_rpc_wrapper.exe") -ErrorAction SilentlyContinue
    }

    Write-Host ""
    Write-Host "Uninstalled. Remember to remove the Wrapper command entry in Heroic"
    Write-Host "(Rocket League -> Settings -> Advanced -> Wrapper command) if you added one."
}

function Show-Menu {
    Write-Host "rl_discord_rpc manager"
    Write-Host "------/ by @noxygalaxy"
    Write-Host "1) Install / Update"
    Write-Host "2) Configure"
    Write-Host "3) Uninstall"
    Write-Host "4) Quit"
    $choice = Read-Host "Choose an option [1-4]"
    switch ($choice) {
        "1" { Invoke-Install }
        "2" { Invoke-Configure }
        "3" { Invoke-Uninstall }
        "4" { return }
        default {
            Write-Host "Invalid choice."
            Show-Menu
        }
    }
}

$script:LastStatsPort = 49123

switch ($Action) {
    "install" { Invoke-Install }
    "configure" { Invoke-Configure }
    "uninstall" { Invoke-Uninstall }
    default { Show-Menu }
}
