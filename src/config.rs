use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_discord_client_id")]
    pub discord_client_id: String,

    #[serde(default = "default_tracker_platform")]
    pub tracker_platform: String,

    #[serde(default)]
    pub tracker_username: String,

    #[serde(default = "default_stats_api_port")]
    pub stats_api_port: u16,
}

fn default_discord_client_id() -> String {
    "YOUR_DISCORD_APPLICATION_ID".to_string()
}

fn default_tracker_platform() -> String {
    "epic".to_string()
}

fn default_stats_api_port() -> u16 {
    49123
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            discord_client_id: default_discord_client_id(),
            tracker_platform: default_tracker_platform(),
            tracker_username: String::new(),
            stats_api_port: default_stats_api_port(),
        }
    }
}

fn config_path() -> PathBuf {
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("config.json")
}

pub fn load_or_init() -> AppConfig {
    let path = config_path();

    if !path.exists() {
        eprintln!(
            "rl_discord_rpc: no config.json found at {}, writing a default template.",
            path.display()
        );
        eprintln!("rl_discord_rpc: edit it with your Discord Client ID and Tracker username, then restart.");
        let default = AppConfig::default();
        if let Ok(json) = serde_json::to_string_pretty(&default_template_json()) {
            let _ = fs::write(&path, json);
        }
        return default;
    }

    match fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<AppConfig>(&contents) {
            Ok(cfg) => {
                if cfg.discord_client_id == default_discord_client_id() {
                    eprintln!("rl_discord_rpc: WARNING: discord_client_id in config.json is still the placeholder value.");
                }
                if cfg.tracker_username.is_empty() {
                    eprintln!("rl_discord_rpc: WARNING: tracker_username in config.json is empty; rank icon will be disabled.");
                }
                cfg
            }
            Err(e) => {
                eprintln!(
                    "rl_discord_rpc: failed to parse {}: {e}. Using defaults.",
                    path.display()
                );
                AppConfig::default()
            }
        },
        Err(e) => {
            eprintln!(
                "rl_discord_rpc: failed to read {}: {e}. Using defaults.",
                path.display()
            );
            AppConfig::default()
        }
    }
}

fn default_template_json() -> serde_json::Value {
    serde_json::json!({
        "discord_client_id": default_discord_client_id(),
        "tracker_platform": default_tracker_platform(),
        "tracker_username": "",
        "stats_api_port": default_stats_api_port()
    })
}
