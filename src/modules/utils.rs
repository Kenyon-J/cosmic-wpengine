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
