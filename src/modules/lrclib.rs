// =============================================================================
// modules/lrclib.rs
// =============================================================================
// Fetches synced lyrics from the free LRCLIB API.
//
// LRCLIB (https://lrclib.net/) provides timestamped LRC files for millions of
// tracks. We parse these into a format our engine can use to drive visual
// pulses and effects perfectly in time with the vocals!
// =============================================================================

use serde::Deserialize;
use tracing::warn;

use super::event::LyricLine;

#[derive(Deserialize)]
struct LrclibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

/// Fetch synced lyrics for a track.
pub async fn fetch_synced_lyrics(title: &str, artist: &str, album: &str) -> Option<Vec<LyricLine>> {
    let client = reqwest::Client::new();
    
    let resp = client.get("https://lrclib.net/api/get")
        .query(&[
            ("track_name", title),
            ("artist_name", artist),
            ("album_name", album),
        ])
        .send()
        .await
        .map_err(|e| warn!("LRCLIB request failed: {}", e))
        .ok()?;

    if !resp.status().is_success() {
        return None; // Commonly 404 if the track isn't in their database yet
    }

    let data: LrclibResponse = resp.json().await.ok()?;
    let lyrics_text = data.synced_lyrics?;

    Some(parse_lrc(&lyrics_text))
}

/// Parses a standard LRC file format string into structured LyricLines.
fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();

    for line in lrc.lines() {
        let line = line.trim();
        if let (Some(start), Some(end)) = (line.find('['), line.find(']')) {
            let time_str = &line[start + 1..end];
            let text = line[end + 1..].trim().to_string();

            let mut parts = time_str.split(':');
            if let (Some(m), Some(s)) = (parts.next(), parts.next()) {
                if let (Ok(mins), Ok(secs)) = (m.parse::<f32>(), s.parse::<f32>()) {
                    lines.push(LyricLine { start_time_secs: mins * 60.0 + secs, text });
                }
            }
        }
    }
    lines
}