use serde::Deserialize;
use tracing::warn;

use super::event::LyricLine;

#[derive(Deserialize)]
struct LrclibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

pub async fn fetch_synced_lyrics(
    title: &str,
    artist: &str,
    album: &str,
    client: &reqwest::Client,
) -> Option<Box<[LyricLine]>> {
    let resp = client
        .get("https://lrclib.net/api/get")
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
        return None;
    }

    let data: LrclibResponse = resp.json().await.ok()?;
    let lyrics_text = data.synced_lyrics?;

    Some(parse_lrc(&lyrics_text))
}

fn parse_lrc(lrc: &str) -> Box<[LyricLine]> {
    use crate::modules::renderer::utils::hash_str;
    let mut lines = Vec::new();

    for line in lrc.lines() {
        let line = line.trim();
        if let (Some(start), Some(end)) = (line.find('['), line.find(']')) {
            if start < end {
                let time_str = &line[start + 1..end];
                let text = line[end + 1..].trim().to_string().into_boxed_str();
                let text_hash = hash_str(&text);

                let mut parts = time_str.split(':');
                if let (Some(m), Some(s)) = (parts.next(), parts.next()) {
                    if let (Ok(mins), Ok(secs)) = (m.parse::<f32>(), s.parse::<f32>()) {
                        lines.push(LyricLine {
                            start_time_secs: mins * 60.0 + secs,
                            text,
                            text_hash,
                        });
                    }
                }
            }
        }
    }
    lines.into_boxed_slice()
}

#[cfg(test)]
mod tests;
