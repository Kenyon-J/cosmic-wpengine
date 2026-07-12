use std::path::{Path, PathBuf};

pub fn resolve_binary(name: &str) -> Option<PathBuf> {
    let trusted_paths = ["/usr/bin", "/bin", "/usr/local/bin", "/opt/homebrew/bin"];
    for path in trusted_paths {
        let full_path = Path::new(path).join(name);
        if full_path.exists() {
            return Some(full_path);
        }
    }
    None
}

#[cfg(test)]
pub mod test_support {
    use std::sync::Mutex;

    /// Guards tests that mutate process-global environment variables (HOME,
    /// XDG_CONFIG_HOME, ...). `cargo test` runs tests in parallel threads by
    /// default, and env vars are shared process state, so any two tests that
    /// touch the same variable without sharing this lock can interleave and
    /// flake. Every test file that sets these vars must lock this, not a
    /// module-local mutex of its own.
    pub static ENV_MUTEX: Mutex<()> = Mutex::new(());
}
