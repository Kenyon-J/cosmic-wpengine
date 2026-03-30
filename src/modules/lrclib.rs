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
) -> Option<Vec<LyricLine>> {
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

fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();

    for line in lrc.lines() {
        let line = line.trim();
        if let (Some(start), Some(end)) = (line.find('['), line.find(']')) {
            if start < end {
                let time_str = &line[start + 1..end];
                let text = line[end + 1..].trim().to_string();

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
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lrc_valid() {
        let lrc_data = "\
            [00:12.34] First line of lyrics\n\
            [01:05.67] Second line of lyrics\n\
            [02:45.00] Third line of lyrics\
        ";

        let lines = parse_lrc(lrc_data);

        assert_eq!(lines.len(), 3);

        assert!((lines[0].start_time_secs - 12.34).abs() < f32::EPSILON);
        assert_eq!(lines[0].text, "First line of lyrics");

        assert!((lines[1].start_time_secs - 65.67).abs() < f32::EPSILON);
        assert_eq!(lines[1].text, "Second line of lyrics");

        assert!((lines[2].start_time_secs - 165.00).abs() < f32::EPSILON);
        assert_eq!(lines[2].text, "Third line of lyrics");
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

        assert!((lines[0].start_time_secs - 12.34).abs() < f32::EPSILON);
        assert_eq!(lines[0].text, "Valid line");

        assert!((lines[1].start_time_secs - 165.00).abs() < f32::EPSILON);
        assert_eq!(lines[1].text, "Another valid line");
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

        assert!((lines[0].start_time_secs - 1.0).abs() < f32::EPSILON);
        assert_eq!(lines[0].text, "");

        assert!((lines[1].start_time_secs - 5.0).abs() < f32::EPSILON);
        assert_eq!(lines[1].text, "Valid lyrics");
    }

    #[test]
    fn test_parse_lrc_edge_cases() {
        let lrc_data = "\
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

        assert_eq!(lines.len(), 1);
        assert!((lines[0].start_time_secs - 10.0).abs() < f32::EPSILON);
        assert_eq!(lines[0].text, "Valid lyrics");
    }
}
