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
