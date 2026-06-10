//! Media metadata extraction: MIME detection, dimension reading (images
//! only), content hashing, and path-template rendering. Covers images,
//! audio, and video.

pub mod detect;
pub mod hash;
pub mod template;

pub use detect::{detect_mime, read_dimensions};
pub use hash::{content_hash, content_hash_hex, short_hash};
pub use template::{render_template, TemplateContext};
