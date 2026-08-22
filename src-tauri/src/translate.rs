//! English -> Vietnamese translation for IslePilot quest strings.
//!
//! The panel serves free-form English prose with no IDs, so translation is
//! keyed on the exact text, three layers deep:
//!
//!   1. compile-time dictionary — the known Prime quest pool, hand-translated
//!      so game terms stay right ("Get nested in" is not machine-translatable);
//!   2. template rules — numeric variants ("Visit 3 Patrol zones") so a
//!      different count never falls through to the network;
//!   3. persistent cache (%LOCALAPPDATA%\TheIsleOverlay\quest_translations.json),
//!      filled by the MyMemory web API for strings the first two layers miss.
//!      Each unknown string costs ONE api call ever.
//!
//! A miss simply stays English — translation must never break the dino panel.
//!
//! MyMemory (api.mymemory.translated.net) is a public API with an anonymous
//! free tier (~5000 chars/day). We never attach the user's email or any other
//! identifier to requests. On failure the string is not retried for 15 min;
//! on quota exhaustion ALL calls are suspended for 6 h. Never tight-loops.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use regex::Regex;

use crate::islepilot::parser::QuestStatus;
use crate::settings;
use crate::state::LockExt;

/// Hand-translated Prime quest pool (exact match, trimmed). Keep game terms
/// recognisable — players see the English names in-game and on the panel.
const DICT: &[(&str, &str)] = &[
    ("Visit a Sanctuary as a juvenile", "Ghé Khu bảo tồn (Sanctuary) khi còn non"),
    ("Get nested in", "Được sinh ra từ tổ (nest)"),
    ("Get perfect diet (1% of each)", "Đạt chế độ ăn hoàn hảo (mỗi loại 1%)"),
    ("Visit Mass Migration zone", "Ghé khu Đại di cư (Mass Migration)"),
    ("Never be Infertile", "Không bao giờ bị Vô sinh (Infertile)"),
    ("Never get Muscle spasms", "Không bao giờ bị Co thắt cơ (Muscle spasms)"),
    ("Raise children to Subadult", "Nuôi con đến Subadult"),
    ("Be a Hypsi, Troodon, Beipi, Dryo or Deino", "Chơi Hypsi, Troodon, Beipi, Dryo hoặc Deino"),
];

/// Numeric variants: `{n}` is replaced by the captured count.
static TEMPLATES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"^Visit (\d+) Migration zones?$", "Ghé {n} khu Di cư (Migration)"),
        (r"^Visit (\d+) Patrol zones?$", "Ghé {n} khu Tuần tra (Patrol)"),
        (r"^Visit (\d+) Sanctuar(?:y|ies)$", "Ghé {n} Khu bảo tồn (Sanctuary)"),
        (r"^Raise (\d+) child(?:ren)? to Subadult$", "Nuôi {n} con đến Subadult"),
    ]
    .into_iter()
    .map(|(re, vi)| (Regex::new(re).unwrap(), vi))
    .collect()
});

fn dict_lookup(text: &str) -> Option<&'static str> {
    DICT.iter().find(|(en, _)| *en == text).map(|(_, vi)| *vi)
}

fn template_lookup(text: &str) -> Option<String> {
    for (re, out) in TEMPLATES.iter() {
        if let Some(caps) = re.captures(text) {
            let n = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            return Some(out.replace("{n}", n));
        }
    }
    None
}

// ------------------------------------------------------------------- cache ---

/// Lazily loaded once; every write goes straight back to disk (a handful of
/// inserts over the app's whole life — the quest pool is finite).
static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

fn cache_path() -> PathBuf {
    settings::local_dir().join("quest_translations.json")
}

/// Corrupt or non-object JSON -> empty map: the cache is rebuildable, so a
/// bad file must never take the poller down (same leniency as settings).
fn cache_from_str(s: &str) -> HashMap<String, String> {
    serde_json::from_str(s).unwrap_or_default()
}

fn with_cache<R>(f: impl FnOnce(&mut HashMap<String, String>) -> R) -> R {
    let mut guard = CACHE.lock_safe();
    let map = guard.get_or_insert_with(|| {
        std::fs::read_to_string(cache_path())
            .map(|s| cache_from_str(&s))
            .unwrap_or_default()
    });
    f(map)
}

fn cache_put(text: &str, vi: &str) {
    with_cache(|map| {
        map.insert(text.to_string(), vi.to_string());
        match serde_json::to_value(&*map) {
            Ok(value) => {
                if let Err(e) = settings::save_json(&cache_path(), &value) {
                    log::warn!("quest translation cache save failed: {e}");
                }
            }
            Err(e) => log::warn!("quest translation cache serialize failed: {e}"),
        }
    });
}

// -------------------------------------------------------------- API fallback ---

const API_URL: &str = "https://api.mymemory.translated.net/get";
const RETRY_AFTER: Duration = Duration::from_secs(15 * 60);
const QUOTA_SUSPEND: Duration = Duration::from_secs(6 * 60 * 60);
/// At most this many API calls per poll tick — the pool converges in a few
/// ticks and is then served from the cache forever.
const API_BUDGET_PER_TICK: u32 = 2;

#[derive(Debug)]
enum ApiFailure {
    /// Free tier exhausted — suspend everything, English is fine meanwhile.
    Quota,
    Other(String),
}

/// Per-string failure timestamps (don't re-ask about the same string for
/// RETRY_AFTER) and the global quota suspension.
static FAILED: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
static SUSPENDED_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

fn api_allowed(text: &str) -> bool {
    if let Some(until) = *SUSPENDED_UNTIL.lock_safe() {
        if Instant::now() < until {
            return false;
        }
    }
    let mut failed = FAILED.lock_safe();
    let map = failed.get_or_insert_with(HashMap::new);
    !matches!(map.get(text), Some(at) if at.elapsed() < RETRY_AFTER)
}

fn note_failure(text: &str, failure: ApiFailure) {
    match failure {
        ApiFailure::Quota => {
            log::warn!("mymemory quota exhausted; suspending quest translation for 6h");
            *SUSPENDED_UNTIL.lock_safe() = Some(Instant::now() + QUOTA_SUSPEND);
        }
        ApiFailure::Other(e) => {
            log::warn!("mymemory translate failed for {text:?}: {e}");
            FAILED
                .lock_safe()
                .get_or_insert_with(HashMap::new)
                .insert(text.to_string(), Instant::now());
        }
    }
}

fn api_translate(client: &reqwest::blocking::Client, text: &str) -> Result<String, ApiFailure> {
    let resp = client
        .get(API_URL)
        .query(&[("q", text), ("langpair", "en|vi")])
        .send()
        .map_err(|e| ApiFailure::Other(e.to_string()))?;
    let status = resp.status().as_u16();
    if status == 403 || status == 429 {
        return Err(ApiFailure::Quota);
    }
    let body = resp.text().map_err(|e| ApiFailure::Other(e.to_string()))?;
    parse_api_response(&body)
}

fn parse_api_response(body: &str) -> Result<String, ApiFailure> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| ApiFailure::Other(e.to_string()))?;
    let text = v
        .get("responseData")
        .and_then(|d| d.get("translatedText"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    // Quota answers arrive as a 200 body whose "translation" is the warning.
    let upper = text.to_uppercase();
    if upper.contains("MYMEMORY WARNING") || upper.contains("QUOTA") {
        return Err(ApiFailure::Quota);
    }
    // responseStatus is a number on success and sometimes a string on error.
    let status_ok = match v.get("responseStatus") {
        Some(serde_json::Value::Number(n)) => n.as_i64() == Some(200),
        Some(serde_json::Value::String(s)) => s == "200",
        _ => false,
    };
    if !status_ok {
        return Err(ApiFailure::Other(format!(
            "responseStatus {:?}",
            v.get("responseStatus")
        )));
    }
    let text = html_unescape(text.trim());
    if text.is_empty() {
        return Err(ApiFailure::Other("empty translation".into()));
    }
    Ok(text)
}

/// MyMemory HTML-escapes its output; only the entities it actually emits.
fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

// --------------------------------------------------------------- public API ---

/// Dictionary + templates + cache. No network; the only I/O is the one-time
/// cache file read.
pub fn translate_offline(text: &str) -> Option<String> {
    let key = text.trim();
    if let Some(vi) = dict_lookup(key) {
        return Some(vi.to_string());
    }
    if let Some(vi) = template_lookup(key) {
        return Some(vi);
    }
    with_cache(|map| map.get(key).cloned())
}

/// Fill `text_vi` on every quest that can be translated; unknown strings hit
/// the MyMemory API (budgeted, cached forever). Runs on the poller thread —
/// never on a UI path.
pub fn translate_quests(quests: &mut [QuestStatus], client: &reqwest::blocking::Client) {
    let mut budget = API_BUDGET_PER_TICK;
    for quest in quests.iter_mut() {
        if quest.text_vi.is_some() {
            continue;
        }
        if let Some(vi) = translate_offline(&quest.text) {
            quest.text_vi = Some(vi);
            continue;
        }
        if budget == 0 || !api_allowed(quest.text.trim()) {
            continue; // stays English until a later tick
        }
        budget -= 1;
        let key = quest.text.trim().to_string();
        match api_translate(client, &key) {
            Ok(vi) => {
                log::info!("mymemory translated quest {key:?} -> {vi:?}");
                cache_put(&key, &vi);
                quest.text_vi = Some(vi);
            }
            Err(failure) => note_failure(&key, failure),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = include_str!("../fixtures/islepilot/me.html");

    /// Pins the seed dictionary to the real quest pool: every string the
    /// fixture page carries must resolve WITHOUT touching cache or network.
    #[test]
    fn dict_and_templates_cover_the_fixture_pool() {
        let stats = crate::islepilot::parser::parse_me(ME);
        assert_eq!(stats.prime_quests.len(), 10);
        for quest in &stats.prime_quests {
            let hit = dict_lookup(&quest.text).is_some() || template_lookup(&quest.text).is_some();
            assert!(hit, "no offline translation for {:?}", quest.text);
        }
    }

    #[test]
    fn templates_fill_in_the_number() {
        assert_eq!(
            template_lookup("Visit 3 Patrol zones").as_deref(),
            Some("Ghé 3 khu Tuần tra (Patrol)")
        );
        assert_eq!(
            template_lookup("Visit 1 Migration zone").as_deref(),
            Some("Ghé 1 khu Di cư (Migration)")
        );
        assert_eq!(
            template_lookup("Visit 2 Sanctuaries").as_deref(),
            Some("Ghé 2 Khu bảo tồn (Sanctuary)")
        );
        assert_eq!(
            template_lookup("Raise 2 children to Subadult").as_deref(),
            Some("Nuôi 2 con đến Subadult")
        );
        assert_eq!(template_lookup("Visit zones"), None);
        assert_eq!(template_lookup("visit 3 patrol zones"), None, "case matters");
    }

    #[test]
    fn corrupt_cache_is_an_empty_map() {
        assert!(cache_from_str("not json").is_empty());
        assert!(cache_from_str("[1,2]").is_empty());
        assert!(cache_from_str(r#"{"a":1}"#).is_empty(), "wrong value type");
        let m = cache_from_str(r#"{"Eat fish":"Ăn cá"}"#);
        assert_eq!(m.get("Eat fish").map(String::as_str), Some("Ăn cá"));
    }

    #[test]
    fn api_response_parsing_and_quota_detection() {
        let ok = r#"{"responseData":{"translatedText":"Xin chào &amp; bạn"},"responseStatus":200}"#;
        assert_eq!(parse_api_response(ok).unwrap(), "Xin chào & bạn");
        let quota = r#"{"responseData":{"translatedText":"MYMEMORY WARNING: YOU USED ALL AVAILABLE FREE TRANSLATIONS FOR TODAY"},"responseStatus":403}"#;
        assert!(matches!(parse_api_response(quota), Err(ApiFailure::Quota)));
        let string_status = r#"{"responseData":{"translatedText":""},"responseStatus":"403"}"#;
        assert!(matches!(parse_api_response(string_status), Err(ApiFailure::Other(_))));
        assert!(parse_api_response("junk").is_err());
        let empty = r#"{"responseData":{"translatedText":"  "},"responseStatus":200}"#;
        assert!(parse_api_response(empty).is_err(), "whitespace is not a translation");
    }

    /// One real MyMemory call through the exact production path:
    ///   THEISLE_TEST_TRANSLATE=1 cargo test -- --ignored live_mymemory
    #[test]
    #[ignore]
    fn live_mymemory() {
        if std::env::var("THEISLE_TEST_TRANSLATE").is_err() {
            eprintln!("set THEISLE_TEST_TRANSLATE=1 to run");
            return;
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        let vi = api_translate(&client, "Visit the lake twice").expect("api reachable");
        eprintln!("mymemory -> {vi:?}");
        assert!(!vi.is_empty());
    }
}
