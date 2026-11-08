//! The glyph a simple TrueType font's code names (ISO 32000-2, 9.6.5.4).
//!
//! A show-text code reaches exactly one glyph through the font's own tables;
//! nothing here shapes, substitutes or reorders.

use crate::fonts::font_dict::{glyph_name_to_unicode, standard_encoding_lookup};
use crate::fonts::{Encoding, FontInfo};
use ttf_parser::{Face, PlatformId};

const NONSYMBOLIC: i32 = 0x20;

/// The glyph id `code` names in a simple TrueType font, 0 when no table gives one.
///
/// Non-symbolic (a named WinAnsi or MacRoman encoding, or the Nonsymbolic
/// flag): glyph name to Unicode to the (3,1) subtable, then the (1,0)
/// subtable by the name's Mac OS Roman code, then the `post` table by name.
/// Symbolic: the (3,0) subtable by the code and then by 0xF000 + code, then
/// the (1,0) subtable by the code, then the (3,1) subtable by the code as a
/// Unicode value (producers flag fonts symbolic wrongly, and other readers
/// fall back the same way).
pub(crate) fn simple_truetype_gid(font: &FontInfo, face: &Face, code: u8) -> u16 {
    let Some(cmap) = face.tables().cmap else {
        return post_name_gid(face, font.diff_glyph_names.get(&code).map(String::as_str), None);
    };
    let subtable = |platform: PlatformId, encoding: u16| {
        cmap.subtables
            .into_iter()
            .find(|s| s.platform_id == platform && s.encoding_id == encoding)
    };
    let lookup = |platform: PlatformId, encoding: u16, key: u32| {
        subtable(platform, encoding)
            .and_then(|s| s.glyph_index(key))
            .map(|g| g.0)
            .filter(|&g| g != 0)
    };

    if !is_nonsymbolic(font) {
        return lookup(PlatformId::Windows, 0, u32::from(code))
            .or_else(|| lookup(PlatformId::Windows, 0, 0xF000 + u32::from(code)))
            .or_else(|| lookup(PlatformId::Macintosh, 0, u32::from(code)))
            .or_else(|| lookup(PlatformId::Windows, 1, u32::from(code)))
            .unwrap_or(0);
    }

    let name = font.diff_glyph_names.get(&code).map(String::as_str);
    let unicode = unicode_of_code(font, code, name);
    if let Some(ch) = unicode {
        if let Some(gid) = lookup(PlatformId::Windows, 1, ch as u32) {
            return gid;
        }
        if let Some(mac) = mac_roman_code(ch) {
            if let Some(gid) = lookup(PlatformId::Macintosh, 0, u32::from(mac)) {
                return gid;
            }
        }
    }
    post_name_gid(face, name, unicode)
}

fn is_nonsymbolic(font: &FontInfo) -> bool {
    matches!(
        &font.encoding,
        Encoding::Standard(n) if n == "WinAnsiEncoding" || n == "MacRomanEncoding"
    ) || font.flags.is_some_and(|f| f & NONSYMBOLIC != 0)
}

/// The Unicode character the encoding gives `code`: its /Differences glyph
/// name first, then the base encoding.
fn unicode_of_code(font: &FontInfo, code: u8, diff_name: Option<&str>) -> Option<char> {
    if let Some(name) = diff_name {
        return glyph_name_to_unicode(name);
    }
    match &font.encoding {
        Encoding::Standard(n) => standard_encoding_lookup(n, code)?.chars().next(),
        Encoding::Custom(map) => map.get(&code).copied(),
        Encoding::Identity => None,
    }
}

fn mac_roman_code(ch: char) -> Option<u8> {
    (0..=255u8).find(|&c| {
        standard_encoding_lookup("MacRomanEncoding", c).and_then(|s| s.chars().next()) == Some(ch)
    })
}

/// The glyph the `post` table names `name`. Without a /Differences name the
/// name is the one `post` gives the encoding's character, found by the
/// glyph-name-to-Unicode mapping.
fn post_name_gid(face: &Face, name: Option<&str>, unicode: Option<char>) -> u16 {
    if let Some(name) = name {
        return face.glyph_index_by_name(name).map_or(0, |g| g.0);
    }
    let Some(ch) = unicode else { return 0 };
    (0..face.number_of_glyphs())
        .filter_map(|g| {
            let id = ttf_parser::GlyphId(g);
            face.glyph_name(id)
                .filter(|n| glyph_name_to_unicode(n) == Some(ch))
                .map(|_| id.0)
        })
        .next()
        .unwrap_or(0)
}
