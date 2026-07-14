#![cfg(test)]

use super::*;
use crate::modules::utils::test_support::ENV_MUTEX;
use std::fs;
use std::path::Path;

/// Tests the `resolve_safe_path` function to ensure it properly blocks path traversal and arbitrary file reads.
/// This prevents untrusted MPRIS metadata (like album art paths) from leaking sensitive local files.
///
/// Mutates the process-global HOME env var, so it must hold the shared
/// ENV_MUTEX (see `config::tests`) to avoid racing other tests that read or
/// mutate HOME/XDG_CONFIG_HOME concurrently.
#[test]
fn test_resolve_safe_path() {
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let old_home = std::env::var("HOME").ok();

    let result = std::panic::catch_unwind(run_test_resolve_safe_path);

    if let Some(old) = old_home {
        std::env::set_var("HOME", old);
    } else {
        std::env::remove_var("HOME");
    }

    if let Err(err) = result {
        std::panic::resume_unwind(err);
    }
}

/// Restores HOME even if an assertion panics, so a failure here can't leave
/// HOME pointing at a since-deleted tempdir for whichever test runs next.
fn run_test_resolve_safe_path() {
    // Create a mock home directory structure outside of /tmp and /run/user
    // to prevent false-positives since /tmp is allowed by default.
    // In many build environments, tempdir() creates inside /tmp.
    // Instead we use a path relative to the test runner or a custom directory.
    let base_dir = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let home_dir = tempfile::Builder::new()
        .prefix("mpris_mock_home")
        .tempdir_in(base_dir)
        .unwrap();
    let home_path = home_dir.path();
    std::env::set_var("HOME", home_path);

    let music_dir = home_path.join("Music");
    let cache_dir = home_path.join(".cache");
    let share_dir = home_path.join(".local/share");
    let ssh_dir = home_path.join(".ssh");

    fs::create_dir_all(&music_dir).unwrap();
    fs::create_dir_all(&cache_dir).unwrap();
    fs::create_dir_all(&share_dir).unwrap();
    fs::create_dir_all(&ssh_dir).unwrap();

    // Create test files to allow canonicalize to succeed
    let art1 = music_dir.join("cover.png");
    let art2 = cache_dir.join("art.jpg");
    let art3 = share_dir.join("player/art.png");
    let rsa_key = ssh_dir.join("id_rsa");
    let doc = home_path.join("document.pdf");

    fs::create_dir_all(art3.parent().unwrap()).unwrap();
    fs::write(&art1, "").unwrap();
    fs::write(&art2, "").unwrap();
    fs::write(&art3, "").unwrap();
    fs::write(&rsa_key, "").unwrap();
    fs::write(&doc, "").unwrap();

    // For /tmp and /run/user, we create actual files there for testing since they usually exist.
    // However, to be robust, we'll create unique temp directories inside /tmp.
    let tmp_test_dir = tempfile::Builder::new()
        .prefix("mpris_test_tmp")
        .tempdir_in("/tmp")
        .unwrap();
    let tmp_art = tmp_test_dir.path().join("art.png");
    fs::write(&tmp_art, "").unwrap();

    let run_user_test_dir = match std::fs::create_dir_all("/run/user/1000/mpris_test") {
        Ok(_) => Some(Path::new("/run/user/1000/mpris_test").to_path_buf()),
        Err(_) => None, // /run/user/1000 might not exist on the test machine
    };

    let mut run_user_art = None;
    if let Some(ref dir) = run_user_test_dir {
        let path = dir.join("art.jpg");
        if fs::write(&path, "").is_ok() {
            run_user_art = Some(path);
        }
    }

    // Valid absolute paths in safe locations
    assert!(MprisWatcher::resolve_safe_path(&tmp_art).is_some());
    if let Some(ref p) = run_user_art {
        assert!(MprisWatcher::resolve_safe_path(p).is_some());
    }
    assert!(MprisWatcher::resolve_safe_path(&art1).is_some());
    assert!(MprisWatcher::resolve_safe_path(&art2).is_some());
    assert!(MprisWatcher::resolve_safe_path(&art3).is_some());

    // Path traversal attempts
    let fake_passwd = home_path.join("passwd");
    fs::write(&fake_passwd, "").unwrap();
    assert!(MprisWatcher::resolve_safe_path(&tmp_test_dir.path().join("../etc/passwd")).is_none());
    assert!(MprisWatcher::resolve_safe_path(Path::new("/run/user/../../var/log")).is_none());

    // Blocked home directory access attempts
    assert!(MprisWatcher::resolve_safe_path(&rsa_key).is_none());
    assert!(MprisWatcher::resolve_safe_path(&doc).is_none());

    // Relative paths
    assert!(MprisWatcher::resolve_safe_path(Path::new("art.png")).is_none());
    assert!(MprisWatcher::resolve_safe_path(Path::new("./art.png")).is_none());

    // Symlink bypass attempt
    let symlink_path = tmp_test_dir.path().join("symlink_to_passwd.png");
    // Create a symlink pointing to an unsafe location (e.g. the fake passwd file or another file outside whitelist)
    let fake_unsafe = home_path.join("unsafe.txt");
    fs::write(&fake_unsafe, "").unwrap();
    std::os::unix::fs::symlink(&fake_unsafe, &symlink_path).unwrap();

    // The symlink is inside /tmp, but it points to an unsafe location.
    // It should be rejected.
    assert!(MprisWatcher::resolve_safe_path(&symlink_path).is_none());
}

/// SSRF guard for album-art URLs: every internal, translation, and
/// special-purpose range must be rejected so untrusted MPRIS metadata can't
/// make the engine probe the local host or network.
#[test]
fn test_is_safe_ip() {
    use std::net::{Ipv4Addr, Ipv6Addr};

    let blocked_v4 = [
        Ipv4Addr::new(127, 0, 0, 1),       // loopback
        Ipv4Addr::new(10, 0, 0, 1),        // private
        Ipv4Addr::new(172, 16, 0, 1),      // private
        Ipv4Addr::new(192, 168, 1, 1),     // private
        Ipv4Addr::new(169, 254, 1, 1),     // link-local
        Ipv4Addr::new(0, 0, 0, 0),         // unspecified
        Ipv4Addr::new(0, 1, 2, 3),         // 0.0.0.0/8
        Ipv4Addr::new(255, 255, 255, 255), // broadcast
        Ipv4Addr::new(224, 0, 0, 1),       // multicast
        Ipv4Addr::new(100, 64, 0, 1),      // shared / CGNAT
        Ipv4Addr::new(100, 127, 255, 254), // shared / CGNAT upper edge
        Ipv4Addr::new(192, 0, 0, 8),       // IETF protocol assignments
        Ipv4Addr::new(198, 18, 0, 1),      // benchmarking
        Ipv4Addr::new(198, 19, 255, 254),  // benchmarking upper edge
        Ipv4Addr::new(240, 0, 0, 1),       // reserved
    ];
    for ip in blocked_v4 {
        assert!(!is_safe_ip(IpAddr::V4(ip)), "{ip} should be blocked");
    }

    let allowed_v4 = [
        Ipv4Addr::new(93, 184, 216, 34),
        Ipv4Addr::new(100, 63, 0, 1),  // just below the CGNAT range
        Ipv4Addr::new(100, 128, 0, 1), // just above the CGNAT range
        Ipv4Addr::new(192, 0, 1, 1),   // adjacent to 192.0.0.0/24
        Ipv4Addr::new(198, 17, 0, 1),  // just below benchmarking
        Ipv4Addr::new(198, 20, 0, 1),  // just above benchmarking
    ];
    for ip in allowed_v4 {
        assert!(is_safe_ip(IpAddr::V4(ip)), "{ip} should be allowed");
    }

    let blocked_v6: [Ipv6Addr; 8] = [
        "::1".parse().unwrap(),              // loopback
        "::".parse().unwrap(),               // unspecified
        "fc00::1".parse().unwrap(),          // unique local
        "fe80::1".parse().unwrap(),          // link local
        "ff02::1".parse().unwrap(),          // multicast
        "::ffff:127.0.0.1".parse().unwrap(), // v4-mapped loopback
        "::10.0.0.1".parse().unwrap(),       // deprecated v4-compatible private
        "64:ff9b::7f00:1".parse().unwrap(),  // NAT64-embedded 127.0.0.1
    ];
    for ip in blocked_v6 {
        assert!(!is_safe_ip(IpAddr::V6(ip)), "{ip} should be blocked");
    }

    let allowed_v6: Ipv6Addr = "2606:2800:220:1:248:1893:25c8:1946".parse().unwrap();
    assert!(is_safe_ip(IpAddr::V6(allowed_v6)));
    let mapped_public: Ipv6Addr = "::ffff:93.184.216.34".parse().unwrap();
    assert!(is_safe_ip(IpAddr::V6(mapped_public)));
}
