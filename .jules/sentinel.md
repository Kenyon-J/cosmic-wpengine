## 2024-05-24 - URL Query Parameter Injection via `format!`
**Vulnerability:** Constructing URL query parameters directly via string interpolation (e.g. `format!("...?id={}", user_input)`) is a critical injection risk, opening up SSRF or arbitrary query parameters if the input contains unescaped special characters.
**Learning:** Found in `mpris.rs` where the `track_id` from external metadata was directly formatted into a proxy URL. This pattern allows an attacker (via maliciously crafted track IDs) to manipulate the outgoing API request.
**Prevention:** Always use safe HTTP client abstractions such as `reqwest`'s `.query(&[("key", value)])` which handles proper URL encoding to prevent injection.
