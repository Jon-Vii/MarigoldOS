//! Runtime encoder for the onboarding hotspot's join QR.
//!
//! The hotspot's WPA2 PSK is minted per portal session on the device, so
//! the join code cannot be a build-time constant the way `qr_generated`'s
//! portal-URL QR is: the `WIFI:T:WPA;S:<ssid>;P:<psk>;;` payload is
//! assembled and encoded when the sync-portal screen renders, which is
//! not latency-sensitive. Encoding is Nayuki's qrcodegen in its no-heap
//! port (`qrcodegen-no-heap`, MIT) — `no_std`, caller-provided buffers —
//! so the same path serves the firmware, the host emulator, and the wasm
//! web emulator.

use qrcodegen_no_heap::{QrCode, QrCodeEcc, Version};

/// The onboarding hotspot's SSID: the `S:` field of the QR payload and
/// the network the Wireless screen's caption names. The firmware's AP
/// config must beacon exactly this. Board-named so an X3 doesn't
/// advertise itself as an X4; both spellings are nine bytes, so the QR
/// payload shape is identical.
pub const PORTAL_SSID: &str = if display::DEVICE_IS_X3 {
    "XTEINK-X3"
} else {
    "XTEINK-X4"
};

/// Highest QR version the scratch buffers accommodate. The payload has a
/// fixed shape — 18 bytes of `WIFI:` scaffolding around the 9-byte SSID
/// and 16-byte PSK, 43 bytes total — which byte mode fits in version 4
/// at EC level M (62-byte capacity; version 3's 42 misses by one), a
/// 33-module symbol. Version 5 leaves the encoder one version of slack
/// without growing the buffers past 173 bytes each.
pub const MAX_VERSION: Version = Version::new(5);

/// Required length of both scratch buffers handed to [`encode`].
pub const BUFFER_LEN: usize = 173;

/// Encodes `WIFI:T:WPA;S:{PORTAL_SSID};P:{psk};;` at EC level M with the
/// smallest version that fits (version 4 for the 16-char PSK) and an
/// automatically chosen mask. The PSK alphabet excludes every character
/// the `WIFI:` payload would need escaped (`\ ; , : "`), so `psk` is
/// spliced in verbatim. Returns `None` only when the payload cannot fit
/// [`MAX_VERSION`]; a portal-shaped PSK never triggers that.
pub fn encode<'a>(
    psk: &str,
    temp: &mut [u8; BUFFER_LEN],
    out: &'a mut [u8; BUFFER_LEN],
) -> Option<QrCode<'a>> {
    let mut payload = [0u8; 64];
    let mut len = 0;
    for part in ["WIFI:T:WPA;S:", PORTAL_SSID, ";P:", psk, ";;"] {
        let bytes = part.as_bytes();
        if len + bytes.len() > payload.len() {
            return None;
        }
        payload[len..len + bytes.len()].copy_from_slice(bytes);
        len += bytes.len();
    }
    let text = core::str::from_utf8(&payload[..len]).ok()?;
    QrCode::encode_text(
        text,
        temp,
        out,
        QrCodeEcc::Medium,
        Version::MIN,
        MAX_VERSION,
        None,
        false,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO_PSK: &str = "emudemqpsk234567";

    fn demo_qr(out: &mut [u8; BUFFER_LEN]) -> QrCode<'_> {
        let mut temp = [0u8; BUFFER_LEN];
        encode(DEMO_PSK, &mut temp, out).expect("demo PSK must encode")
    }

    #[test]
    fn sixteen_char_psk_lands_in_version_4() {
        let mut out = [0u8; BUFFER_LEN];
        let qr = demo_qr(&mut out);
        assert_eq!(qr.version().value(), 4);
        assert_eq!(qr.size(), 33);
    }

    #[test]
    fn symbol_carries_the_fixed_structure() {
        let mut out = [0u8; BUFFER_LEN];
        let qr = demo_qr(&mut out);
        let size = qr.size();
        // Finder pattern corners are dark in every QR.
        for &(x, y) in &[(0, 0), (size - 1, 0), (0, size - 1)] {
            assert!(qr.get_module(x, y), "finder corner ({x},{y}) must be dark");
        }
        // The horizontal timing pattern alternates along row 6.
        for x in 8..size - 8 {
            assert_eq!(qr.get_module(x, 6), x % 2 == 0, "timing row at x={x}");
        }
        // The dark module the spec mandates at (8, 4 * version + 9).
        assert!(qr.get_module(8, size - 8));
    }

    #[test]
    fn oversized_psk_is_refused_not_truncated() {
        let long = "23456789ABCDEFGH23456789ABCDEFGH23456789ABCDEFGH";
        let mut temp = [0u8; BUFFER_LEN];
        let mut out = [0u8; BUFFER_LEN];
        assert!(encode(long, &mut temp, &mut out).is_none());
    }
}
