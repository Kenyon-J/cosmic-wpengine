use anyhow::Result;
use tracing::{info, warn};
use url::Url;

use super::MprisWatcher;
use crate::modules::colour::extract_palette;
use crate::modules::utils::is_safe_ip;

/// URL schemes are case-insensitive (RFC 3986), and some players report
/// uppercase ones - a plain `starts_with("http")` would misroute those to the
/// local-file path.
pub(super) fn is_http_url(url: &str) -> bool {
    url.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("http"))
}

#[derive(serde::Deserialize)]
struct ITunesResponse {
    results: Vec<ITunesResult>,
}

#[derive(serde::Deserialize)]
struct ITunesResult {
    #[serde(rename = "artworkUrl100")]
    artwork_url: Option<String>,
}

#[derive(serde::Deserialize)]
struct CanvasResponse {
    #[serde(rename = "canvas_url")]
    url: Option<String>,
}

impl MprisWatcher {
    /// Resizes oversized art and derives its colour palette (reusing a cached
    /// palette when supplied), off the async executor.
    pub(super) async fn process_art(
        img: image::DynamicImage,
        cached_palette: Option<Box<[[f32; 3]]>>,
    ) -> (Option<image::RgbaImage>, Option<Box<[[f32; 3]]>>) {
        // Optimization: Offload heavy CPU-bound palette extraction and image conversion
        // to a dedicated blocking thread. This saves ~50-100ms of executor stall time.
        tokio::task::spawn_blocking(move || {
            let palette = cached_palette.unwrap_or_else(|| extract_palette(&img));

            // Optimisation: Limit album art size to 1024x1024 to prevent massive RAM usage.
            let resized_img = if img.width() > 1024 || img.height() > 1024 {
                img.thumbnail(1024, 1024)
            } else {
                img
            };

            (Some(resized_img.into_rgba8()), Some(palette))
        })
        .await
        .unwrap_or((None, None))
    }

    /// Dynamic gradient stand-in for when no art can be found anywhere.
    pub(super) async fn generate_placeholder_art() -> Option<image::DynamicImage> {
        tokio::task::spawn_blocking(|| {
            let mut img = image::RgbaImage::new(640, 640);
            for y in 0..640 {
                for x in 0..640 {
                    let r = ((x as f32 / 640.0) * 80.0) as u8 + 20;
                    let b = ((y as f32 / 640.0) * 80.0) as u8 + 40;
                    img.put_pixel(x, y, image::Rgba([r, 20, b, 255]));
                }
            }
            image::DynamicImage::ImageRgba8(img)
        })
        .await
        .ok()
    }

    fn decode_image_safely(bytes: impl AsRef<[u8]>) -> Result<image::DynamicImage> {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| anyhow::anyhow!("Failed to guess image format: {}", e))?;

        let mut limits = image::Limits::default();
        // Prevent OOM from maliciously crafted or ultra-high-res local album art.
        // Caps to 4K resolution (~67MB uncompressed RGB)
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(1024 * 1024 * 128); // 128 MB maximum buffer allocation
        reader.limits(limits);

        reader
            .decode()
            .map_err(|e| anyhow::anyhow!("Failed to decode image: {}", e))
    }

    pub(super) async fn fetch_album_art(url_str: &str) -> Result<image::DynamicImage> {
        info!("Attempting to fetch album art from: {}", url_str);
        if is_http_url(url_str) {
            let parsed_url = Url::parse(url_str)?;
            let host_str = parsed_url
                .host_str()
                .ok_or_else(|| anyhow::anyhow!("No host in URL"))?;
            let port = parsed_url.port_or_known_default().unwrap_or(80);

            let mut safe_addr = None;
            let host_port = format!("{}:{}", host_str, port);
            if let Ok(mut addrs) = tokio::net::lookup_host(&host_port).await {
                for addr in addrs.by_ref() {
                    if is_safe_ip(addr.ip()) {
                        safe_addr = Some(addr);
                        break;
                    }
                }
            }

            let safe_addr =
                safe_addr.ok_or_else(|| anyhow::anyhow!("No safe IP found (SSRF protection)"))?;

            let safe_client = reqwest::Client::builder()
                .user_agent("cosmic-wallpaper/1.0")
                .timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .resolve(host_str, safe_addr)
                .build()?;

            let response = safe_client.get(url_str).send().await.map_err(|e| {
                warn!("HTTP request failed for art: {}", e);
                e
            })?;

            const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024; // 10 MB limit
            let bytes = crate::modules::utils::read_capped(response, MAX_IMAGE_SIZE).await?;

            // Optimization: Image decoding is a synchronous, CPU-intensive task.
            // Offloading this to spawn_blocking prevents it from stalling the main async executor.
            return tokio::task::spawn_blocking(move || {
                Self::decode_image_safely(&bytes).map_err(|e| {
                    warn!("Failed to decode HTTP image data: {}", e);
                    e
                })
            })
            .await
            .map_err(|e| anyhow::anyhow!("Image decoding task panicked: {}", e))?;
        }

        // Use the `url` crate for robust parsing of file:// paths
        if let Ok(url) = Url::parse(url_str) {
            if url.scheme() == "file" {
                if let Ok(path) = url.to_file_path() {
                    // `resolve_safe_path` does several `std::fs::canonicalize`
                    // calls - real syscalls, not awaited - so run it on the
                    // blocking pool rather than stalling this task (and, since
                    // MPRIS metadata handling isn't isolated per-player, every
                    // other player's track-change processing riding the same
                    // executor) on a slow or unresponsive filesystem.
                    let path_owned = path.clone();
                    let real_path = match tokio::task::spawn_blocking(move || {
                        Self::resolve_safe_path(&path_owned)
                    })
                    .await
                    {
                        Ok(Some(p)) => p,
                        Ok(None) => anyhow::bail!(
                            "Security violation: Attempted path traversal via file:// URL: {:?}",
                            path
                        ),
                        Err(e) => {
                            anyhow::bail!("path safety check task panicked: {e}")
                        }
                    };
                    info!("Successfully parsed file path: {:?}", real_path);
                    let bytes = tokio::fs::read(&real_path).await.map_err(|e| {
                        warn!(
                            "Failed to read art file from disk at {:?}: {}",
                            real_path, e
                        );
                        e
                    })?;
                    return tokio::task::spawn_blocking(move || {
                        Self::decode_image_safely(&bytes).map_err(|e| {
                            warn!("Failed to decode image from disk {:?}: {}", real_path, e);
                            e
                        })
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("Image decoding task panicked: {}", e))?;
                }
                warn!(
                    "Could not cleanly convert URL to valid file path: {}",
                    url_str
                );
            }
        }

        // Fallback for absolute paths that are not valid file URLs (e.g. /tmp/art.png)
        info!("Attempting raw path fallback read for: {}", url_str);
        // See the file:// branch above for why this runs on the blocking pool.
        let path_owned = std::path::PathBuf::from(url_str);
        let real_path =
            match tokio::task::spawn_blocking(move || Self::resolve_safe_path(&path_owned)).await {
                Ok(Some(p)) => p,
                Ok(None) => anyhow::bail!(
                    "Security violation: Attempted path traversal or unsafe raw path: {}",
                    url_str
                ),
                Err(e) => anyhow::bail!("path safety check task panicked: {e}"),
            };

        let bytes = tokio::fs::read(&real_path).await.map_err(|e| {
            warn!("Failed to read raw path {}: {}", url_str, e);
            e
        })?;
        let url_str_owned = url_str.to_string();
        tokio::task::spawn_blocking(move || {
            Self::decode_image_safely(&bytes).map_err(|e| {
                warn!(
                    "Failed to decode image from raw path {}: {}",
                    url_str_owned, e
                );
                e
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("Image decoding task panicked: {}", e))?
    }

    pub(super) async fn fetch_fallback_album_art(
        artist: &str,
        album: &str,
        title: &str,
        client: &reqwest::Client,
    ) -> Result<image::DynamicImage> {
        let search_str = if album.is_empty() || album == "Unknown" {
            format!("{} {}", artist, title)
        } else {
            format!("{} {}", artist, album)
        };

        let response = client
            .get("https://itunes.apple.com/search")
            .query(&[
                ("term", search_str.as_str()),
                ("entity", "song"),
                ("limit", "1"),
            ])
            .send()
            .await?;

        const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10 MB limit
        let bytes = crate::modules::utils::read_capped(response, MAX_JSON_SIZE).await?;
        let resp: ITunesResponse = serde_json::from_slice(&bytes)?;

        if let Some(first) = resp.results.first() {
            if let Some(art_url) = &first.artwork_url {
                let high_res_url = art_url.replace("100x100bb", "600x600bb");
                return Self::fetch_album_art(&high_res_url).await;
            }
        }
        anyhow::bail!("No fallback art found on iTunes")
    }

    pub(super) fn resolve_safe_path(path: &std::path::Path) -> Option<std::path::PathBuf> {
        // Ensure path is absolute and does not contain any '..' components
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return None;
        }

        // Canonicalize to resolve symlinks and prevent bypasses.
        // If the path does not exist, we reject it.
        let real_path = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return None,
        };

        // Restrict to common album art locations for desktop media players:
        // 1. /tmp/ (used by some players for temporary art)
        // 2. /run/user/ (used by some players for art storage)
        // 3. ~/Music, ~/.cache and ~/.local/share (players' media libraries
        //    and art caches) - deliberately NOT all of HOME, so untrusted
        //    MPRIS metadata can't point us at ~/.ssh and friends.
        let safe_prefixes = [
            std::path::Path::new("/tmp"),
            std::path::Path::new("/run/user"),
        ];

        if safe_prefixes
            .iter()
            .any(|p| std::fs::canonicalize(p).is_ok_and(|real_p| real_path.starts_with(real_p)))
        {
            return Some(real_path);
        }

        if let Ok(home) = std::env::var("HOME") {
            let home_path = std::path::Path::new(&home);

            // The firefox-mpris directories are where Firefox exports the
            // artwork for whatever is playing in a tab: ~/.mozilla for stock
            // builds, ~/.config/mozilla for XDG-enabled builds (Arch), and
            // the .var path for the Flatpak. Only the art subdirectory is
            // allowed, not the whole profile (which holds cookies/logins).
            for subdir in [
                "Music",
                ".cache",
                ".local/share",
                ".mozilla/firefox/firefox-mpris",
                ".config/mozilla/firefox/firefox-mpris",
                ".var/app/org.mozilla.firefox/.mozilla/firefox/firefox-mpris",
            ] {
                if let Ok(real_prefix) = std::fs::canonicalize(home_path.join(subdir)) {
                    if real_path.starts_with(real_prefix) {
                        return Some(real_path);
                    }
                }
            }
        }

        None
    }

    pub(super) async fn fetch_spotify_canvas(
        track_id: &str,
        proxy_url: Option<&str>,
        client: &reqwest::Client,
    ) -> Option<String> {
        // Note: The official Spotify Web API does NOT expose Canvas URLs.
        // To get them, the community routes requests through API proxies that
        // handle the internal gRPC/Protobuf token auth (e.g. 'spotify-canvas-api').
        // The proxy is configured via `audio.canvas_proxy_url` and canvas is
        // disabled when unset: defaulting to a well-known localhost port would
        // let any local process impersonate the proxy and feed the engine
        // attacker-controlled video URLs.
        let proxy_url = proxy_url?;

        let parsed_url = url::Url::parse(proxy_url).ok()?;
        let host_str = parsed_url.host_str()?;
        let port = parsed_url.port_or_known_default().unwrap_or(80);

        let host_port = format!("{}:{}", host_str, port);

        // SSRF Guard (Same tradeoff as video decoder URL fetching): Ensure
        // the resolved host address is a safe IP before proceeding with the request.
        // This mitigates attacks where users inject local endpoints.
        let mut all_safe = true;
        let mut has_addrs = false;
        if let Ok(mut addrs) = tokio::net::lookup_host(&host_port).await {
            for addr in addrs.by_ref() {
                has_addrs = true;
                if !crate::modules::utils::is_safe_ip(addr.ip()) {
                    all_safe = false;
                    break;
                }
            }
        }

        if !has_addrs || !all_safe {
            warn!("Security violation: canvas proxy URL host '{}' resolves to a non-public address (SSRF protection)", host_str);
            return None;
        }

        if let Ok(resp) = client
            .get(proxy_url)
            .query(&[("track_id", track_id)])
            .send()
            .await
        {
            const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10 MB limit
            let bytes = crate::modules::utils::read_capped(resp, MAX_JSON_SIZE)
                .await
                .ok()?;
            if let Ok(canvas) = serde_json::from_slice::<CanvasResponse>(&bytes) {
                return canvas.url;
            }
        }
        None
    }
}
