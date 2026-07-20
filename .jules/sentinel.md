## 2026-07-20 - Unbounded memory allocation via `.text().await`
**Vulnerability:** The `perform_update` function in `src/modules/gui/updater.rs` downloads the `SHA256SUMS.txt` and its signature using `.text().await`. This reads the entire body into memory as a `String` without size limits. While these are GitHub releases endpoints, there is no size cap, meaning a tampered or excessively large release asset (e.g. gigabytes of text) could exhaust memory and cause an OOM Denial-of-Service.
**Learning:** `reqwest::Response::text().await` buffers the entire body into memory unbounded.
**Prevention:** Use `read_capped` (already defined in `src/modules/utils.rs`) with a sensible limit to safely read remote response bodies before converting them into strings.
