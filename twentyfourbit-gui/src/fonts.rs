//! Finding a font that can draw Chinese.
//!
//! egui ships only Latin faces (Ubuntu-Light / NotoEmoji / Hack), so without
//! this every song title on a site that is entirely in Chinese renders as tofu.
//!
//! Rather than hard-code paths, the system font database is queried for a face
//! whose family is a known CJK family. Files inside `.ttc` collections carry a
//! face index, which egui's `FontData::index` understands — that is why this
//! looks up faces, not just files.

use std::sync::Arc;

/// A CJK-capable font found on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CjkFont {
    /// Path or in-memory source of the font file.
    pub source: FontSource,
    /// Face index inside a collection file.
    pub index: u32,
    /// Family name, for logging and for the "which font am I using" display.
    pub family: String,
}

/// Where the font bytes live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontSource {
    /// A file on disk.
    File(std::path::PathBuf),
}

/// Family names that are known to carry Simplified Chinese glyphs, best first.
///
/// Ordering matters: PingFang is the macOS UI face, Microsoft YaHei the Windows
/// one, and Noto Sans CJK the usual Linux package. Falling back to any of the
/// others still beats tofu.
const PREFERRED_FAMILIES: &[&str] = &[
    "pingfang sc",
    "pingfang",
    "hiragino sans gb",
    "microsoft yahei",
    "microsoft yahei ui",
    "noto sans cjk sc",
    "noto sans sc",
    "source han sans sc",
    "source han sans cn",
    "wenquanyi micro hei",
    "droid sans fallback",
    "heiti sc",
    "stheiti",
    "songti sc",
    "simhei",
    "simsun",
    "arial unicode ms",
];

/// Find a CJK font on this machine.
///
/// Returns `None` when no Chinese-capable face is installed, which the caller
/// should turn into a visible banner rather than silence.
pub fn find_cjk_font() -> Option<CjkFont> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    find_in(&db)
}

/// The lookup half of [`find_cjk_font`], split out so it can be tested against
/// a database built from a specific file.
pub fn find_in(db: &fontdb::Database) -> Option<CjkFont> {
    let faces: Vec<fontdb::FaceInfo> = db.faces().cloned().collect();

    for wanted in PREFERRED_FAMILIES {
        let found = faces.iter().find(|face| {
            face.families
                .iter()
                .any(|(name, _)| name.to_lowercase() == *wanted)
        });
        if let Some(face) = found {
            let family = face
                .families
                .first()
                .map(|(n, _)| n.clone())
                .unwrap_or_else(|| (*wanted).to_owned());
            if let Some(font) = to_font(face, family) {
                return Some(font);
            }
        }
    }

    // Last resort: any face whose family mentions CJK-ish names.
    for face in &faces {
        let name = face
            .families
            .first()
            .map(|(n, _)| n.to_lowercase())
            .unwrap_or_default();
        if ["cjk", "han", "hei", "song", "ming", "kai"]
            .iter()
            .any(|k| name.contains(k))
        {
            let family = face.families[0].0.clone();
            if let Some(font) = to_font(face, family) {
                return Some(font);
            }
        }
    }

    None
}

fn to_font(face: &fontdb::FaceInfo, family: String) -> Option<CjkFont> {
    match &face.source {
        fontdb::Source::File(path) => Some(CjkFont {
            source: FontSource::File(path.clone()),
            index: face.index,
            family,
        }),
        // Binary sources belong to the caller's own database; not reachable for
        // system fonts, and we cannot persist them.
        fontdb::Source::Binary(_) | fontdb::Source::SharedFile(_, _) => None,
    }
}

/// Load the bytes for a discovered font.
pub fn load_bytes(font: &CjkFont) -> Option<Arc<Vec<u8>>> {
    match &font.source {
        FontSource::File(path) => std::fs::read(path).ok().map(Arc::new),
    }
}
