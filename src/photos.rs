//! Recipe import from photos (`POST /api/recipes/import/photos`).
//!
//! Wee Chef reads the photos with the vision model in one structured-output call (see
//! `llm::extract_recipe_from_photos`). Before that, each photo is checked by its magic
//! bytes, turned upright from its EXIF orientation, scaled to at most [`LONG_EDGE`] px and
//! re-encoded as JPEG, which also drops EXIF (GPS included). The photos are never stored.
//!
//! Without an AI key the browser runs OCR itself and sends the text to
//! `/api/recipes/import` instead, so this route answers 400.

use std::io::Cursor;
use std::time::Instant;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use tokio::sync::Semaphore;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::model::Recipe;

/// Photos in one import: a recipe over a few pages.
pub const MAX_PHOTOS: usize = 6;
/// The whole upload. Phones send 3 to 8 MB photos when the browser can't shrink them.
pub const MAX_BODY_BYTES: usize = 40 * 1024 * 1024;
/// Longest side sent to the model; more doesn't read any better and costs more tokens.
pub const LONG_EDGE: u32 = 1568;
const JPEG_QUALITY: u8 = 85;
/// An upright JPEG this small and no bigger than [`LONG_EDGE`] is sent as it is (minus EXIF).
const PASS_THROUGH_BYTES: usize = 1_500_000;
/// Decoded size guards against decompression bombs.
/// 40 MP covers every phone camera's default mode (48 MP sensors save 12 MP by default).
const MAX_PIXELS: u64 = 40_000_000;
const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;
/// A 40 MP photo takes about 120 MB decoded, so only this many are decoded at once.
static DECODES: Semaphore = Semaphore::const_new(2);

/// One page, ready for the model.
#[derive(Debug, Clone)]
pub struct Photo {
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}

/// The formats the image crate here decodes (and every vision API takes).
fn sniff(bytes: &[u8]) -> Result<ImageFormat, AppError> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok(ImageFormat::Jpeg);
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(ImageFormat::Png);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok(ImageFormat::Gif);
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Ok(ImageFormat::WebP);
    }
    // ISO media files: HEIC/HEIF (iPhone photos) and AVIF
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if [
            b"heic", b"heix", b"hevc", b"hevx", b"heim", b"heis", b"mif1", b"msf1", b"avif",
            b"avis",
        ]
        .iter()
        .any(|b| brand == *b)
        {
            return Err(AppError::new(
                415,
                "Crumb can't read HEIC or AVIF photos yet. Save it as a JPEG (or take a screenshot of it) and try again.",
            ));
        }
    }
    Err(AppError::new(
        415,
        "That doesn't look like a photo Crumb can read. Use a JPEG, PNG, WebP or GIF.",
    ))
}

/// Checks, turns upright, scales and re-encodes one photo. Blocking: run it off the
/// async threads.
pub fn prepare(bytes: &[u8]) -> AppResult<Photo> {
    let format = sniff(bytes)?;
    let unreadable =
        |e: image::ImageError| AppError::new(422, format!("Couldn't read that photo ({e})"));
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(unreadable)?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err(AppError::new(422, "That photo is empty"));
    }
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(AppError::new(
            413,
            format!("That photo is too big ({w}×{h}). Up to 40 megapixels."),
        ));
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    if format == ImageFormat::Jpeg
        && orientation == Orientation::NoTransforms
        && bytes.len() <= PASS_THROUGH_BYTES
        && w.max(h) <= LONG_EDGE
        && let Some(clean) = strip_jpeg_metadata(bytes)
    {
        return Ok(Photo {
            media_type: "image/jpeg",
            bytes: clean,
        });
    }

    let mut img = DynamicImage::from_decoder(decoder).map_err(unreadable)?;
    // Shrink before turning upright, so the rotation copies the small image, not the full
    // decode. The bounds are a square, so the result is the same either way.
    if img.width().max(img.height()) > LONG_EDGE {
        img = img.resize(LONG_EDGE, LONG_EDGE, FilterType::Triangle);
    }
    img.apply_orientation(orientation);
    let rgb = if img.color().has_alpha() {
        // Transparent areas become white paper, not black
        let rgba = img.to_rgba8();
        let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
        for (out, px) in rgb.pixels_mut().zip(rgba.pixels()) {
            let a = u16::from(px[3]);
            for c in 0..3 {
                out[c] = ((u16::from(px[c]) * a + 255 * (255 - a)) / 255) as u8;
            }
        }
        rgb
    } else {
        img.to_rgb8()
    };
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY)
        .encode_image(&rgb)
        .map_err(AppError::internal)?;
    Ok(Photo {
        media_type: "image/jpeg",
        bytes: out,
    })
}

/// A copy of a JPEG without its EXIF/XMP (APP1) and IPTC (APP13) segments, or None if the
/// markers don't parse (then it's re-encoded instead).
fn strip_jpeg_metadata(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[..2]);
    let mut i = 2;
    loop {
        if bytes.get(i) != Some(&0xFF) {
            return None;
        }
        let marker = *bytes.get(i + 1)?;
        // Fill bytes between segments
        if marker == 0xFF {
            i += 1;
            continue;
        }
        // Start of scan: the rest is image data
        if marker == 0xDA {
            out.extend_from_slice(&bytes[i..]);
            return Some(out);
        }
        let len = usize::from(u16::from_be_bytes([*bytes.get(i + 2)?, *bytes.get(i + 3)?]));
        if len < 2 {
            return None;
        }
        let end = i + 2 + len;
        let segment = bytes.get(i..end)?;
        if marker != 0xE1 && marker != 0xED {
            out.extend_from_slice(segment);
        }
        i = end;
    }
}

/// [`prepare`] for each photo in order, on the blocking pool, at most two at once
/// across all requests.
pub async fn prepare_all(uploads: Vec<Vec<u8>>) -> AppResult<Vec<Photo>> {
    let count = uploads.len();
    let mut photos = Vec::with_capacity(count);
    for (i, bytes) in uploads.into_iter().enumerate() {
        let _permit = DECODES.acquire().await.map_err(AppError::internal)?;
        let photo = tokio::task::spawn_blocking(move || prepare(&bytes))
            .await
            .map_err(AppError::internal)?;
        photos.push(photo.map_err(|mut err| {
            if count > 1 {
                err.message = format!("Photo {}: {}", i + 1, err.message);
            }
            err
        })?);
    }
    Ok(photos)
}

/// Reads a recipe from photos of its pages and saves it (source "photo"). `hint` is an
/// optional note from the cook, e.g. "the pancakes on the left".
pub async fn import(
    state: &AppState,
    uploads: Vec<Vec<u8>>,
    hint: Option<&str>,
) -> AppResult<(Recipe, bool)> {
    if !crate::llm::available(state) {
        return Err(AppError::bad_request(
            "Wee Chef isn't set up here, so photos are read on your device instead.",
        ));
    }
    if uploads.is_empty() {
        return Err(AppError::bad_request("Add a photo of the recipe"));
    }
    if uploads.len() > MAX_PHOTOS {
        return Err(AppError::bad_request(format!(
            "Up to {MAX_PHOTOS} photos at a time"
        )));
    }
    let started = Instant::now();
    let photos = prepare_all(uploads).await?;
    let sent: usize = photos.iter().map(|p| p.bytes.len()).sum();
    let fields = crate::llm::extract_recipe_from_photos(state, &photos, hint).await;
    tracing::info!(
        "[photos] {} photo(s), {} KB sent, {} ms",
        photos.len(),
        sent / 1024,
        started.elapsed().as_millis()
    );
    drop(photos);
    let fields = fields.ok_or_else(|| {
        AppError::new(
            422,
            "Wee Chef couldn't find a recipe in that. Try a closer, sharper photo in good light, or paste the text instead.",
        )
    })?;
    if fields.ingredients.is_empty() && fields.instructions.is_empty() {
        return Err(AppError::new(
            422,
            "Wee Chef couldn't find ingredients or steps in that. Try a closer, sharper photo.",
        ));
    }
    // The same Wee Chef checks as other outside imports: tidy before the save, the
    // background check after it (both in create_checked)
    crate::recipes::create_checked(state, fields, "photo")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([x as u8, y as u8, 90]));
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 80)
            .encode_image(&img)
            .unwrap();
        out
    }

    /// `jpeg` with an EXIF APP1 segment right after SOI: orientation `o` and a fake GPS tag.
    pub fn jpeg_with_orientation(w: u32, h: u32, o: u16) -> Vec<u8> {
        let plain = jpeg(w, h);
        // Little-endian TIFF: IFD0 with Orientation (0x0112) and GPSInfo (0x8825) entries
        let mut tiff = b"II*\0\x08\0\0\0".to_vec();
        tiff.extend_from_slice(&2u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&u32::from(o).to_le_bytes());
        tiff.extend_from_slice(&0x8825u16.to_le_bytes());
        tiff.extend_from_slice(&4u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes());
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(&tiff);
        let mut out = plain[..2].to_vec();
        out.extend_from_slice(&[0xFF, 0xE1]);
        out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&app1);
        out.extend_from_slice(&plain[2..]);
        out
    }

    fn size(bytes: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory(bytes).unwrap();
        (img.width(), img.height())
    }

    #[test]
    fn rotated_photos_come_out_upright_and_scaled() {
        // Orientation 6: stored landscape, shown portrait
        let photo = prepare(&jpeg_with_orientation(2400, 1800, 6)).unwrap();
        assert_eq!(photo.media_type, "image/jpeg");
        let (w, h) = size(&photo.bytes);
        assert!(h > w, "{w}x{h}");
        assert_eq!(h, LONG_EDGE);
        assert_eq!(w, 1176);
        assert!(!photo.bytes.windows(4).any(|w| w == b"Exif"));
    }

    #[test]
    fn small_upright_jpegs_pass_through_without_exif() {
        let original = jpeg_with_orientation(800, 600, 1);
        let photo = prepare(&original).unwrap();
        assert_eq!(size(&photo.bytes), (800, 600));
        assert!(original.windows(4).any(|w| w == b"Exif"));
        assert!(!photo.bytes.windows(4).any(|w| w == b"Exif"));
        // Everything else is byte for byte the same
        assert_eq!(photo.bytes.len(), jpeg(800, 600).len());
        // A small rotated one is re-encoded upright
        let rotated = prepare(&jpeg_with_orientation(800, 600, 6)).unwrap();
        assert_eq!(size(&rotated.bytes), (600, 800));
    }

    #[test]
    fn other_formats_become_jpeg_on_white() {
        let img = image::RgbaImage::from_pixel(40, 20, image::Rgba([0, 0, 0, 0]));
        let mut png = Cursor::new(Vec::new());
        img.write_to(&mut png, ImageFormat::Png).unwrap();
        let photo = prepare(png.get_ref()).unwrap();
        let out = image::load_from_memory(&photo.bytes).unwrap().to_rgb8();
        assert!(out.get_pixel(5, 5)[0] > 240);
    }

    #[test]
    fn heic_and_unknown_files_are_415() {
        let heic = b"\0\0\0\x18ftypheic\0\0\0\0mif1heic";
        let err = prepare(heic).unwrap_err();
        assert_eq!(err.status.as_u16(), 415);
        assert!(err.message.contains("HEIC"));
        let avif = b"\0\0\0\x1cftypavif\0\0\0\0avifmif1miaf";
        assert_eq!(prepare(avif).unwrap_err().status.as_u16(), 415);
        assert_eq!(prepare(b"%PDF-1.7").unwrap_err().status.as_u16(), 415);
    }

    #[test]
    fn huge_photos_are_refused_before_decoding() {
        // A real JPEG header claiming 10000×10000 (the SOF has no checksum)
        let mut bytes = jpeg(16, 16);
        let sof = bytes.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
        bytes[sof + 5..sof + 7].copy_from_slice(&10_000u16.to_be_bytes());
        bytes[sof + 7..sof + 9].copy_from_slice(&10_000u16.to_be_bytes());
        let err = prepare(&bytes).unwrap_err();
        assert_eq!(err.status.as_u16(), 413);
    }
}
