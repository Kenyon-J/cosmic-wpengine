use std::collections::HashMap;
use std::path::Path;

use sha2::{Digest, Sha256};

const RELEASE_DOWNLOAD_BASE: &str = "https://github.com/Kenyon-J/cosmic-wpengine/releases/download";
const ENGINE_ASSET: &str = "cosmic-wallpaper-x86_64-linux-gnu";
const GUI_ASSET: &str = "cosmic-wallpaper-gui-x86_64-linux-gnu";
const MAX_BINARY_SIZE: usize = 200 * 1024 * 1024; // 200 MB safety cap

/// Minisign public key verifying every release's SHA256SUMS.txt. The matching
/// secret key exists only in the repository's Actions secrets
/// (MINISIGN_SECRET_KEY) - it is deliberately not in the tree, so a
/// compromised GitHub account or CI run can rewrite release assets but cannot
/// produce a signature this updater will accept unless the signing secret
/// also leaks.
const MINISIGN_PUBLIC_KEY: &str = "RWTuwhv3rLFRkAR/0Jr+VAgT1YN+Y+Tu76AUUI3m9sVYOlEAztfseXnS";

/// All release asset downloads are pinned to the tag the user approved in the
/// update dialog. Fetching `releases/latest/...` instead would be a TOCTOU:
/// "latest" can move between the version check and the install, silently
/// swapping which binaries get executed.
fn tagged_download_url(tag: &str, asset: &str) -> String {
    format!("{RELEASE_DOWNLOAD_BASE}/{tag}/{asset}")
}

/// Verifies `sums_text` against a detached minisign signature (`minisig_text`,
/// the full .minisig file contents) using the given base64 public key.
/// Factored out of `perform_update` so tests can exercise it with their own
/// throwaway keypair; production always passes `MINISIGN_PUBLIC_KEY`.
fn verify_sums_signature(
    pk_base64: &str,
    sums_text: &str,
    minisig_text: &str,
) -> Result<(), String> {
    let pk = minisign_verify::PublicKey::from_base64(pk_base64)
        .map_err(|e| format!("Invalid embedded minisign public key: {e}"))?;
    let sig = minisign_verify::Signature::decode(minisig_text)
        .map_err(|e| format!("Malformed SHA256SUMS.txt.minisig: {e}"))?;
    pk.verify(sums_text.as_bytes(), &sig, false).map_err(|e| {
        format!(
            "SHA256SUMS.txt signature verification FAILED ({e}). \
             Refusing to update: the release may have been tampered with."
        )
    })
}

/// Package-managed installs (pacman, apt, ...) live under /usr and must be
/// updated via the system package manager: overwriting them here would
/// fight the package manager's file ownership and get silently reverted on
/// the next system upgrade.
pub fn is_self_updatable() -> bool {
    std::env::current_exe()
        .map(|p| !p.starts_with("/usr"))
        .unwrap_or(false)
}

fn parse_sha256sums(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            let name = parts.next()?;
            Some((name.to_string(), hash.to_lowercase()))
        })
        .collect()
}

async fn download_and_verify(
    client: &reqwest::Client,
    tag: &str,
    asset_name: &str,
    expected_sha256: &str,
) -> Result<Vec<u8>, String> {
    let url = tagged_download_url(tag, asset_name);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} downloading {asset_name}", resp.status()));
    }

    let bytes = cosmic_wallpaper::modules::utils::read_capped(resp, MAX_BINARY_SIZE)
        .await
        .map_err(|e| format!("downloading {asset_name}: {e}"))?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual_hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    if actual_hex != expected_sha256 {
        return Err(format!(
            "Checksum mismatch for {asset_name}: expected {expected_sha256}, got {actual_hex}. \
             Download may be corrupted or tampered with."
        ));
    }

    Ok(bytes)
}

/// Writes `bytes` to a temp file beside `dest`, marks it executable, then
/// atomically renames it over `dest`. Overwriting a running executable's
/// file this way is safe on Linux: the kernel keeps serving the old inode
/// to whatever process currently has it mapped, so this doesn't crash
/// `dest` even if it's executing right now.
fn install_binary(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let tmp = dest.with_extension("update-tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

/// Finds the PID of the process currently running `engine_path`, if any, by
/// scanning /proc/*/exe rather than shelling out to pgrep/pkill: `-x` name
/// matching silently truncates at 15 characters (`cosmic-wallpaper` is 16),
/// and a substring match like `-f cosmic-wallpaper` would also catch
/// `cosmic-wallpaper-gui` — the very process running this update.
fn find_running_engine_pid(engine_path: &Path) -> Option<u32> {
    let canonical_engine = std::fs::canonicalize(engine_path).ok()?;
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        // /proc has plenty of non-PID entries (self, net, sys, cpuinfo, ...).
        // Skip them and keep scanning - `?` here would instead bail out of
        // the whole function the moment any one of them was encountered,
        // silently failing to find a real, later-listed PID.
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if let Ok(target) = std::fs::read_link(entry.path().join("exe")) {
            if target == canonical_engine {
                return Some(pid);
            }
        }
    }
    None
}

/// Terminates the given (already-running, pre-swap) engine PID if any, and
/// launches a fresh instance of the newly-installed binary in its place.
///
/// The PID must be found *before* `install_binary` replaces the file: once
/// the old binary's directory entry is gone, `/proc/<pid>/exe` for the
/// still-running old process reports its path with a `(deleted)` suffix,
/// so a lookup performed after the swap never matches and silently fails
/// to find the process that's still running.
fn terminate_and_relaunch(engine_path: &Path, old_pid: Option<u32>) -> Result<(), String> {
    if let Some(pid) = old_pid {
        // A raw kill(2) syscall rather than shelling out to a `kill` binary:
        // terminating a PID doesn't need an external process, and relying on
        // one being installed at a guessed path is a needless failure mode
        // (e.g. minimal containers/systems without procps-ng).
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
        // Give it a moment to release its Wayland/wgpu resources before
        // the new instance tries to claim the same layer-shell surfaces.
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    std::process::Command::new(engine_path)
        .spawn()
        .map_err(|e| format!("Failed to relaunch engine: {e}"))?;
    Ok(())
}

/// Downloads and verifies the latest release's two binaries, installs them
/// over the current ones, and restarts the background engine process. The
/// GUI's own binary is swapped on disk too, but this process keeps running
/// its old code in memory until Settings is restarted by hand.
pub async fn perform_update(client: reqwest::Client, target_tag: String) -> Result<String, String> {
    let gui_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let install_dir = gui_path
        .parent()
        .ok_or_else(|| "Could not determine install directory".to_string())?
        .to_path_buf();
    let engine_path = install_dir.join("cosmic-wallpaper");

    let sums_resp = client
        .get(tagged_download_url(&target_tag, "SHA256SUMS.txt"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let sums_bytes = cosmic_wallpaper::modules::utils::read_capped(sums_resp, 1024 * 1024)
        .await
        .map_err(|e| e.to_string())?;
    let sums_text = String::from_utf8(sums_bytes).map_err(|e| e.to_string())?;

    // The hashes only authenticate the binaries against SHA256SUMS.txt; the
    // minisign signature authenticates SHA256SUMS.txt itself. Verify it
    // before trusting a single hash in the file.
    let sig_resp = client
        .get(tagged_download_url(&target_tag, "SHA256SUMS.txt.minisig"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let sig_bytes = cosmic_wallpaper::modules::utils::read_capped(sig_resp, 1024 * 1024)
        .await
        .map_err(|e| e.to_string())?;
    let sig_text = String::from_utf8(sig_bytes).map_err(|e| e.to_string())?;
    verify_sums_signature(MINISIGN_PUBLIC_KEY, &sums_text, &sig_text)?;

    let sums = parse_sha256sums(&sums_text);

    let engine_hash = sums
        .get(ENGINE_ASSET)
        .ok_or_else(|| "SHA256SUMS.txt is missing the engine binary entry".to_string())?
        .clone();
    let gui_hash = sums
        .get(GUI_ASSET)
        .ok_or_else(|| "SHA256SUMS.txt is missing the GUI binary entry".to_string())?
        .clone();

    let engine_bytes =
        download_and_verify(&client, &target_tag, ENGINE_ASSET, &engine_hash).await?;
    let gui_bytes = download_and_verify(&client, &target_tag, GUI_ASSET, &gui_hash).await?;

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let old_engine_pid = find_running_engine_pid(&engine_path);
        install_binary(&engine_path, &engine_bytes).map_err(|e| e.to_string())?;
        install_binary(&gui_path, &gui_bytes).map_err(|e| e.to_string())?;
        terminate_and_relaunch(&engine_path, old_engine_pid)
    })
    .await
    .map_err(|e| format!("Update task panicked: {e}"))??;

    Ok(target_tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tagged_download_url_pins_to_tag() {
        assert_eq!(
            tagged_download_url("v1.0.0", "SHA256SUMS.txt"),
            "https://github.com/Kenyon-J/cosmic-wpengine/releases/download/v1.0.0/SHA256SUMS.txt"
        );
    }

    /// Round-trips a signature through the same verification code
    /// `perform_update` runs: a SHA256SUMS body signed by a (test-local,
    /// never committed) keypair must verify, and any post-signing tampering
    /// with the body must be rejected.
    #[test]
    fn test_minisign_signature_roundtrip_and_tamper_rejection() {
        let keypair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let sums_text = "abc123  cosmic-wallpaper-x86_64-linux-gnu\n";

        let sig_box = minisign::sign(
            None,
            &keypair.sk,
            std::io::Cursor::new(sums_text.as_bytes()),
            Some("cosmic-wpengine test signature"),
            None,
        )
        .unwrap();
        let sig_text = sig_box.into_string();
        let pk_base64 = keypair.pk.to_base64();

        verify_sums_signature(&pk_base64, sums_text, &sig_text)
            .expect("genuine signature must verify");

        let tampered = "evil99  cosmic-wallpaper-x86_64-linux-gnu\n";
        assert!(
            verify_sums_signature(&pk_base64, tampered, &sig_text).is_err(),
            "tampered SHA256SUMS body must be rejected"
        );

        let other_keypair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        assert!(
            verify_sums_signature(&other_keypair.pk.to_base64(), sums_text, &sig_text).is_err(),
            "signature from a different key must be rejected"
        );
    }

    /// The embedded production public key must stay parseable — a typo here
    /// would brick every future self-update at the verification step.
    #[test]
    fn test_embedded_public_key_parses() {
        minisign_verify::PublicKey::from_base64(MINISIGN_PUBLIC_KEY)
            .expect("embedded MINISIGN_PUBLIC_KEY must be valid base64 minisign key");
    }

    #[test]
    fn test_parse_sha256sums_valid() {
        let text = "\
            abc123  cosmic-wallpaper-x86_64-linux-gnu\n\
            def456  cosmic-wallpaper-gui-x86_64-linux-gnu\n\
        ";
        let sums = parse_sha256sums(text);
        assert_eq!(sums.get(ENGINE_ASSET).map(String::as_str), Some("abc123"));
        assert_eq!(sums.get(GUI_ASSET).map(String::as_str), Some("def456"));
    }

    #[test]
    fn test_parse_sha256sums_lowercases_hash() {
        let text = "ABC123DEF  cosmic-wallpaper-x86_64-linux-gnu\n";
        let sums = parse_sha256sums(text);
        assert_eq!(
            sums.get(ENGINE_ASSET).map(String::as_str),
            Some("abc123def")
        );
    }

    #[test]
    fn test_parse_sha256sums_ignores_blank_and_malformed_lines() {
        let text = "\n   \nnotahashline\nabc123  file.bin\n";
        let sums = parse_sha256sums(text);
        assert_eq!(sums.len(), 1);
        assert_eq!(sums.get("file.bin").map(String::as_str), Some("abc123"));
    }

    #[test]
    fn test_install_binary_writes_and_makes_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("some-binary");

        install_binary(&dest, b"fake binary contents").unwrap();

        let contents = std::fs::read(&dest).unwrap();
        assert_eq!(contents, b"fake binary contents");
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    #[test]
    fn test_install_binary_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("some-binary");
        std::fs::write(&dest, b"old contents").unwrap();

        install_binary(&dest, b"new contents").unwrap();

        assert_eq!(std::fs::read(&dest).unwrap(), b"new contents");
    }

    #[test]
    fn test_find_running_engine_pid_returns_none_for_nonexistent_path() {
        let dir = tempfile::tempdir().unwrap();
        let never_run = dir.path().join("definitely-not-a-running-process");
        assert_eq!(find_running_engine_pid(&never_run), None);
    }

    #[test]
    fn test_is_self_updatable_true_under_test_runner() {
        // cargo test binaries run from target/debug/deps, never under /usr.
        assert!(is_self_updatable());
    }

    /// Spawns a copy of `sleep` at `path` and returns the child handle. The
    /// caller must SIGTERM and `.wait()` it to avoid leaving a zombie behind
    /// (this test process is its real parent, so nothing else will reap it).
    fn spawn_standin(path: &Path) -> std::process::Child {
        std::fs::copy("/usr/bin/sleep", path).unwrap();
        std::fs::set_permissions(path, {
            use std::os::unix::fs::PermissionsExt;
            std::fs::Permissions::from_mode(0o755)
        })
        .unwrap();
        std::process::Command::new(path)
            .arg("5")
            .spawn()
            .expect("failed to spawn stand-in process")
    }

    fn reap(mut child: std::process::Child) {
        // `child` is our own spawned process, so `Child::kill()` (SIGKILL, no
        // external binary needed) is sufficient - this is cleanup, not a
        // test of the graceful-SIGTERM behavior that's covered below.
        let _ = child.kill();
        let _ = child.wait();
    }

    /// Regression test for a real bug caught during manual verification:
    /// find_running_engine_pid originally used `?` on the parsed PID inside
    /// the /proc scan loop, which bails out of the *whole function* the
    /// moment any non-numeric /proc entry (self, net, cpuinfo, ...) is
    /// encountered, instead of just skipping it - so it essentially never
    /// found a real running process in practice.
    #[test]
    fn test_find_running_engine_pid_finds_real_running_process() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stand-in");
        let child = spawn_standin(&path);
        std::thread::sleep(std::time::Duration::from_millis(200));

        assert_eq!(find_running_engine_pid(&path), Some(child.id()));

        reap(child);
    }

    /// Regression test for a second bug caught during manual verification:
    /// once install_binary renames a new file over a still-running binary's
    /// path, the old process's /proc/<pid>/exe reports the path with a
    /// "(deleted)" suffix, so a lookup performed *after* the swap never
    /// matches. The PID must be captured before the swap, not after.
    #[test]
    fn test_terminate_and_relaunch_kills_pid_captured_before_swap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stand-in");
        let mut child = spawn_standin(&path);
        std::thread::sleep(std::time::Duration::from_millis(200));
        let old_pid = child.id();

        let captured_pid = find_running_engine_pid(&path);
        assert_eq!(captured_pid, Some(old_pid));

        // terminate_and_relaunch respawns engine_path with no arguments
        // (matching the real cosmic-wallpaper binary, which needs none), so
        // the swapped-in stand-in must tolerate that too - unlike `sleep`,
        // `yes` runs indefinitely with zero required arguments.
        let yes_bytes = std::fs::read("/usr/bin/yes").unwrap();
        install_binary(&path, &yes_bytes).unwrap();

        // Documents the "(deleted)" behavior a post-swap lookup would hit.
        assert_eq!(find_running_engine_pid(&path), None);

        terminate_and_relaunch(&path, captured_pid).unwrap();
        let exit_status = child.wait().unwrap();
        assert!(
            !exit_status.success(),
            "expected the old process to be killed by SIGTERM"
        );

        std::thread::sleep(std::time::Duration::from_millis(200));
        let new_pid = find_running_engine_pid(&path);
        assert!(
            new_pid.is_some(),
            "a fresh process should be running at the same path"
        );
        assert_ne!(new_pid, Some(old_pid));

        // `new_pid` is a foreign process (relaunched internally by
        // terminate_and_relaunch, not a Child we hold), so this needs a raw
        // kill(2) rather than Child::kill() - mirrors production cleanup.
        unsafe {
            libc::kill(new_pid.unwrap() as libc::pid_t, libc::SIGTERM);
        }
    }
}
