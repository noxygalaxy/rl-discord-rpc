#!/usr/bin/env bash
set -euo pipefail

# script made by @noxygalaxy
# feel free to review it, it has nothing dangerous

REPO="noxygalaxy/rl-discord-rpc"
SHARE_DIR="$HOME/.local/share/rl-discord-rpc"
BIN_DIR="$HOME/.local/bin"
CONFIG_PATH="$SHARE_DIR/config.json"
DISCORD_CLIENT_ID="1540801470503329932"

prompt() {
  local __resultvar="$1"
  local __question="$2"
  local __default="${3:-}"
  local __answer=""

  if [[ -r /dev/tty ]]; then
    read -rp "$__question" __answer < /dev/tty
  else
    echo "$__question(no terminal available, using default: '${__default}')"
    __answer=""
  fi

  if [[ -z "$__answer" ]]; then
    __answer="$__default"
  fi

  printf -v "$__resultvar" '%s' "$__answer"
}

kill_running() {
  pkill -9 -f rl_rpc_wrapper 2>/dev/null || true
  pkill -9 -f rl_discord_rpc 2>/dev/null || true
  sleep 1
}

latest_tag() {
  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name":' \
    | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/'
}

enable_stats_api() {
  local port="$1"
  local ans prefix_path

  echo ""
  prompt ans "Turn on the Rocket League Stats API now via this script? [Y/n]: " "Y"
  if [[ ! "$ans" =~ ^[Yy]$ ]]; then
    echo "Skipped. You can enable it manually later -- see the README."
    return
  fi

  echo ""
  echo "This needs the path to Rocket League's Proton prefix."
  echo "(In Heroic: click Rocket League -> the vertical dots menu ->"
  echo " \"Open Wine Prefix\" or \"Open Container Folder\" -- copy that path.)"
  echo ""
  prompt prefix_path "Enter your Rocket League Proton prefix path: " ""

  if [ -z "$prefix_path" ] || [ ! -d "$prefix_path" ]; then
    echo "WARNING: that path doesn't exist or wasn't provided. Skipping auto-configure."
    echo "You'll need to edit TAStatsAPI.ini manually -- see the README."
    return
  fi

  local config_dir="$prefix_path/drive_c/users/steamuser/My Documents/My Games/Rocket League/TAGame/Config"

  if [ ! -d "$config_dir" ]; then
    echo "WARNING: expected config folder not found at:"
    echo "  $config_dir"
    echo "Rocket League may not have been launched at least once yet with this prefix."
    echo "Skipping auto-configure -- you'll need to edit the .ini manually. See the README."
    return
  fi

  local ini_path="$config_dir/TAStatsAPI.ini"
  if [ ! -f "$ini_path" ]; then
    ini_path="$config_dir/DefaultStatsAPI.ini"
  fi

  echo "> Writing Stats API config to: $ini_path"
  cat > "$ini_path" <<EOF
[TAGame.MatchStatsExporter_TA]
Port=${port}
WebPort=$((port + 1))
PacketSendRate=30

[IniVersion]
0=1786011205.000000
EOF

  echo "> Done. Fully quit Rocket League if it's running, then relaunch for this to take effect."
}

do_install() {
  echo "> Stopping any running instances ..."
  kill_running

  echo "> Detecting latest release for $REPO ..."
  local tag
  tag=$(latest_tag)
  if [ -z "$tag" ]; then
    echo "ERROR: could not determine latest release tag. Check the REPO variable in this script." >&2
    exit 1
  fi
  echo "> Latest release: $tag"

  local asset="rl_discord_rpc-linux-x86_64.tar.gz"
  local url="https://github.com/${REPO}/releases/download/${tag}/${asset}"
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "$tmp_dir"' RETURN

  echo "> Downloading ${asset} ..."
  if [[ -t 1 ]]; then
    curl -fL --progress-bar "$url" -o "$tmp_dir/$asset"
  else
    curl -fsSL "$url" -o "$tmp_dir/$asset"
  fi

  echo "> Extracting ..."
  tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"

  mkdir -p "$SHARE_DIR" "$BIN_DIR"

  echo "> Installing real binaries to $SHARE_DIR ..."
  cp "$tmp_dir/rl_discord_rpc" "$SHARE_DIR/rl_discord_rpc"
  cp "$tmp_dir/rl_rpc_wrapper" "$SHARE_DIR/rl_rpc_wrapper"
  chmod +x "$SHARE_DIR/rl_discord_rpc" "$SHARE_DIR/rl_rpc_wrapper"

  echo "> Linking into $BIN_DIR (for PATH / Heroic wrapper command) ..."
  ln -sf "$SHARE_DIR/rl_discord_rpc" "$BIN_DIR/rl_discord_rpc"
  ln -sf "$SHARE_DIR/rl_rpc_wrapper" "$BIN_DIR/rl_rpc_wrapper"

  local stats_port=49123
  if [ -f "$CONFIG_PATH" ]; then
    echo "> Existing config.json found at $CONFIG_PATH, leaving it untouched."
  else
    write_config
    stats_port="$LAST_STATS_PORT"
  fi

  enable_stats_api "$stats_port"

  echo ""
  echo "Done. Installed:"
  echo "  $SHARE_DIR/rl_discord_rpc  (real binary)"
  echo "  $SHARE_DIR/rl_rpc_wrapper  (real binary)"
  echo "  $BIN_DIR/rl_discord_rpc    (symlink)"
  echo "  $BIN_DIR/rl_rpc_wrapper    (symlink)"
  echo "  $CONFIG_PATH"
  echo ""
  echo "Next step: in Heroic, Rocket League -> Settings -> Advanced -> Wrapper command ->"
  echo "  $BIN_DIR/rl_rpc_wrapper"
  echo ""
  if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo "NOTE: $BIN_DIR is not in your PATH. Add this to your shell rc file:"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
  fi
}

write_config() {
  local tracker_platform tracker_username stats_port

  echo "> Configuring rl_discord_rpc ..."
  prompt tracker_platform "Tracker Network platform [epic/steam/psn/xbl] (default: epic): " "epic"
  prompt tracker_username "Tracker Network username (e.g. your Epic display name): " ""
  prompt stats_port "Stats API port (default: 49123): " "49123"
  LAST_STATS_PORT="$stats_port"

  mkdir -p "$SHARE_DIR"
  cat > "$CONFIG_PATH" <<EOF
{
  "discord_client_id": "${DISCORD_CLIENT_ID}",
  "tracker_platform": "${tracker_platform}",
  "tracker_username": "${tracker_username}",
  "stats_api_port": ${stats_port}
}
EOF
  echo "> Wrote $CONFIG_PATH"
}

do_configure() {
  local ans ans2

  if [ ! -d "$SHARE_DIR" ]; then
    echo "rl_discord_rpc doesn't appear to be installed yet ($SHARE_DIR not found)."
    prompt ans "Run install first? [Y/n]: " "Y"
    if [[ "$ans" =~ ^[Yy]$ ]]; then
      do_install
      return
    else
      exit 1
    fi
  fi

  if [ -f "$CONFIG_PATH" ]; then
    echo "> Current config.json:"
    cat "$CONFIG_PATH"
    echo ""
    prompt ans2 "Overwrite with new values? [y/N]: " "N"
    if [[ ! "$ans2" =~ ^[Yy]$ ]]; then
      echo "Cancelled."
      return
    fi
  fi

  kill_running
  write_config
  enable_stats_api "$LAST_STATS_PORT"
  echo "Done. Restart rl_discord_rpc (or relaunch via Heroic) for changes to take effect."
}

do_uninstall() {
  local ans ans2

  echo "This will remove:"
  echo "  $SHARE_DIR"
  echo "  $BIN_DIR/rl_discord_rpc (symlink)"
  echo "  $BIN_DIR/rl_rpc_wrapper (symlink)"
  prompt ans "Are you sure? [y/N]: " "N"
  if [[ ! "$ans" =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    return
  fi

  echo "> Stopping any running instances ..."
  kill_running

  echo "> Removing symlinks ..."
  rm -f "$BIN_DIR/rl_discord_rpc" "$BIN_DIR/rl_rpc_wrapper"

  prompt ans2 "Also delete your config.json (Discord ID / Tracker username)? [y/N]: " "N"
  if [[ "$ans2" =~ ^[Yy]$ ]]; then
    echo "> Removing $SHARE_DIR entirely ..."
    rm -rf "$SHARE_DIR"
  else
    echo "> Removing binaries but keeping config.json in $SHARE_DIR ..."
    rm -f "$SHARE_DIR/rl_discord_rpc" "$SHARE_DIR/rl_rpc_wrapper"
  fi

  echo ""
  echo "Uninstalled. Remember to remove the Wrapper command entry in Heroic"
  echo "(Rocket League -> Settings -> Advanced -> Wrapper command) if you added one."
}

show_menu() {
  local choice

  echo "rl_discord_rpc manager"
  echo "------/ by @noxygalaxy"
  echo "1) Install / Update"
  echo "2) Configure"
  echo "3) Uninstall"
  echo "4) Quit"
  prompt choice "Choose an option [1-4]: " "1"
  case "$choice" in
    1) do_install ;;
    2) do_configure ;;
    3) do_uninstall ;;
    4) exit 0 ;;
    *) echo "Invalid choice."; show_menu ;;
  esac
}

LAST_STATS_PORT=49123

case "${1:-}" in
  install) do_install ;;
  configure) do_configure ;;
  uninstall) do_uninstall ;;
  "") show_menu ;;
  *)
    echo "Usage: $0 [install|configure|uninstall]"
    exit 1
    ;;
esac