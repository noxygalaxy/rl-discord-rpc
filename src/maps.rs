// maps fetched via 3 methods:
// - 1. Static images from assets or github repo of bakkesmod discordrpc plugin.
// - 2. Fandom wiki imageinfo API.
// - 3. Google Image Search via SerpApi (last resort fallback).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;

const MAP_REPO_BASE: &str =
    "https://raw.githubusercontent.com/segalll/DiscordRPCPlugin/master/maps";
const FANDOM_API_BASE: &str = "https://rocketleague.fandom.com/api.php";

static IMAGE_CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

fn cache_get(key: &str) -> Option<String> {
    let guard = IMAGE_CACHE.lock().unwrap();
    guard.as_ref()?.get(key).cloned()
}

fn cache_set(key: &str, value: &str) {
    let mut guard = IMAGE_CACHE.lock().unwrap();
    guard
        .get_or_insert_with(HashMap::new)
        .insert(key.to_string(), value.to_string());
}

pub fn map_display_name(asset: &str) -> &'static str {
    let table: HashMap<&str, &str> = HashMap::from([
        ("EuroStadium_P", "Mannfield"),
        ("EuroStadium_Dusk_P", "Mannfield"),
        ("EuroStadium_Night_P", "Mannfield"),
        ("EuroStadium_Rainy_P", "Mannfield"),
        ("eurostadium_snownight_p", "Mannfield"),
        ("CHN_Stadium_P", "Forbidden Temple"),
        ("CHN_Stadium_Day_P", "Forbidden Temple"),
        ("Stadium_P", "DFH Stadium"),
        ("stadium_day_p", "DFH Stadium"),
        ("Stadium_Foggy_P", "DFH Stadium"),
        ("Stadium_Winter_P", "DFH Stadium"),
        ("Stadium_Race_Day_p", "DFH Stadium"),
        ("UtopiaStadium_P", "Utopia Coliseum"),
        ("UtopiaStadium_Dusk_P", "Utopia Coliseum"),
        ("UtopiaStadium_Snow_P", "Utopia Coliseum"),
        ("UtopiaStadium_Lux_P", "Utopia Coliseum"),
        ("Underwater_P", "AquaDome"),
        ("Underwater_GRS_P", "AquaDome"),
        ("NeoTokyo_Standard_P", "Neo Tokyo"),
        ("NeoTokyo_P", "Neo Tokyo"),
        ("NeoTokyo_Arcade_P", "Neo Tokyo"),
        ("NeoTokyo_Hax_P", "Neo Tokyo"),
        ("NeoTokyo_Toon_p", "Neo Tokyo"),
        ("cs_p", "Champions Field"),
        ("cs_day_p", "Champions Field"),
        ("cs_hw_p", "Rivals Arena"),
        ("Park_P", "Beckwith Park"),
        ("Park_Night_P", "Beckwith Park"),
        ("Park_Rainy_P", "Beckwith Park"),
        ("Park_Snowy_P", "Beckwith Park"),
        ("park_bman_p", "Beckwith Park"),
        ("TrainStation_P", "Urban Central"),
        ("TrainStation_Night_P", "Urban Central"),
        ("TrainStation_Dawn_P'", "Urban Central"),
        ("Haunted_TrainStation_P", "Urban Central"),
        ("Wasteland_P", "Badlands"),
        ("Wasteland_Night_P", "Badlands"),
        ("Wasteland_GRS_P", "Badlands"),
        ("wasteland_s_p", "Badlands"),
        ("wasteland_Night_S_P", "Badlands"),
        ("Farm_GRS_P", "Farmstead"),
        ("Farm_HW_P", "Farmstead"),
        ("Farm_Night_P", "Farmstead"),
        ("farm_p", "Farmstead"),
        ("beach_P", "Salty Shores"),
        ("beach_night_p", "Salty Shores"),
        ("beach_night_grs_p", "Salty Shores"),
        ("music_p", "Neon Fields"),
        ("outlaw_p", "Deadeye Canyon"),
        ("Outlaw_Oasis_P", "Deadeye Canyon"),
        ("street_p", "Sovereign Heights"),
        ("FF_Dusk_P", "Estadio Vida"),
        ("Woods_P", "Forbidden Temple"),
        ("Woods_Night_P", "Drift Woods"),
        ("UF_Day_P", "Futura Garden"),
        ("ARC_P", "Core 707"),
        ("ARC_Darc_P", "Core 707"),
        ("arc_standard_p", "Core 707"),
        ("STADIUM_10A_P", "Neon Fields"),
        ("ShatterShot_P", "Discotheque"),
        ("KO_Calavera_P", "Knockout Calavera"),
        ("KO_Carbon_P", "Knockout Carbon"),
        ("KO_Quadron_P", "Knockout Quadron"),
        ("swoosh_p", "Deadeye Canyon"),
        ("throwbackstadium_P", "Throwback Stadium"),
        ("throwbackhockey_p", "Throwback Stadium"),
        ("hoopsStreet_p", "Dunk House"),
        ("HoopsStadium_P", "Dunk House"),
        ("bb_p", "The Block"),
        ("FNI_Stadium_P", "DFH Stadium"),
        ("random", "Random Map"),
    ]);
    table.get(asset).copied().unwrap_or("Unknown Map")
}

#[derive(Deserialize)]
struct FandomApiResponse {
    query: Option<FandomQuery>,
}

#[derive(Deserialize)]
struct FandomQuery {
    pages: HashMap<String, FandomPage>,
}

#[derive(Deserialize)]
struct FandomPage {
    imageinfo: Option<Vec<FandomImageInfo>>,
}

#[derive(Deserialize)]
struct FandomImageInfo {
    url: String,
}

async fn fandom_image_url(http: &reqwest::Client, file_title: &str) -> Option<String> {
    let title = format!("File:{}", file_title);
    let resp = http
        .get(FANDOM_API_BASE)
        .query(&[
            ("action", "query"),
            ("titles", title.as_str()),
            ("prop", "imageinfo"),
            ("iiprop", "url"),
            ("format", "json"),
        ])
        .send()
        .await
        .ok()?;

    let parsed: FandomApiResponse = resp.json().await.ok()?;
    let pages = parsed.query?.pages;

    for (_id, page) in pages {
        if let Some(infos) = page.imageinfo {
            if let Some(first) = infos.into_iter().next() {
                return Some(first.url);
            }
        }
    }
    None
}

fn fandom_filename_candidates(display_name: &str) -> Vec<String> {
    let base = display_name.replace(' ', "_");
    vec![
        format!("{}.jpg", base),
        format!("{}.png", base),
        format!("{}_arena.jpg", base),
        format!("{}_standard.jpg", base),
    ]
}

#[derive(Deserialize)]
struct SerpImage {
    original: Option<String>,
    thumbnail: Option<String>,
}

#[derive(Deserialize)]
struct SerpResponse {
    images_results: Option<Vec<SerpImage>>,
}

async fn google_image_search_first(http: &reqwest::Client, query: &str) -> Option<String> {
    let api_key = std::env::var("SERPAPI_KEY").ok()?;
    let url = format!(
        "https://serpapi.com/search.json?engine=google_images&q={}&api_key={}",
        urlencoding::encode(query),
        api_key
    );
    let resp = http.get(&url).send().await.ok()?;
    let parsed: SerpResponse = resp.json().await.ok()?;
    let first = parsed.images_results?.into_iter().next()?;
    first.original.or(first.thumbnail)
}

pub async fn resolve_map_image_url(
    http: &reqwest::Client,
    asset_name: &str,
    display_name: &str,
) -> String {
    if let Some(cached) = cache_get(asset_name) {
        return cached;
    }

    let static_url = format!("{}/{}.png", MAP_REPO_BASE, asset_name);
    if let Ok(resp) = http.head(&static_url).send().await {
        if resp.status().is_success() {
            cache_set(asset_name, &static_url);
            return static_url;
        }
    }

    for candidate in fandom_filename_candidates(display_name) {
        if let Some(url) = fandom_image_url(http, &candidate).await {
            cache_set(asset_name, &url);
            return url;
        }
    }

    let resolved = match google_image_search_first(
        http,
        &format!("{} rocket league map", display_name),
    )
    .await
    {
        Some(url) => url,
        None => format!("{}/Template.png", MAP_REPO_BASE),
    };
    cache_set(asset_name, &resolved);
    resolved
}
