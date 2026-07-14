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
    let mut resp = client
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

    let mut bytes = Vec::new();
    const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10 MB limit
    while let Ok(Some(chunk)) = resp.chunk().await {
        if bytes.len() + chunk.len() > MAX_JSON_SIZE {
            warn!("LRCLIB response exceeded 10MB limit");
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }

    let data: LrclibResponse = serde_json::from_slice(&bytes).ok()?;
    let lyrics_text = data.synced_lyrics?;

    Some(parse_lrc(&lyrics_text))
}

fn parse_lrc(lrc: &str) -> Box<[LyricLine]> {
    use crate::modules::renderer::utils::hash_str;
    let mut lines = Vec::new();

    for line in lrc.lines() {
        // Strip every leading [..] tag: the compressed LRC form puts several
        // timestamps on one line ("[00:12.00][00:45.00]chorus") to repeat it,
        // so each parsed timestamp gets its own entry. Tags that don't parse
        // as times (metadata like "[ar:Artist]", malformed stamps) are
        // consumed but produce no entry.
        let mut rest = line.trim();
        let mut times = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let tag = &stripped[..end];
            let mut parts = tag.split(':');
            if let (Some(m), Some(s)) = (parts.next(), parts.next()) {
                if let (Ok(mins), Ok(secs)) = (m.parse::<f32>(), s.parse::<f32>()) {
                    times.push(mins * 60.0 + secs);
                }
            }
            rest = stripped[end + 1..].trim_start();
        }

        if times.is_empty() {
            continue;
        }

        let text: Box<str> = rest.trim().into();
        let text_hash = hash_str(&text);
        for start_time_secs in times {
            lines.push(LyricLine {
                start_time_secs,
                text: text.clone(),
                text_hash,
            });
        }
    }

    // The renderer's lyric-scroll tracking (both its O(1) fast path and its
    // partition_point fallback) assumes lines are in ascending start_time_secs
    // order. Community-submitted LRC files aren't guaranteed to be sorted, so
    // enforce it here rather than at every consumer.
    lines.sort_by(|a, b| a.start_time_secs.total_cmp(&b.start_time_secs));

    lines.into_boxed_slice()
}

#[cfg(test)]
mod tests;
