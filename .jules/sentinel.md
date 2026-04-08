## 2024-05-24 - URL Query Parameter Injection via `format!`
**Vulnerability:** Constructing URL query parameters directly via string interpolation (e.g. `format!("...?id={}", user_input)`) is a critical injection risk, opening up SSRF or arbitrary query parameters if the input contains unescaped special characters.
**Learning:** Found in `mpris.rs` where the `track_id` from external metadata was directly formatted into a proxy URL. This pattern allows an attacker (via maliciously crafted track IDs) to manipulate the outgoing API request.
**Prevention:** Always use safe HTTP client abstractions such as `reqwest`'s `.query(&[("key", value)])` which handles proper URL encoding to prevent injection.
## 2024-04-02 - URL Query Parameter Injection via format!
**Vulnerability:** Constructing HTTP URLs using string interpolation (`format!`) for query parameters (e.g. `format!("...?lat={}&lon={}", lat, lon)`).
**Learning:** Even if the current types (like `f64`) aren't exploitable, this is a vulnerability pattern and anti-pattern that can lead to SSRF or query parameter injection if types change or new string parameters are added.
**Prevention:** Always utilize safe HTTP client abstractions such as `reqwest`'s `.query(&[...])` method which handles proper URL encoding automatically.
## 2025-02-26 - [HIGH] Fix Command Injection / Arbitrary URL Execution via FFmpeg
**Vulnerability:** The application passed an unvalidated URL directly to FFmpeg's `-i` parameter. FFmpeg supports numerous protocols (e.g., `concat://`, `file://`), allowing a malicious media metadata source or Spotify canvas proxy to force the application to read local files or perform SSRF.
**Learning:** External URLs passed to powerful media processing tools like FFmpeg must always be validated and sanitized. FFmpeg has its own internal protocol handlers that bypass standard OS-level path or URL checks.
**Prevention:** Always parse untrusted URLs to verify their scheme (`http`, `https`). Additionally, use FFmpeg's `-protocol_whitelist` flag to restrict the allowed protocols (e.g., `http,https,tcp,tls,crypto`) to prevent internal protocol smuggling or redirection to dangerous handlers.
## 2025-02-27 - [MEDIUM] Fix TOCTOU Vulnerability in Theme Creation
**Vulnerability:** A Time-of-Check to Time-of-Use (TOCTOU) vulnerability existed in `src/modules/gui.rs` when creating new themes. The code checked `!path.exists()` before using `std::fs::write()`. In a concurrent environment, another process could create a symlink or file at that path in the split second between the check and the write operation, potentially allowing file overwriting or privilege escalation.
**Learning:** File existence checks followed by file creation are inherently racy.
**Prevention:** Avoid Time-Of-Check to Time-Of-Use (TOCTOU) race conditions during file I/O by directly attempting to read/write the file and handling errors. For reads, use `std::fs::read_to_string` and match `std::io::ErrorKind::NotFound`. For atomic file creation, use `std::fs::OpenOptions::new().write(true).create_new(true).open(...)` instead of first checking file existence with `path.exists()`.
## 2025-02-28 - [MEDIUM] Fix TOCTOU Vulnerability in Default Theme Creation
**Vulnerability:** A Time-of-Check to Time-of-Use (TOCTOU) vulnerability existed in `src/modules/config.rs` when writing the default theme files. The code used `if !path.exists() { std::fs::write(...) }`. Another process could potentially create the file or symlink after the `exists()` check but before the `write()`, causing the application to potentially overwrite unintended files.
**Learning:** Checking file existence before creation (via `.exists()`) creates an unsafe race condition.
**Prevention:** Use `std::fs::OpenOptions::new().write(true).create_new(true).open(...)` instead to atomically open and create the file. If the file already exists, it will return an `AlreadyExists` error which can be gracefully ignored without risking file overwrite.
## 2025-03-01 - [HIGH] Path Traversal via Path::join with Absolute Paths
**Vulnerability:** User-controlled configuration values used in `Path::join` can result in path traversal or complete path replacement if the input is an absolute path. In `src/main.rs`, the `video_background_path` config was joined directly to the base directory, allowing an attacker to escape the `videos` directory.
**Learning:** `std::path::Path::join` in Rust completely replaces the existing path if the argument is an absolute path (e.g., `/etc/passwd`). This makes it a critical vector for arbitrary file access if the input is unvalidated.
**Prevention:** Always extract the file name using `.file_name()` before joining user-controlled paths to base directories, or explicitly validate that the path does not contain path separators.
