use std::time::Instant;

#[derive(Clone, PartialEq, Default)]
struct MetadataUpdate {
    title: String,
    artist: String,
    album: String,
    art_url: Option<String>,
    track_id: String,
}

fn main() {
    let mut last_metadata: Option<MetadataUpdate> = Some(MetadataUpdate {
        title: "Test Title".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        art_url: Some("http://example.com/art.jpg".to_string()),
        track_id: "test_id".to_string(),
    });

    let last_processed_metadata: Option<MetadataUpdate> = None;
    let visible = true;
    let is_timed_out = false;

    let iterations = 10_000_000;

    let start = Instant::now();
    let mut hits_clone = 0;
    for _ in 0..iterations {
        if visible && !is_timed_out && last_metadata != last_processed_metadata {
            if let Some(meta) = last_metadata.clone() {
                hits_clone += 1;
            }
        }
    }
    let duration_clone = start.elapsed();
    println!("With clone: {:?} (hits: {})", duration_clone, hits_clone);

    let start = Instant::now();
    let mut hits_as_ref = 0;
    for _ in 0..iterations {
        if visible && !is_timed_out && last_metadata != last_processed_metadata {
            if let Some(meta) = last_metadata.as_ref() {
                hits_as_ref += 1;
            }
        }
    }
    let duration_as_ref = start.elapsed();
    println!("With as_ref: {:?} (hits: {})", duration_as_ref, hits_as_ref);
}
