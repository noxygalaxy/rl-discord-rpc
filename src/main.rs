// connector to official rl stats api

mod config;
mod maps;
mod rank;

use config::AppConfig;
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use maps::{map_display_name, resolve_map_image_url};
use rank::{fetch_rank_info, playlist_id as trn_playlist_id, RankInfo};
use rlstatsapi::{ClientOptions, RocketLeagueStatsClient, StatsEvent};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

fn playlist_name_from_id(id: i64) -> &'static str {
    match id {
        0 => "Casual",
        1 => "Duel (Casual 1v1)",
        2 => "Doubles (Casual 2v2)",
        3 => "Standard (Casual 3v3)",
        4 => "Chaos (Casual 4v4)",
        6 => "Private Match",
        9 => "Training",
        10 => "Ranked Solo Duel",
        11 => "Ranked Doubles",
        13 => "Ranked Standard",
        15 => "Snow Day",
        17 => "Hoops",
        18 => "Rumble",
        19 => "Workshop",
        21 => "Custom Training",
        22 => "Tournament",
        23 => "Dropshot",
        27 => "Ranked Hoops",
        28 => "Ranked Rumble",
        30 => "Ranked Snow Day",
        32 => "Beach Ball",
        35 => "Rocket Labs",
        41 => "Boomer Ball",
        48 => "Tactical Rumble",
        49 => "Spring Loaded",
        50 => "Speed Demon",
        54 => "Knockout",
        _ => "Unknown Playlist",
    }
}

fn is_ranked_playlist(internal_playlist_id: i64) -> bool {
    matches!(internal_playlist_id, 10 | 11 | 13 | 27 | 28 | 30)
}

fn trn_playlist_id_from_internal(id: i64) -> i64 {
    match id {
        10 => trn_playlist_id::RANKED_DUEL_1V1,
        11 => trn_playlist_id::RANKED_DOUBLES_2V2,
        13 => trn_playlist_id::RANKED_STANDARD_3V3,
        27 => trn_playlist_id::HOOPS,
        28 => trn_playlist_id::RUMBLE,
        30 => trn_playlist_id::SNOWDAY,
        _ => trn_playlist_id::CASUAL,
    }
}

fn client_options_from_config(cfg: &AppConfig) -> ClientOptions {
    let mut opts = ClientOptions::default();
    opts.port_override = Some(cfg.stats_api_port);
    opts.auto_enable_packet_rate = true;
    opts.set_packet_rate_only_when_zero = true;
    opts
}

async fn connect_with_retry(cfg: &AppConfig) -> RocketLeagueStatsClient {
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        match RocketLeagueStatsClient::connect(client_options_from_config(cfg)).await {
            Ok(client) => {
                eprintln!(
                    "rl_discord_rpc: connected to Stats API on port {} (attempt {})",
                    cfg.stats_api_port, attempt
                );
                return client;
            }
            Err(e) => {
                if attempt == 1 || attempt % 5 == 0 {
                    eprintln!(
                        "rl_discord_rpc: connection attempt {} failed ({e}). Waiting for Rocket League to finish loading...",
                        attempt
                    );
                }
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }
    }
}

struct MatchState {
    playlist: String,
    internal_playlist_id: i64,
    trn_playlist_id: i64,
    map_asset: String,
    map_image_url: String,
    team_score: i64,
    opponent_score: i64,
    match_start_unix: i64,
    in_match: bool,
    rank: Option<RankInfo>,
}

impl MatchState {
    fn fresh() -> Self {
        Self {
            playlist: "Unknown Playlist".to_string(),
            internal_playlist_id: -1,
            trn_playlist_id: trn_playlist_id::CASUAL,
            map_asset: String::new(),
            map_image_url: String::new(),
            team_score: 0,
            opponent_score: 0,
            match_start_unix: now_unix(),
            in_match: false,
            rank: None,
        }
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn send_activity(discord: &mut DiscordIpcClient, state: &MatchState) -> Result<(), Box<dyn Error>> {
    let details = if state.in_match {
        state.playlist.clone()
    } else {
        "In Menus".to_string()
    };
    let state_text = if state.in_match {
        format!("Score: {} - {}", state.team_score, state.opponent_score)
    } else {
        "Idle".to_string()
    };
    let map_display = map_display_name(&state.map_asset);

    let large_image = if state.map_image_url.is_empty() {
        "rl_logo"
    } else {
        state.map_image_url.as_str()
    };

    let mut assets = activity::Assets::new()
        .large_image(large_image)
        .large_text(map_display);

    if state.in_match && is_ranked_playlist(state.internal_playlist_id) {
        if let Some(rank) = &state.rank {
            assets = assets.small_image(&rank.icon_url).small_text(&rank.tier_name);
        }
    }

    let activity = activity::Activity::new()
        .details(&details)
        .state(&state_text)
        .timestamps(activity::Timestamps::new().start(state.match_start_unix))
        .assets(assets);

    discord.set_activity(activity)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cfg = config::load_or_init();
    let http = reqwest::Client::new();
    let mut discord = DiscordIpcClient::new(&cfg.discord_client_id)?;
    loop {
        match discord.connect() {
            Ok(_) => {
                eprintln!("rl_discord_rpc: connected to Discord IPC.");
                break;
            }
            Err(e) => {
                eprintln!("rl_discord_rpc: Discord IPC connect failed ({e}), retrying in 3s...");
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }
    }

    eprintln!("rl_discord_rpc: waiting for Rocket League's Stats API socket...");
    let mut client = connect_with_retry(&cfg).await;

    let mut state = MatchState::fresh();
    send_activity(&mut discord, &state)?;

    loop {
        match client.next_event().await {
            Ok(Some(event)) => handle_event(event, &mut state, &http, &mut discord, &cfg).await?,
            Ok(None) => {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Err(e) => {
                eprintln!("rl_discord_rpc: stream error: {e}, reconnecting...");
                client = connect_with_retry(&cfg).await;
            }
        }
    }
}

async fn handle_event(
    event: StatsEvent,
    state: &mut MatchState,
    http: &reqwest::Client,
    discord: &mut DiscordIpcClient,
    cfg: &AppConfig,
) -> Result<(), Box<dyn Error>> {
    let mut changed = false;

    match event {
        StatsEvent::MatchCreated(payload) => {
            if let Some(id_val) = payload.extra.get("PlaylistId") {
                if let Some(id) = id_val.as_i64() {
                    state.internal_playlist_id = id;
                    state.playlist = playlist_name_from_id(id).to_string();
                    state.trn_playlist_id = trn_playlist_id_from_internal(id);
                }
            }
            state.match_start_unix = now_unix();
            state.in_match = true;
            state.team_score = 0;
            state.opponent_score = 0;
            changed = true;
        }
        StatsEvent::UpdateState(payload) => {
            state.map_asset = payload.game.arena.clone().unwrap_or_default();

            let display = map_display_name(&state.map_asset);
            state.map_image_url = resolve_map_image_url(http, &state.map_asset, display).await;

            let id_val = payload
                .extra
                .get("PlaylistId")
                .or_else(|| payload.game.extra.get("PlaylistId"));

            if let Some(id_val) = id_val {
                if let Some(id) = id_val.as_i64() {
                    let is_new_playlist = state.internal_playlist_id != id;
                    state.internal_playlist_id = id;
                    state.playlist = playlist_name_from_id(id).to_string();
                    state.trn_playlist_id = trn_playlist_id_from_internal(id);

                    if is_new_playlist {
                        if is_ranked_playlist(id) && !cfg.tracker_username.is_empty() {
                            state.rank = fetch_rank_info(
                                http,
                                &cfg.tracker_platform,
                                &cfg.tracker_username,
                                state.trn_playlist_id,
                            )
                            .await;
                        } else {
                            state.rank = None;
                        }
                    }
                }
            }

            for team in &payload.game.teams {
                match team.name.as_deref() {
                    Some("Blue") => {
                        if let Some(score) = team.score {
                            if state.team_score != score {
                                state.team_score = score;
                                changed = true;
                            }
                        }
                    }
                    Some("Orange") => {
                        if let Some(score) = team.score {
                            if state.opponent_score != score {
                                state.opponent_score = score;
                                changed = true;
                            }
                        }
                    }
                    _ => {}
                }
            }

            state.in_match = true;
            changed = true;
        }
        StatsEvent::MatchEnded(_) | StatsEvent::MatchDestroyed(_) => {
            *state = MatchState::fresh();
            changed = true;
        }
        _ => {}
    }

    if changed {
        send_activity(discord, state)?;
    }

    Ok(())
}