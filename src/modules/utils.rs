use std::net::IpAddr;
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

/// SSRF guard shared by every code path that turns an untrusted URL into an
/// outbound request (album-art fetches in `mpris`, the canvas video decoder in
/// `video`): rejects addresses in internal, translation, and special-purpose
/// ranges so untrusted metadata can't make the engine probe the local host or
/// network.
pub fn is_safe_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let o = ipv4.octets();
            // Ranges without stable std helpers (as of our MSRV):
            let is_shared = o[0] == 100 && (o[1] & 0b1100_0000) == 64; // 100.64.0.0/10 (CGNAT)
            let is_ietf_protocol = o[0] == 192 && o[1] == 0 && o[2] == 0; // 192.0.0.0/24
            let is_benchmarking = o[0] == 198 && (o[1] & 0xfe) == 18; // 198.18.0.0/15
            let is_reserved = (o[0] & 0xf0) == 240; // 240.0.0.0/4 (incl. broadcast)

            !ipv4.is_loopback()
                && !ipv4.is_private()
                && !ipv4.is_link_local()
                && !ipv4.is_unspecified()
                && !ipv4.is_broadcast()
                && !ipv4.is_multicast()
                && !is_shared
                && !is_ietf_protocol
                && !is_benchmarking
                && !is_reserved
                && o[0] != 0 // Block the entire 0.0.0.0/8 block
        }
        IpAddr::V6(ipv6) => {
            // to_ipv4() also catches the deprecated IPv4-compatible form
            // (::a.b.c.d), which to_ipv4_mapped() alone would let through.
            if let Some(mapped_v4) = ipv6.to_ipv4_mapped().or_else(|| ipv6.to_ipv4()) {
                return is_safe_ip(IpAddr::V4(mapped_v4));
            }
            let seg = ipv6.segments();
            let is_unique_local = (seg[0] & 0xfe00) == 0xfc00;
            let is_link_local = (seg[0] & 0xffc0) == 0xfe80;
            // 64:ff9b::/32 - NAT64/DNS64 translation prefixes embed an IPv4
            // address, letting a v6 literal smuggle a request to a v4 target.
            let is_nat64 = seg[0] == 0x64 && seg[1] == 0xff9b;

            !ipv6.is_loopback()
                && !ipv6.is_unspecified()
                && !ipv6.is_multicast()
                && !is_unique_local
                && !is_link_local
                && !is_nat64
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// SSRF guard for untrusted URLs: every internal, translation, and
    /// special-purpose range must be rejected so untrusted MPRIS metadata
    /// can't make the engine probe the local host or network.
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
}
