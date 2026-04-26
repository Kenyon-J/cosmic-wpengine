#![cfg(test)]

use super::*;

/// Tests the `tick_transition` method to ensure it correctly updates the transition progress.
/// This prevents bugs where animations might jump abruptly or get stuck, ensuring smooth UX.
#[test]
fn test_state_transitions() {
    let config = Config::default();
    let mut state = AppState::new(config);

    state.begin_transition();
    assert_eq!(state.transition_progress, 0.0);

    state.tick_transition(0.1);
    // 0.1 * 1.5 speed
    assert!((state.transition_progress - 0.15).abs() < f32::EPSILON);

    state.tick_transition(1.0);
    assert_eq!(state.transition_progress, 1.0); // Should be safely capped at 1.0
}

/// Tests that the correct `SceneHint` is returned based on current state parameters like album art presence.
/// This prevents incorrect rendering contexts being applied, such as drawing the visualiser when album art should be shown.
#[test]
fn test_scene_description() {
    let config = Config::default();
    let mut state = AppState::new(config);

    // Default empty state should be Ambient
    assert_eq!(state.scene_description(), SceneHint::Ambient);

    // With significant audio energy, it should switch to AudioVisualiser
    state.audio_energy = 1.0;
    assert_eq!(state.scene_description(), SceneHint::AudioVisualiser);

    // Track with album art should take highest precedence over everything
    state.current_track = Some(TrackInfo {
        title: "Test".into(),
        artist: "Test".into(),
        album: "Test".into(),
        album_art: Some(image::RgbaImage::new(1, 1)),
        palette: None,
        lyrics: None,
        video_url: None,
    });
    state.has_album_art = true;
    assert_eq!(state.scene_description(), SceneHint::AlbumArt);
}

/// Tests edge cases of `scene_description` like empty audio bands and exact threshold values.
/// This prevents divide-by-zero panics and off-by-one errors in state transition logic.
#[test]
fn test_scene_description_edge_cases() {
    let config = Config::default();
    let mut state = AppState::new(config);

    // Edge case 1: zero energy
    state.audio_energy = 0.0;
    assert_eq!(state.scene_description(), SceneHint::Ambient);

    // Edge case 2: exact boundary condition for audio energy (0.05)
    state.audio_energy = 0.05;
    assert_eq!(state.scene_description(), SceneHint::Ambient);

    // Edge case 3: slightly above boundary
    state.audio_energy = 0.05001;
    assert_eq!(state.scene_description(), SceneHint::AudioVisualiser);
}

/// Tests the `update_time` function to verify it bounds time values correctly.
/// This prevents out-of-bounds errors in time-dependent logic like color shifting.
#[test]
fn test_update_time() {
    let config = Config::default();
    let mut state = AppState::new(config);

    // Initially time of day should be within [0.0, 1.0]
    assert!(state.time_of_day >= 0.0 && state.time_of_day <= 1.0);

    // Modify time to out of bounds
    state.time_of_day = 2.0;

    // Call update_time
    state.update_time();

    // Check it's back in valid range
    assert!(state.time_of_day >= 0.0 && state.time_of_day <= 1.0);
}
