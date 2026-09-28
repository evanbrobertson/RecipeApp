//! Photos kept in a recipe itself: `data:image/...;base64,` URIs from older photo imports,
//! backups and other Crumbs' shares. Pure, so recipe validation can accept them wherever it
//! runs; the server's resizer (`src/images.rs`) decodes them with the same functions.

use base64::Engine;

/// The biggest photo the resizer takes, and so the biggest one kept in a recipe.
pub const MAX_SOURCE_BYTES: usize = 15 * 1024 * 1024;

/// Why a photo was refused for its size.
pub const TOO_LARGE: &str = "image too large";

/// Bytes of a `data:image/...;base64,` URI.
pub fn decode_data_uri(uri: &str) -> Result<Vec<u8>, String> {
    let rest = uri.strip_prefix("data:").ok_or("not a data URI")?;
    let (meta, data) = rest.split_once(',').ok_or("malformed data URI")?;
    let meta = meta.to_ascii_lowercase();
    if !meta.starts_with("image/") || !meta.split(';').any(|p| p == "base64") {
        return Err("data URI isn't a base64 image".into());
    }
    if data.len() > MAX_SOURCE_BYTES / 3 * 4 + 4096 {
        return Err(TOO_LARGE.into());
    }
    let data: String = data.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let data = percent_encoding::percent_decode_str(&data).decode_utf8_lossy();
    let engine = base64::engine::general_purpose::STANDARD;
    engine
        .decode(data.as_bytes())
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(data.as_bytes()))
        .map_err(|e| format!("bad base64 in data URI: {e}"))
}

/// The type of a photo that may be kept in a recipe itself, by its first bytes: JPEG, PNG
/// or WebP, whatever it's labelled.
pub fn photo_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Whether `image` is a photo kept in the recipe itself (older photo imports, or one saved
/// from another Crumb's share): a base64 `data:image/(jpeg|png|webp)` URI whose bytes are
/// that, no bigger than the resizer takes. Any other `data:` URI isn't one.
pub fn is_embedded_photo(image: &str) -> bool {
    let Some(meta) = image
        .get(..image.find(',').unwrap_or(0))
        .and_then(|m| m.strip_prefix("data:"))
    else {
        return false;
    };
    let named = meta.split(';').next().unwrap_or("").to_ascii_lowercase();
    matches!(named.as_str(), "image/jpeg" | "image/png" | "image/webp")
        && decode_data_uri(image).is_ok_and(|b| photo_type(&b).is_some())
}

/// A photo's bytes as a `data:` URI to keep in the recipe, when they're a JPEG, PNG or WebP
/// no bigger than the resizer takes.
pub fn embed(bytes: &[u8]) -> Option<String> {
    let kind = photo_type(bytes).filter(|_| bytes.len() <= MAX_SOURCE_BYTES)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{kind};base64,{b64}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Just the signatures: enough for `photo_type`, which only reads the first bytes.
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10];
    const WEBP: &[u8] = b"RIFF\x24\0\0\0WEBPVP8 ";

    fn uri(label: &str, bytes: &[u8]) -> String {
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        format!("data:{label};base64,{b64}")
    }

    #[test]
    fn photo_types_come_from_the_bytes() {
        assert_eq!(photo_type(PNG), Some("image/png"));
        assert_eq!(photo_type(JPEG), Some("image/jpeg"));
        assert_eq!(photo_type(WEBP), Some("image/webp"));
        assert_eq!(photo_type(b"GIF89a"), None);
        assert_eq!(photo_type(b"<svg/>"), None);
    }

    #[test]
    fn reads_base64_data_uris() {
        assert_eq!(decode_data_uri(&uri("image/png", PNG)).unwrap(), PNG);
        // Whitespace and percent-encoding, as some exports write them
        let b64 = base64::engine::general_purpose::STANDARD.encode(PNG);
        let spaced = format!("data:image/png;base64,{}\n{}", &b64[..8], &b64[8..]);
        assert_eq!(decode_data_uri(&spaced).unwrap(), PNG);
        assert!(decode_data_uri("data:text/plain;base64,aGk=").is_err());
        assert!(decode_data_uri("data:image/svg+xml,<svg/>").is_err());
        assert!(decode_data_uri("https://example.com/a.png").is_err());
    }

    #[test]
    fn only_jpeg_png_and_webp_photos_are_kept_in_a_recipe() {
        assert!(is_embedded_photo(&uri("image/png", PNG)));
        assert!(is_embedded_photo(&uri("image/webp", WEBP)));
        // The label may be off, as long as it names one of the three and the bytes are one
        assert!(is_embedded_photo(&uri("image/jpeg", PNG)));
        assert!(!is_embedded_photo(&uri("image/gif", PNG)));
        assert!(!is_embedded_photo(&uri("text/html", PNG)));
        assert!(!is_embedded_photo(&uri("image/png", b"<svg/>")));
        assert!(!is_embedded_photo("data:image/png;base64,AA"));
        assert!(!is_embedded_photo("https://example.com/a.png"));
        assert!(!is_embedded_photo("javascript:alert(1)"));
    }

    #[test]
    fn embeds_only_photos() {
        assert_eq!(embed(JPEG).unwrap(), uri("image/jpeg", JPEG));
        assert_eq!(embed(b"<svg/>"), None);
    }
}
