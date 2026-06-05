//! MIME type and dimension detection for image files.

use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DetectError {
    #[error("could not detect MIME type from content")]
    UnknownMime,
    #[error("could not read image dimensions: {0}")]
    Dimension(String),
}

/// Detect MIME type by inspecting file content first, then falling back to
/// the file extension. Returns `"application/octet-stream"` if nothing
/// matched.
pub fn detect_mime(bytes: &[u8], path: Option<&Path>) -> String {
    if let Some(kind) = infer::get(bytes) {
        return kind.mime_type().to_string();
    }
    if let Some(p) = path {
        if let Some(guess) = mime_guess_fallback(p) {
            return guess;
        }
    }
    "application/octet-stream".to_string()
}

fn mime_guess_fallback(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "tiff" | "tif" => "image/tiff",
        "heic" => "image/heic",
        "avif" => "image/avif",
        _ => return None,
    };
    Some(mime.to_string())
}

/// Read image dimensions for the common raster formats supported by the
/// `image` crate. Returns `Ok(None)` if dimensions cannot be determined
/// (e.g. SVG, unknown).
pub fn read_dimensions(bytes: &[u8]) -> Result<Option<(u32, u32)>, DetectError> {
    use std::io::Cursor;
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| DetectError::Dimension(e.to_string()))?;
    match reader.into_dimensions() {
        Ok((w, h)) => Ok(Some((w, h))),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_png_by_magic() {
        // Smallest valid PNG (1x1)
        let png = [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        let mime = detect_mime(&png, Some(Path::new("cover.png")));
        assert_eq!(mime, "image/png");
    }

    #[test]
    fn falls_back_to_extension_when_unknown_magic() {
        // SVG without XML preamble; infer returns None.
        let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>";
        let mime = detect_mime(svg, Some(Path::new("foo.svg")));
        assert_eq!(mime, "image/svg+xml");
    }

    #[test]
    fn octet_stream_for_unknown() {
        let bytes = b"random text content";
        let mime = detect_mime(bytes, Some(Path::new("file.unknown")));
        assert_eq!(mime, "application/octet-stream");
    }
}
