// fetcher of rank data

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;

static RANK_CACHE: Mutex<Option<HashMap<String, Option<RankInfo>>>> = Mutex::new(None);

#[derive(Clone, Debug)]
pub struct RankInfo {
    pub icon_url: String,
    pub tier_name: String,
    pub division_name: Option<String>,
}

fn cache_get(key: &str) -> Option<Option<RankInfo>> {
    let guard = RANK_CACHE.lock().unwrap();
    guard.as_ref()?.get(key).cloned()
}

fn cache_set(key: &str, value: Option<RankInfo>) {
    let mut guard = RANK_CACHE.lock().unwrap();
    guard.get_or_insert_with(HashMap::new).insert(key.to_string(), value);
}

#[derive(Deserialize)]
struct TrnProfileResponse {
    data: Option<TrnData>,
}

#[derive(Deserialize)]
struct TrnData {
    segments: Option<Vec<TrnSegment>>,
}

#[derive(Deserialize)]
struct TrnSegment {
    #[serde(rename = "type")]
    segment_type: Option<String>,
    attributes: Option<TrnAttributes>,
    stats: Option<TrnStats>,
}

#[derive(Deserialize)]
struct TrnAttributes {
    #[serde(rename = "playlistId")]
    playlist_id: Option<i64>,
}

#[derive(Deserialize)]
struct TrnStats {
    tier: Option<TrnStat>,
    division: Option<TrnStat>,
}

#[derive(Deserialize)]
struct TrnStat {
    metadata: Option<TrnStatMetadata>,
}

#[derive(Deserialize)]
struct TrnStatMetadata {
    #[serde(rename = "iconUrl")]
    icon_url: Option<String>,
    name: Option<String>,
}

/// rl playlist id's
pub mod playlist_id {
    pub const CASUAL: i64 = 0;
    pub const RANKED_DUEL_1V1: i64 = 10;
    pub const RANKED_DOUBLES_2V2: i64 = 11;
    pub const RANKED_STANDARD_3V3: i64 = 13;
    pub const HOOPS: i64 = 27;
    pub const RUMBLE: i64 = 28;
    pub const DROPSHOT: i64 = 29;
    pub const SNOWDAY: i64 = 30;
    pub const TOURNAMENT: i64 = 34;
    pub const RANKED_4V4_QUADS: i64 = 61;
    pub const HEATSEEKER: i64 = 63;
}

// fetches player current rank via tracker.gg
pub async fn fetch_rank_info(
    http: &reqwest::Client,
    platform: &str,
    username: &str,
    playlist_id: i64,
) -> Option<RankInfo> {
    let cache_key = format!("{platform}:{username}:{playlist_id}");
    if let Some(cached) = cache_get(&cache_key) {
        return cached;
    }

    let url = format!(
        "https://api.tracker.gg/api/v2/rocket-league/standard/profile/{}/{}",
        platform,
        urlencoding::encode(username)
    );

    let result = async {
        let resp = http
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
            .header("Accept", "application/json")
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            eprintln!(
                "rl_discord_rpc: tracker.gg returned status {} (may be Cloudflare-blocked)",
                resp.status()
            );
            return None;
        }

        let parsed: TrnProfileResponse = resp.json().await.ok()?;
        let segments = parsed.data?.segments?;

        for segment in segments {
            if segment.segment_type.as_deref() != Some("playlist") {
                continue;
            }
            let attrs = segment.attributes?;
            if attrs.playlist_id != Some(playlist_id) {
                continue;
            }

            let stats = segment.stats?;
            let tier = stats.tier?;
            let tier_meta = tier.metadata?;
            let icon_url = tier_meta.icon_url?;
            let tier_name = tier_meta.name.unwrap_or_else(|| "Unranked".to_string());

            let division_name = stats
                .division
                .and_then(|d| d.metadata)
                .and_then(|m| m.name);

            return Some(RankInfo {
                icon_url,
                tier_name,
                division_name,
            });
        }
        None
    }
    .await;

    cache_set(&cache_key, result.clone());
    result
}
