//! HTML parsers for IslePilot server panels (mixi.islepilot.eu,
//! sdvn2.islepilot.eu, ...). Ported from the user's proven prototype
//! (`islepilot-overlay/src/main.rs`).
//!
//! There is no public JSON API — the panel is a Next.js app that
//! server-renders plain HTML (Tailwind classes + lucide icons). Because there
//! is no API contract, this WILL break whenever IslePilot changes their
//! markup; the poller watches /api/version's buildId to flag that, and every
//! field here is Option so the UI fails soft to "—" instead of crashing.

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatBar {
    /// Raw display text, e.g. "96 / 96" — always present as fallback.
    pub raw: String,
    pub current: Option<f64>,
    pub max: Option<f64>,
}

impl StatBar {
    fn parse(raw: String) -> Self {
        // "96 / 96" -> (96, 96); "874 / 1000" etc. Tolerates thousands
        // separators just in case.
        let mut parts = raw.split('/').map(|p| {
            p.trim()
                .replace(',', "")
                .parse::<f64>()
                .ok()
        });
        let current = parts.next().flatten();
        let max = parts.next().flatten();
        Self { raw, current, max }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuestStatus {
    pub text: String,
    /// Vietnamese translation, filled AFTER parsing (translate.rs) — the
    /// parser only ever sees the panel's English prose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_vi: Option<String>,
    pub completed: bool,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    pub dino_name: Option<String>,
    pub online: Option<bool>,
    /// Raw growth text, e.g. "28%".
    pub growth: Option<String>,
    /// Parsed 0-100.
    pub growth_pct: Option<f64>,
    pub health: Option<StatBar>,
    pub hunger: Option<StatBar>,
    pub thirst: Option<StatBar>,
    pub prime_quests: Vec<QuestStatus>,
}

impl PlayerStats {
    /// Whether any dino stats parsed. NOT a session check: a logged-in
    /// player with no living dino yields none of these (the page says
    /// "No dino") — use `looks_authenticated` on the raw HTML for that.
    pub fn looks_logged_in(&self) -> bool {
        self.growth.is_some()
            || self.health.is_some()
            || self.hunger.is_some()
            || self.thirst.is_some()
    }
}

/// Session check that does not depend on having a dino: the header account
/// chip and its logout link are rendered only for an authenticated session
/// (verified against a live panel: present with a valid cookie — even with
/// "No dino" — absent on the signed-out shell, in both the visible markup
/// and the RSC payload).
pub fn looks_authenticated(html: &str) -> bool {
    html.contains("/api/player/logout") || html.contains("personaName")
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapPosition {
    pub map_disabled: bool,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub heading_deg: Option<f64>,
    pub view_box: Option<[f64; 4]>,
    /// Position as 0-100% of the map frame.
    pub pct_x: Option<f64>,
    pub pct_y: Option<f64>,
}

fn text_of(el: ElementRef) -> String {
    el.text().collect::<String>().trim().to_string()
}

/// Find the span whose text equals `label`, then read the sibling
/// `span.font-medium` value on its parent.
fn stat_value(doc: &Html, label: &str) -> Option<String> {
    let span_sel = Selector::parse("span").ok()?;
    let value_sel = Selector::parse("span.font-medium").ok()?;
    for span in doc.select(&span_sel) {
        if text_of(span) == label {
            let parent_el = ElementRef::wrap(span.parent()?)?;
            if let Some(value_el) = parent_el.select(&value_sel).next() {
                return Some(text_of(value_el));
            }
        }
    }
    None
}

pub fn parse_me(html: &str) -> PlayerStats {
    let doc = Html::parse_document(html);

    let h1_sel = Selector::parse("h1").unwrap();
    let dino_name = doc.select(&h1_sel).next().map(text_of).filter(|s| !s.is_empty());

    let span_sel = Selector::parse("span").unwrap();
    let mut online: Option<bool> = None;
    for span in doc.select(&span_sel) {
        match text_of(span).as_str() {
            "Online" => {
                online = Some(true);
                break;
            }
            "Offline" => {
                online = Some(false);
                break;
            }
            _ => {}
        }
    }

    let growth = stat_value(&doc, "Growth");
    let growth_pct = growth
        .as_deref()
        .and_then(|g| g.trim().trim_end_matches('%').parse::<f64>().ok());
    let health = stat_value(&doc, "Health").map(StatBar::parse);
    let hunger = stat_value(&doc, "Hunger").map(StatBar::parse);
    let thirst = stat_value(&doc, "Thirst").map(StatBar::parse);

    // Quest list: the container that holds the "Prime progress" heading.
    let heading_sel = Selector::parse("h2, h3").unwrap();
    let li_sel = Selector::parse("li").unwrap();
    let svg_sel = Selector::parse("svg").unwrap();

    let mut prime_quests = Vec::new();
    if let Some(heading) = doc
        .select(&heading_sel)
        .find(|h| text_of(*h) == "Prime progress")
    {
        if let Some(container) = heading.parent().and_then(ElementRef::wrap) {
            for li in container.select(&li_sel) {
                let completed = li
                    .select(&svg_sel)
                    .next()
                    .and_then(|svg| svg.value().attr("class"))
                    .map(|c| c.contains("lucide-check"))
                    .unwrap_or(false);
                prime_quests.push(QuestStatus {
                    text: text_of(li),
                    text_vi: None,
                    completed,
                });
            }
        }
    }

    PlayerStats {
        dino_name,
        online,
        growth,
        growth_pct,
        health,
        hunger,
        thirst,
        prime_quests,
    }
}

static TRANSFORM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"translate\(\s*(-?[\d.]+)[ ,]+(-?[\d.]+)\s*\)(?:\s*rotate\(\s*(-?[\d.]+)\s*\))?",
    )
    .unwrap()
});

pub fn parse_map(html: &str) -> MapPosition {
    let mut result = MapPosition {
        map_disabled: false,
        x: None,
        y: None,
        heading_deg: None,
        view_box: None,
        pct_x: None,
        pct_y: None,
    };

    if html.to_lowercase().contains("map is disabled") {
        result.map_disabled = true;
        return result;
    }

    let doc = Html::parse_document(html);
    let text_sel = Selector::parse("svg text").unwrap();
    let polygon_sel = Selector::parse("polygon").unwrap();
    let svg_sel = Selector::parse("svg").unwrap();

    // The player's own marker is the <g> holding <text>You</text>.
    let Some(you_text) = doc.select(&text_sel).find(|t| text_of(*t) == "You") else {
        return result; // offline / hidden
    };
    let Some(g_el) = you_text.parent().and_then(ElementRef::wrap) else {
        return result;
    };

    if let Some(polygon) = g_el.select(&polygon_sel).next() {
        if let Some(transform) = polygon.value().attr("transform") {
            if let Some(caps) = TRANSFORM_RE.captures(transform) {
                result.x = caps.get(1).and_then(|m| m.as_str().parse().ok());
                result.y = caps.get(2).and_then(|m| m.as_str().parse().ok());
                result.heading_deg = caps.get(3).and_then(|m| m.as_str().parse().ok());
            }
        }
    }

    // Nearest ancestor <svg> provides the viewBox for the 0-100% conversion.
    let mut node = g_el.parent();
    while let Some(n) = node {
        if let Some(el) = ElementRef::wrap(n) {
            if svg_sel.matches(&el) {
                if let Some(vb) = el.value().attr("viewBox") {
                    let parts: Vec<f64> =
                        vb.split_whitespace().filter_map(|p| p.parse().ok()).collect();
                    if parts.len() == 4 {
                        result.view_box = Some([parts[0], parts[1], parts[2], parts[3]]);
                    }
                }
                break;
            }
        }
        node = n.parent();
    }

    if let (Some(x), Some(y), Some(vb)) = (result.x, result.y, result.view_box) {
        let (minx, miny, w, h) = (vb[0], vb[1], vb[2], vb[3]);
        if w != 0.0 && h != 0.0 {
            result.pct_x = Some((x - minx) / w * 100.0);
            result.pct_y = Some((y - miny) / h * 100.0);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = include_str!("../../fixtures/islepilot/me.html");
    const MAP: &str = include_str!("../../fixtures/islepilot/map.html");
    const MAP_DISABLED: &str = include_str!("../../fixtures/islepilot/map_disabled.html");

    #[test]
    fn parses_stats_from_me_fixture() {
        let stats = parse_me(ME);
        assert_eq!(stats.dino_name.as_deref(), Some("Triceratops"));
        assert_eq!(stats.online, Some(true));
        assert_eq!(stats.growth.as_deref(), Some("28%"));
        assert_eq!(stats.growth_pct, Some(28.0));
        let health = stats.health.as_ref().unwrap();
        assert_eq!((health.current, health.max), (Some(96.0), Some(96.0)));
        let hunger = stats.hunger.as_ref().unwrap();
        assert_eq!((hunger.current, hunger.max), (Some(35.0), Some(48.0)));
        let thirst = stats.thirst.as_ref().unwrap();
        assert_eq!((thirst.current, thirst.max), (Some(874.0), Some(1000.0)));
        assert!(stats.looks_logged_in());

        assert_eq!(stats.prime_quests.len(), 10);
        let done: Vec<&str> = stats
            .prime_quests
            .iter()
            .filter(|q| q.completed)
            .map(|q| q.text.as_str())
            .collect();
        assert_eq!(done, ["Never be Infertile", "Never get Muscle spasms"]);
    }

    #[test]
    fn empty_page_is_not_logged_in() {
        let stats = parse_me("<html><body><h1>Sign in</h1></body></html>");
        assert!(!stats.looks_logged_in());
    }

    /// Field case: a valid session on a server where the player has no dino
    /// — /me renders the account chip + "No dino" and zero stats. That must
    /// count as authenticated, while the signed-out shell must not.
    #[test]
    fn no_dino_page_is_authenticated_but_not_logged_in() {
        let html = r#"<html><body>
            <header><span class="font-medium">Survivor</span>
            <a href="/api/player/logout?redirect=%2Fme">Logout</a></header>
            <main><section>No dino</section></main>
        </body></html>"#;
        assert!(looks_authenticated(html), "logout link proves the session");
        assert!(!parse_me(html).looks_logged_in(), "and yet no stats parse");
        assert!(
            !looks_authenticated("<html><body><h1>Sign in through Steam</h1></body></html>"),
            "the signed-out shell has neither marker"
        );
    }

    #[test]
    fn parses_you_marker_from_map_fixture() {
        let map = parse_map(MAP);
        assert!(!map.map_disabled);
        assert!((map.x.unwrap() - 281.798).abs() < 0.01);
        assert!((map.y.unwrap() - 861.338).abs() < 0.01);
        assert!((map.heading_deg.unwrap() - 171.699).abs() < 0.01);
        assert_eq!(map.view_box, Some([0.0, 0.0, 1000.0, 1000.0]));
        assert!((map.pct_x.unwrap() - 28.18).abs() < 0.01);
        assert!((map.pct_y.unwrap() - 86.13).abs() < 0.01);
    }

    /// Dev check against a REAL saved page (fixtures only prove the shape we
    /// wrote them to). Save one with your own cookie, then:
    ///   THEISLE_LIVE_ME=C:\path\me.html cargo test -- --ignored parse_live
    #[test]
    #[ignore]
    fn parse_live_pages() {
        if let Ok(path) = std::env::var("THEISLE_LIVE_ME") {
            let html = std::fs::read_to_string(&path).expect("read live /me");
            let stats = parse_me(&html);
            println!("live /me -> {stats:#?}");
            assert!(stats.looks_logged_in(), "live page must parse as logged in");
            assert!(stats.dino_name.is_some());
            assert!(stats.health.as_ref().unwrap().current.is_some());
            assert_eq!(stats.prime_quests.len(), 10, "expected 10 prime quests");
        }
        if let Ok(path) = std::env::var("THEISLE_LIVE_MAP") {
            let html = std::fs::read_to_string(&path).expect("read live /map");
            let map = parse_map(&html);
            println!("live /map -> {map:#?}");
            assert!(!map.map_disabled);
            assert!(map.pct_x.is_some() && map.pct_y.is_some());
        }
    }

    #[test]
    fn detects_disabled_map() {
        let map = parse_map(MAP_DISABLED);
        assert!(map.map_disabled);
        assert_eq!(map.x, None);
    }
}
