//! Upload naming and identity-sidecar formats shared by the firmware's
//! browser-to-shelf upload path.
//!
//! These are the pure pieces of that path: 8.3 name derivation, label
//! shaping, and the identity-sidecar wire format. They live here rather
//! than in `fw` so host `cargo test` actually exercises them — `fw` only
//! compiles for the riscv32 firmware target and is excluded from the CI
//! test job.

use heapless::String;

/// 8.3 names cap at twelve characters.
pub type UploadName = String<12>;

/// Display-name budget for the upload label sidecar, matched to the catalog
/// label width.
pub type UploadLabel = String<64>;

/// Derives an 8.3 upload name from percent-decoded filename bytes:
/// the first four ASCII alphanumerics uppercased (default BOOK),
/// four base-36 digits hashed from the whole decoded stem, and extension
/// `.EPU` (which the catalog scan accepts alongside `.epub`). A prefix
/// alone is not enough — book filenames often share their first eight
/// characters (author, series), and the write path replaces an
/// identity-matched existing name, so a library of same-prefix uploads
/// collapsed to one file. The hash spreads those apart while staying
/// deterministic: re-uploading the same filename still replaces the same
/// book.
pub fn sanitized_name(client_name: &[u8]) -> UploadName {
    let stem_end = client_name
        .iter()
        .rposition(|byte| *byte == b'.')
        .unwrap_or(client_name.len());
    let stem = &client_name[..stem_end];
    let mut name = UploadName::new();
    let mut hash: u32 = 0x811c_9dc5;
    let mut at = 0;
    while at < stem.len() {
        let byte = stem[at];
        hash = (hash ^ byte as u32).wrapping_mul(0x0100_0193);
        if name.len() < 4 && byte.is_ascii_alphanumeric() {
            let _ = name.push(byte.to_ascii_uppercase() as char);
        }
        at += 1;
    }
    if name.is_empty() {
        let _ = name.push_str("BOOK");
    } else {
        while name.len() < 4 {
            let _ = name.push('X');
        }
    }
    let digits = base36_tail(hash);
    for digit in digits {
        let _ = name.push(digit as char);
    }
    let _ = name.push_str(".EPU");
    name
}

/// Creates a readable label source from pre-decoded client filename bytes,
/// preserving spaces and case (unlike `sanitized_name`, which forces 8.3).
/// The catalog label derivation later strips the extension and prettifies it,
/// so the result is shaped exactly like a copied book's filename label.
/// Falls back to ASCII-only if the bytes aren't valid UTF-8 (e.g. a
/// multibyte character truncated at the buffer edge).
pub fn readable_filename(client_name: &[u8]) -> UploadLabel {
    let mut bytes = [0u8; 64];
    let len = client_name.len().min(bytes.len());
    bytes[..len].copy_from_slice(&client_name[..len]);
    let mut out = UploadLabel::new();
    match core::str::from_utf8(&bytes[..len]) {
        Ok(text) => {
            let _ = out.push_str(text);
        }
        Err(err) => {
            let valid_len = err.valid_up_to();
            if let Ok(valid_text) = core::str::from_utf8(&bytes[..valid_len]) {
                let _ = out.push_str(valid_text);
            }
            for &byte in &bytes[valid_len..len] {
                if byte.is_ascii() && byte >= 0x20 {
                    let _ = out.push(byte as char);
                }
            }
        }
    }
    out
}

/// The list label a catalog record shows, derived from its file name: strip
/// the epub extension and prettify the stem the same way for copied and
/// uploaded books. Non-injective — distinct filenames can map to the same
/// label — so identity must come from the sidecar hash, never a label match.
pub fn derive_catalog_label(display_name: &str, open_name: &str, out: &mut String<64>) {
    if open_name.eq_ignore_ascii_case("HPMOR.EPU") || open_name.eq_ignore_ascii_case("HPMOR.EPUB") {
        let _ = out.push_str("Harry Potter and the Methods of Rationality");
        return;
    }

    let file_name = display_name
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(display_name);
    let stem = strip_epub_suffix(file_name).unwrap_or(file_name);
    push_pretty_file_stem(stem, out);
    if out.is_empty() {
        let _ = out.push_str(display_name);
    }
}

fn strip_epub_suffix(name: &str) -> Option<&str> {
    let bytes = name.as_bytes();
    if bytes.len() >= 5 && bytes[bytes.len() - 5..].eq_ignore_ascii_case(b".epub") {
        return Some(&name[..name.len() - 5]);
    }
    if bytes.len() >= 4 && bytes[bytes.len() - 4..].eq_ignore_ascii_case(b".epu") {
        return Some(&name[..name.len() - 4]);
    }
    None
}

fn push_pretty_file_stem(stem: &str, out: &mut String<64>) {
    let mut capitalize_next = true;
    for byte in stem.bytes() {
        let ch = match byte {
            b'-' | b'_' => {
                capitalize_next = true;
                b' '
            }
            b'a'..=b'z' if capitalize_next => {
                capitalize_next = false;
                byte - b'a' + b'A'
            }
            b'A'..=b'Z' | b'0'..=b'9' => {
                capitalize_next = false;
                byte
            }
            b'.' => break,
            _ => byte,
        };
        if ch == b' ' && out.as_str().ends_with(' ') {
            continue;
        }
        let _ = out.push(ch as char);
    }
    while out.as_str().ends_with(' ') {
        out.pop();
    }
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn base36_tail(hash: u32) -> [u8; 4] {
    const BASE36: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut tail = hash % 36u32.pow(4);
    let mut digits = [0u8; 4];
    for digit in digits.iter_mut().rev() {
        *digit = BASE36[(tail % 36) as usize];
        tail /= 36;
    }
    digits
}

pub fn hash_identity(client_name: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in client_name {
        hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn percent_decode_in_place(bytes: &mut [u8]) -> usize {
    let mut read = 0;
    let mut write = 0;
    while read < bytes.len() {
        if bytes[read] == b'%' && read + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_nibble(bytes[read + 1]), hex_nibble(bytes[read + 2]))
            {
                bytes[write] = (high << 4) | low;
                read += 3;
                write += 1;
                continue;
            }
        }
        bytes[write] = bytes[read];
        read += 1;
        write += 1;
    }
    write
}

/// Percent-decoded `name=` value from a path's query string.
pub fn raw_query_name(path: &mut [u8]) -> Option<&mut [u8]> {
    let query_at = path.iter().position(|byte| *byte == b'?')? + 1;
    let pair = path[query_at..]
        .split_mut(|byte| *byte == b'&')
        .find(|pair| pair.starts_with(b"name="))?;
    let raw_name = &mut pair[5..];
    let len = percent_decode_in_place(raw_name);
    if len == 0 {
        return None;
    }
    Some(&mut raw_name[..len])
}

/// Interprets an identity sidecar read: an 8-byte file whose read returns all
/// 8 bytes is a valid little-endian identity hash.
///
/// The two failure shapes get different verdicts because one is deterministic
/// and one is transient. A sidecar of any other length is malformed for good —
/// retrying can't fix it — so it reads as `Ok(None)` (no identity) and the
/// collision probe moves on; the worst outcome is a visible duplicate book
/// instead of every upload probing that slot failing forever. A short read or
/// I/O error on a correctly-sized file means the card can't be trusted right
/// now, so it surfaces as `Err` and the upload aborts and can be retried.
// The unit error mirrors fw's sidecar helpers (read_upload_identity et al.),
// where the only response to an I/O failure is aborting the upload.
#[allow(clippy::result_unit_err)]
pub fn parse_identity_read<E>(
    file_len: u32,
    read_result: Result<usize, E>,
    buf: &[u8; 8],
) -> Result<Option<u64>, ()> {
    if file_len != 8 {
        return Ok(None);
    }
    match read_result {
        Ok(8) => Ok(Some(u64::from_le_bytes(*buf))),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitized_name() {
        let input1 = b"MyCoolBook.epub";
        let res1 = sanitized_name(input1);
        let res1_again = sanitized_name(input1);
        assert_eq!(res1, res1_again);
        assert!(res1.ends_with(".EPU"));

        let input2 = b"MyCoSecondBook.epub";
        let res2 = sanitized_name(input2);
        assert!(res2.ends_with(".EPU"));

        assert_eq!(&res1.as_str()[0..4], "MYCO");
        assert_eq!(&res2.as_str()[0..4], "MYCO");
        assert_ne!(res1, res2);

        let input_short = b"abc.epub";
        let res_short = sanitized_name(input_short);
        assert_eq!(&res_short.as_str()[0..4], "ABCX");
        assert!(res_short.ends_with(".EPU"));
        assert_eq!(res_short.len(), 12);

        let input_empty = b".epub";
        let res_empty = sanitized_name(input_empty);
        assert_eq!(&res_empty.as_str()[0..4], "BOOK");
        assert!(res_empty.ends_with(".EPU"));
        assert_eq!(res_empty.len(), 12);
    }

    #[test]
    fn test_derive_catalog_label_ambiguity() {
        // This test documents that `derive_catalog_label` is non-injective and
        // maps distinct original filenames to the exact same normalized catalog label.
        // Because of this ambiguity, we cannot safely migrate or overwrite a legacy book
        // based on a normalized label match alone.
        let mut label1 = String::<64>::new();
        derive_catalog_label("MyCoolBook-One.epub", "MYCOOLBO.EPU", &mut label1);

        let mut label2 = String::<64>::new();
        derive_catalog_label("MyCoolBook_One.epub", "MYCOOLBO.EPU", &mut label2);

        assert_eq!(label1.as_str(), "MyCoolBook One");
        assert_eq!(label1, label2);
    }

    #[test]
    fn test_parse_identity_read() {
        let buf = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];

        // An 8-byte file read in full is a valid identity.
        assert_eq!(
            parse_identity_read::<()>(8, Ok(8), &buf),
            Ok(Some(0x8877665544332211))
        );

        // A wrong-length sidecar is deterministically malformed: report "no
        // identity" so the probe skips the slot instead of aborting every
        // future upload whose probe window crosses it. The read result is
        // irrelevant — the file can't hold a valid hash.
        assert_eq!(parse_identity_read::<()>(0, Ok(0), &buf), Ok(None));
        assert_eq!(parse_identity_read::<()>(7, Ok(7), &buf), Ok(None));
        assert_eq!(parse_identity_read::<()>(9, Ok(8), &buf), Ok(None));
        assert_eq!(parse_identity_read::<()>(7, Err(()), &buf), Ok(None));

        // A short read or I/O error on a correctly-sized file is transient
        // card trouble: abort the upload so a retry can succeed.
        assert_eq!(parse_identity_read::<()>(8, Ok(7), &buf), Err(()));
        assert_eq!(parse_identity_read::<()>(8, Ok(0), &buf), Err(()));
        assert_eq!(parse_identity_read::<()>(8, Err(()), &buf), Err(()));
    }

    #[test]
    fn test_hash_identity() {
        let input1 = b"MyCoolBook.epub";
        let res1 = hash_identity(input1);
        let res1_again = hash_identity(input1);
        assert_eq!(res1, res1_again);

        let input2 = b"MyCoolBook_v2.epub";
        let res2 = hash_identity(input2);
        assert_ne!(res1, res2);

        // Files that collide in legacy 8.3 naming must have distinct identity hashes
        let collide1 = b"MyCoolBook One.epub";
        let collide2 = b"MyCoolBook Two.epub";
        assert_ne!(hash_identity(collide1), hash_identity(collide2));
    }

    #[test]
    fn test_percent_decode() {
        let mut buf1 = b"?name=My%20Cool%20Book.epub&other=1".to_vec();
        let name1 = raw_query_name(&mut buf1).unwrap();
        assert_eq!(name1, b"My Cool Book.epub");

        let mut buf2 = b"?name=A%26B%3D%2B%3F.epub".to_vec();
        let name2 = raw_query_name(&mut buf2).unwrap();
        assert_eq!(name2, b"A&B=+?.epub");

        let mut buf3 = b"other=1".to_vec();
        assert!(raw_query_name(&mut buf3).is_none());

        let mut buf4 = b"?name=&other=1".to_vec();
        assert!(raw_query_name(&mut buf4).is_none());
    }
}
