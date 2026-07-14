#![cfg(test)]

use super::*;

// A safer tolerance for floating point time comparisons (1 millisecond)
const TOLERANCE: f32 = 0.001;

/// Tests basic parsing of valid LRC formatted strings into usable line structs.
/// This ensures synced lyrics correctly sync to the audio playback times.
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
    assert!(lines[0].text_hash != 0);

    assert!((lines[1].start_time_secs - 65.67).abs() < TOLERANCE);
    assert_eq!(lines[1].text.as_ref(), "Second line of lyrics");
    assert!(lines[1].text_hash != 0);

    assert!((lines[2].start_time_secs - 165.00).abs() < TOLERANCE);
    assert_eq!(lines[2].text.as_ref(), "Third line of lyrics");
    assert!(lines[2].text_hash != 0);
}

/// Tests that the parser gracefully ignores rows with malformed timestamps instead of panicking.
/// This prevents application crashes when confronted with poorly formatted community lyrics.
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

/// Tests that the parser safely processes empty text lines and skips metadata tags (like `[ti:Title]`).
/// This ensures structural spaces in lyrics are preserved while unrenderable tags are hidden.
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

/// Tests that lines are sorted into ascending start_time_secs order regardless
/// of the order they appear in the source text. The renderer's lyric-scroll
/// tracking (both its O(1) fast path and its partition_point fallback) relies
/// on this invariant, and community-submitted LRC files aren't guaranteed to
/// list lines in chronological order.
#[test]
fn test_parse_lrc_sorts_out_of_order_timestamps() {
    let lrc_data = "\
        [02:00.00] Third chronologically\n\
        [00:05.00] First chronologically\n\
        [01:00.00] Second chronologically\
    ";

    let lines = parse_lrc(lrc_data);

    assert_eq!(lines.len(), 3);
    assert!((lines[0].start_time_secs - 5.0).abs() < TOLERANCE);
    assert_eq!(lines[0].text.as_ref(), "First chronologically");
    assert!((lines[1].start_time_secs - 60.0).abs() < TOLERANCE);
    assert_eq!(lines[1].text.as_ref(), "Second chronologically");
    assert!((lines[2].start_time_secs - 120.0).abs() < TOLERANCE);
    assert_eq!(lines[2].text.as_ref(), "Third chronologically");
}

/// Tests the compressed LRC form where one line carries several timestamps
/// ("[t1][t2]text") to repeat the same text - common in community files for
/// choruses. Each timestamp must yield its own entry with the bare text, and
/// none of the timestamps may leak into the rendered lyric.
#[test]
fn test_parse_lrc_multiple_timestamps_per_line() {
    let lrc_data = "\
        [00:10.00][01:10.00] Repeated chorus line\n\
        [00:30.00] [00:50.00] Spaced repeated tags\n\
        [ar:Artist][00:40.00] Metadata tag mixed with a timestamp\n\
        [00:20.00] Ordinary line\
    ";

    let lines = parse_lrc(lrc_data);

    assert_eq!(lines.len(), 6);

    let expected = [
        (10.0, "Repeated chorus line"),
        (20.0, "Ordinary line"),
        (30.0, "Spaced repeated tags"),
        (40.0, "Metadata tag mixed with a timestamp"),
        (50.0, "Spaced repeated tags"),
        (70.0, "Repeated chorus line"),
    ];
    for (line, (time, text)) in lines.iter().zip(expected) {
        assert!((line.start_time_secs - time).abs() < TOLERANCE);
        assert_eq!(line.text.as_ref(), text);
    }

    // Repeated entries share one hash since they share the same text.
    assert_eq!(lines[0].text_hash, lines[5].text_hash);
}

/// Tests edge cases of LRC syntax like missing colons, dots, reversed brackets, or extra bracket layers.
/// This guarantees robust fallback mechanisms so the lyrics parser is as resilient as possible.
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

    // It successfully extracts 3 valid timestamps from the edge cases,
    // sorted into ascending chronological order rather than file order.
    assert_eq!(lines.len(), 3);

    // "[00:10.00] Valid lyrics" parses as 0 mins, 10 secs
    assert!((lines[0].start_time_secs - 10.0).abs() < TOLERANCE);
    assert_eq!(lines[0].text.as_ref(), "Valid lyrics");

    // "[01:22] No dot should be ignored safely" parses as 1 min, 22 secs
    assert!((lines[1].start_time_secs - 82.0).abs() < TOLERANCE);
    assert_eq!(lines[1].text.as_ref(), "No dot should be ignored safely");

    assert!((lines[2].start_time_secs - 180.0).abs() < TOLERANCE);
    assert_eq!(lines[2].text.as_ref(), "Valid amid chaos");
}
