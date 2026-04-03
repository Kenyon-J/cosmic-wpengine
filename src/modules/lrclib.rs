use serde::Deserialize;
use std::borrow::Cow;
use tracing::warn;

use super::event::LyricLine;

#[derive(Deserialize)]
struct LrclibResponse<'a> {
    #[serde(borrow, rename = "syncedLyrics")]
    synced_lyrics: Option<Cow<'a, str>>,
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

    let text = resp.text().await.ok()?;
    let data: LrclibResponse = serde_json::from_str(&text).ok()?;
    let lyrics_text = data.synced_lyrics?;

    Some(parse_lrc(&lyrics_text))
}

fn parse_lrc(lrc: &str) -> Box<[LyricLine]> {
    let mut lines = Vec::new();

    for line in lrc.lines() {
        let line = line.trim();
        if let (Some(start), Some(end)) = (line.find('['), line.find(']')) {
            if start < end {
                let time_str = &line[start + 1..end];
                let text = line[end + 1..].trim().to_string().into_boxed_str();

                let mut parts = time_str.split(':');
                if let (Some(m), Some(s)) = (parts.next(), parts.next()) {
                    if let (Ok(mins), Ok(secs)) = (m.parse::<f32>(), s.parse::<f32>()) {
                        lines.push(LyricLine {
                            start_time_secs: mins * 60.0 + secs,
                            text,
                        });
                    }
                }
            }
        }
    }
    lines.into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    // A safer tolerance for floating point time comparisons (1 millisecond)
    const TOLERANCE: f32 = 0.001;

    #[test]
    fn test_parse_lrc_valid() {
        let lrc_data = "\
            [00:12.34] First line of lyrics\n\
            [01:05.67] Second line of lyrics\n\
            [02:45.00] Third line of lyrics\
        ";

        let lines = parse_lrc(lrc_data);

        assert_eq!(lines.len(), 3);

        assert!((lines[0].start_time_secs - 12.34).abs() < TOLERANCE);
        assert_eq!(lines[0].text.as_ref(), "First line of lyrics");

        assert!((lines[1].start_time_secs - 65.67).abs() < TOLERANCE);
        assert_eq!(lines[1].text.as_ref(), "Second line of lyrics");

        assert!((lines[2].start_time_secs - 165.00).abs() < TOLERANCE);
        assert_eq!(lines[2].text.as_ref(), "Third line of lyrics");
    }

    #[test]
    fn test_parse_lrc_invalid_time() {
        let lrc_data = "\
            [00:12.34] Valid line\n\
            [01:xx.67] Invalid time format\n\
            [invalid] Also invalid time\n\
            [02:45.00] Another valid line\
        ";

        let lines = parse_lrc(lrc_data);

        assert_eq!(lines.len(), 2);

        assert!((lines[0].start_time_secs - 12.34).abs() < TOLERANCE);
        assert_eq!(lines[0].text.as_ref(), "Valid line");

        assert!((lines[1].start_time_secs - 165.00).abs() < TOLERANCE);
        assert_eq!(lines[1].text.as_ref(), "Another valid line");
    }

    #[test]
    fn test_parse_lrc_empty_lines_and_metadata() {
        let lrc_data = "\
            [ti:Song Title]\n\
            [ar:Artist Name]\n\
            \n\
            [00:01.00]    \n\
            [00:05.00] Valid lyrics\n\
        ";

        let lines = parse_lrc(lrc_data);

        // the `[00:01.00]    ` line will be parsed but its text will be empty, which is valid behaviour
        assert_eq!(lines.len(), 2);

        assert!((lines[0].start_time_secs - 1.0).abs() < TOLERANCE);
        assert_eq!(lines[0].text.as_ref(), "");

        assert!((lines[1].start_time_secs - 5.0).abs() < TOLERANCE);
        assert_eq!(lines[1].text.as_ref(), "Valid lyrics");
    }

    #[test]
    fn test_parse_lrc_edge_cases() {
        let lrc_data = "\
            ]01:22.00[ Reversed brackets should not panic\n\
            [01.22] No colon should be ignored safely\n\
            [01:22] No dot should be ignored safely\n\
            No brackets here at all\n\
            [[02:00.00] Nested brackets\n\
            [03:00.00] Valid amid chaos\n\
            ]00:01.00[ Reversed brackets\n\
            [00:02.00 Valid line without closing bracket\n\
            00:03.00] Valid line without opening bracket\n\
            [a:b] invalid time format\n\
            [0100] missing colon\n\
            [aa:00.00] invalid minute\n\
            [00:aa.00] invalid second\n\
            [00:10.00] Valid lyrics\n\
        ";

        let lines = parse_lrc(lrc_data);

        // It successfully extracts 3 valid timestamps from the edge cases
        assert_eq!(lines.len(), 3);

        // "[01:22] No dot should be ignored safely" parses as 1 min, 22 secs
        assert!((lines[0].start_time_secs - 82.0).abs() < TOLERANCE);
        assert_eq!(lines[0].text.as_ref(), "No dot should be ignored safely");

        // "[03:00.00] Valid amid chaos" parses as 3 mins, 0 secs
        assert!((lines[1].start_time_secs - 180.0).abs() < TOLERANCE);
        assert_eq!(lines[1].text.as_ref(), "Valid amid chaos");

        // "[00:10.00] Valid lyrics" parses as 0 mins, 10 secs
        assert!((lines[2].start_time_secs - 10.0).abs() < TOLERANCE);
        assert_eq!(lines[2].text.as_ref(), "Valid lyrics");
    }
}
