//! PDF font handling — font resolution, glyph widths, and Unicode mapping.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Adobe Glyph List data, embedded at compile time.
static AGL_DATA: &str = include_str!("data/glyphlist.txt");

/// Lazily-initialized AGL lookup: glyph name → Unicode string.
static AGL_MAP: LazyLock<HashMap<&'static str, String>> = LazyLock::new(|| {
    let mut map = HashMap::with_capacity(4300);
    for line in AGL_DATA.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, hex_part)) = line.split_once(';') {
            // hex_part may be a single value "0041" or multi-codepoint "05D3 05B2"
            let mut s = String::new();
            for hex_str in hex_part.split_whitespace() {
                if let Ok(cp) = u32::from_str_radix(hex_str, 16) {
                    if let Some(c) = char::from_u32(cp) {
                        s.push(c);
                    }
                }
            }
            if !s.is_empty() {
                map.insert(name, s);
            }
        }
    }
    map
});

/// Extra glyph names used by TeX Computer Modern fonts (CMSY, CMEX, CMMI, etc.)
/// that are not in the Adobe Glyph List.
static TEX_GLYPH_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // CMSY (Computer Modern Symbol) extras
    m.insert("asteriskmath", "\u{2217}"); // ∗
    m.insert("diamondmath", "\u{22C4}"); // ⋄
    m.insert("minusplus", "\u{2213}"); // ∓
    m.insert("circleminus", "\u{2296}"); // ⊖
    m.insert("circledivide", "\u{2298}"); // ⊘
    m.insert("circledot", "\u{2299}"); // ⊙
    m.insert("circlecopyrt", "\u{00A9}"); // ©
    m.insert("equivasymptotic", "\u{224D}"); // ≍
    m.insert("precedesequal", "\u{227C}"); // ≼
    m.insert("followsequal", "\u{227D}"); // ≽
    m.insert("similarequal", "\u{2243}"); // ≃
    m.insert("lessmuch", "\u{226A}"); // ≪
    m.insert("greatermuch", "\u{226B}"); // ≫
    m.insert("follows", "\u{227B}"); // ≻
    m.insert("arrownortheast", "\u{2197}"); // ↗
    m.insert("arrowsoutheast", "\u{2198}"); // ↘
    m.insert("arrownorthwest", "\u{2196}"); // ↖
    m.insert("arrowsouthwest", "\u{2199}"); // ↙
    m.insert("negationslash", "\u{0338}"); // combining long solidus
    m.insert("owner", "\u{220B}"); // ∋
    m.insert("triangleinv", "\u{25BD}"); // ▽
    m.insert("latticetop", "\u{22A4}"); // ⊤
                                        // CMMI (Computer Modern Math Italic) extras
    m.insert("tie", "\u{2040}"); // ⁀  (character tie)
    m.insert("dotlessj", "\u{0237}"); // ȷ
    m.insert("vector", "\u{20D7}"); // combining right arrow above
                                    // CMEX (Computer Modern Extension) and CMR extras
    m.insert("bardbl", "\u{2016}"); // ‖
    m.insert("mapsto", "\u{21A6}"); // ↦
    m.insert("lscript", "\u{2113}"); // ℓ
    m.insert("weierstrass", "\u{2118}"); // ℘
    m.insert("visiblespace", "\u{2423}"); // ␣
    m
});

/// Represents a resolved PDF font with metrics and encoding info.
///
/// # Invariants
///
/// `widths`, `default_width`, `ascent`, `descent`, and `font_bbox` are always
/// stored in **per-mille text-space units** (1000 = 1 em). Callers convert to
/// user space with `value / 1000.0 * font_size`. Type 3 fonts, whose native
/// `/Widths` live in glyph space scaled by `/FontMatrix`, are normalized at
/// resolve time by [`crate::pdf::font_type3`] so this invariant holds for every
/// subtype.
#[derive(Debug, Clone)]
pub struct PdfFont {
    /// Resource name (e.g., "F1")
    pub name: String,
    /// Base font name (e.g., "Helvetica", "ArialMT")
    pub base_font: String,
    /// Font subtype (Type1, TrueType, Type0, Type3)
    pub subtype: String,
    /// Glyph widths indexed by character code (per-mille text space)
    pub widths: HashMap<u32, f64>,
    /// Default width for missing glyphs (per-mille text space)
    pub default_width: f64,
    /// ToUnicode mapping: character code → Unicode string
    pub to_unicode: HashMap<u32, String>,
    /// Encoding name
    pub encoding: String,
    /// Whether this is a standard 14 font
    pub is_standard: bool,
    /// Font descriptor flags
    pub flags: u32,
    /// Italic angle
    pub italic_angle: f64,
    /// Font weight (estimated)
    pub weight: f64,
    /// Bytes per character code (1 for Type1/TrueType, 2 for Type0/CID)
    pub bytes_per_code: u8,
    /// Font ascent in per-mille text-space units (1000 = 1 em)
    pub ascent: f64,
    /// Font descent in per-mille text-space units (negative)
    pub descent: f64,
    /// Font bounding box [llx, lly, urx, ury] in per-mille text-space units
    pub font_bbox: [f64; 4],
}

impl PdfFont {
    /// Create a default font with standard metrics.
    pub fn default_font(name: &str) -> Self {
        Self {
            name: name.to_string(),
            base_font: "Unknown".to_string(),
            subtype: "Type1".to_string(),
            widths: HashMap::new(),
            default_width: 600.0, // Monospace default
            to_unicode: HashMap::new(),
            encoding: "WinAnsiEncoding".to_string(),
            is_standard: false,
            flags: 0,
            italic_angle: 0.0,
            weight: 400.0,
            bytes_per_code: 1,
            ascent: 800.0,
            descent: -200.0,
            font_bbox: [0.0, -200.0, 1000.0, 800.0],
        }
    }

    /// Get glyph width for a character code (in glyph space, typically 1/1000 of text space).
    pub fn glyph_width(&self, char_code: u32) -> f64 {
        *self.widths.get(&char_code).unwrap_or(&self.default_width)
    }

    /// Get Unicode string for a character code.
    ///
    /// Fallback order (ISO 32000 text extraction practice):
    /// 1. `/ToUnicode` CMap (and Differences / embedded cmap filled at resolve time)
    /// 2. Declared single-byte encoding (WinAnsi / MacRoman)
    /// 3. For Type0 / Identity-H: ASCII-range CID → Latin-1; else replacement `.`
    pub fn decode_char(&self, char_code: u32) -> String {
        if let Some(unicode) = self.to_unicode.get(&char_code) {
            return unicode.clone();
        }
        if self.bytes_per_code == 2 {
            // Identity-H without mapping: ASCII printable CIDs are common for
            // simple embedded fonts; avoid WinAnsi on high CIDs (mojibake).
            if (0x20..=0x7E).contains(&char_code) {
                return char::from_u32(char_code).unwrap_or('.').to_string();
            }
            // Private-use / unmapped — leave a visible placeholder so layout
            // still advances; callers may OCR glyphs when enabled.
            return "\u{FFFD}".to_string();
        }
        match self.encoding.as_str() {
            "MacRomanEncoding" => decode_macroman(char_code),
            _ => decode_winansi(char_code),
        }
    }

    /// Whether the font is bold (estimated from name or flags).
    pub fn is_bold(&self) -> bool {
        let name_lower = self.base_font.to_lowercase();
        name_lower.contains("bold")
            || name_lower.contains("black")
            || name_lower.contains("heavy")
            || (self.flags & 0x40000) != 0
            || self.weight >= 700.0
    }

    /// Whether the font is italic (estimated from name or italic angle).
    pub fn is_italic(&self) -> bool {
        let name_lower = self.base_font.to_lowercase();
        name_lower.contains("italic")
            || name_lower.contains("oblique")
            || (self.flags & 0x40) != 0
            || self.italic_angle.abs() > 0.5
    }
}

/// Font cache for resolved fonts per page.
#[derive(Debug, Default, Clone)]
pub struct FontCache {
    fonts: HashMap<String, PdfFont>,
}

impl FontCache {
    /// Get a font by resource name, or create a default placeholder.
    pub fn get_or_default(&mut self, name: &str) -> &PdfFont {
        if !self.fonts.contains_key(name) {
            self.fonts
                .insert(name.to_string(), PdfFont::default_font(name));
        }
        &self.fonts[name]
    }

    /// Insert a resolved font.
    pub fn insert(&mut self, name: String, font: PdfFont) {
        self.fonts.insert(name, font);
    }

    /// Get a font by name.
    pub fn get(&self, name: &str) -> Option<&PdfFont> {
        self.fonts.get(name)
    }

    /// Iterate over all fonts.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &PdfFont)> {
        self.fonts.iter()
    }
}

/// Resolve fonts from a page's /Resources /Font dictionary.
pub fn resolve_page_fonts(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> FontCache {
    let mut cache = FontCache::default();

    // Get page dictionary
    let page_dict = match doc.get_object(page_id).and_then(|o| o.as_dict()) {
        Ok(d) => d,
        Err(_) => return cache,
    };

    // Get Resources dict
    let resources = match page_dict.get(b"Resources") {
        Ok(r) => resolve_object(doc, r),
        Err(_) => return cache,
    };

    let resources_dict = match resources.as_dict() {
        Ok(d) => d,
        Err(_) => return cache,
    };

    // Get Font dict from resources
    let font_dict = match resources_dict.get(b"Font") {
        Ok(f) => resolve_object(doc, f),
        Err(_) => return cache,
    };

    let font_dict = match font_dict.as_dict() {
        Ok(d) => d,
        Err(_) => return cache,
    };

    // Resolve each font
    for (name_bytes, font_ref) in font_dict.iter() {
        let name = String::from_utf8_lossy(name_bytes).to_string();
        let font_obj = resolve_object(doc, font_ref);

        if let Ok(fd) = font_obj.as_dict() {
            let pdf_font = resolve_font_dict(doc, &name, fd);
            cache.insert(name, pdf_font);
        }
    }

    cache
}

/// Resolve a font dictionary into a PdfFont.
pub(crate) fn resolve_font_dict(
    doc: &lopdf::Document,
    name: &str,
    dict: &lopdf::Dictionary,
) -> PdfFont {
    let subtype = dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| {
            if let lopdf::Object::Name(n) = o {
                Some(String::from_utf8_lossy(n).to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "Type1".to_string());

    let base_font_raw = dict.get(b"BaseFont").ok().and_then(|o| {
        if let lopdf::Object::Name(n) = o {
            Some(String::from_utf8_lossy(n).to_string())
        } else {
            None
        }
    });

    let encoding = dict
        .get(b"Encoding")
        .ok()
        .and_then(|o| {
            let resolved = resolve_object(doc, o);
            match resolved {
                lopdf::Object::Name(n) => Some(String::from_utf8_lossy(&n).to_string()),
                lopdf::Object::Dictionary(ref d) => d.get(b"BaseEncoding").ok().and_then(|be| {
                    if let lopdf::Object::Name(n) = be {
                        Some(String::from_utf8_lossy(n).to_string())
                    } else {
                        None
                    }
                }),
                _ => None,
            }
        })
        .unwrap_or_else(|| "WinAnsiEncoding".to_string());

    let is_type0 = subtype == "Type0";
    let is_type3 = subtype == "Type3";
    let bytes_per_code: u8 = if is_type0 { 2 } else { 1 };

    let mut widths = resolve_widths(doc, dict);
    let mut to_unicode = resolve_tounicode(doc, dict);

    let mut flags = 0u32;
    let mut italic_angle = 0.0f64;
    let mut stem_v = 0.0f64;
    let mut descriptor_font_weight: Option<f64> = None;
    let mut descriptor_font_name: Option<String> = None;
    let mut ascent = 800.0f64;
    let mut descent = -200.0f64;
    let mut font_bbox = [0.0f64, -200.0, 1000.0, 800.0];
    let mut default_width = 1000.0f64;

    if is_type0 {
        if let Ok(desc_ref) = dict.get(b"DescendantFonts") {
            let desc_obj = resolve_object(doc, desc_ref);
            if let Ok(desc_arr) = desc_obj.as_array() {
                if let Some(first) = desc_arr.first() {
                    let desc_font_obj = resolve_object(doc, first);
                    if let Ok(desc_dict) = desc_font_obj.as_dict() {
                        if let Ok(dw) = desc_dict.get(b"DW") {
                            if let Some(dw_val) = obj_to_f64(resolve_object(doc, dw)) {
                                default_width = dw_val;
                            }
                        }
                        resolve_cid_widths(doc, desc_dict, &mut widths);
                        let desc = resolve_font_descriptor(doc, desc_dict);
                        flags = desc.flags;
                        italic_angle = desc.italic_angle;
                        stem_v = desc.stem_v;
                        descriptor_font_weight = desc.font_weight;
                        descriptor_font_name = desc.font_name;
                        ascent = desc.ascent;
                        descent = desc.descent;
                        font_bbox = desc.font_bbox;

                        // Layered Unicode fallback when ToUnicode is missing/sparse:
                        // Encoding/Differences on Type0 (rare) → embedded TTF cmap.
                        resolve_encoding_differences(doc, dict, &mut to_unicode);
                        resolve_cid_font_cmap_fallback(doc, desc_dict, &mut to_unicode);
                    }
                }
            }
        }
    } else {
        resolve_encoding_differences(doc, dict, &mut to_unicode);
        if subtype == "Type1" {
            resolve_type1_font_program_encoding(doc, dict, &mut to_unicode);
        }
        let desc = resolve_font_descriptor(doc, dict);
        flags = desc.flags;
        italic_angle = desc.italic_angle;
        stem_v = desc.stem_v;
        descriptor_font_weight = desc.font_weight;
        descriptor_font_name = desc.font_name;
        ascent = desc.ascent;
        descent = desc.descent;
        font_bbox = desc.font_bbox;

        // Type 3: FontBBox often lives on the font dict itself (Skia).
        if is_type3 {
            if let Some(bbox) = read_number_array(doc, dict, b"FontBBox") {
                if bbox.len() >= 4 {
                    font_bbox = [bbox[0], bbox[1], bbox[2], bbox[3]];
                }
            }
        }
    }

    let base_font = crate::pdf::font_style::resolve_base_font_name(
        base_font_raw.as_deref(),
        descriptor_font_name.as_deref(),
    );

    let is_standard = is_standard_font(&base_font);
    if is_standard {
        default_width = standard_font_default_width(&base_font);
    }

    // Type 3: normalize glyph-space widths / bbox through FontMatrix into the
    // per-mille text-space invariant documented on PdfFont.
    if is_type3 {
        let matrix_vals = read_number_array(doc, dict, b"FontMatrix").unwrap_or_default();
        let matrix = crate::pdf::font_type3::font_matrix_from_array(&matrix_vals);
        let scale = matrix.em_scale();
        crate::pdf::font_type3::normalize_widths(&mut widths, scale);
        default_width *= scale;
        if desc_missing_ascent_descent(doc, dict) {
            let (a, d) = crate::pdf::font_type3::vertical_extent(font_bbox, matrix);
            ascent = a;
            descent = d;
        }
        font_bbox = crate::pdf::font_type3::normalize_font_bbox(font_bbox, matrix);
    }

    let weight =
        crate::pdf::font_style::resolve_font_weight(&base_font, descriptor_font_weight, stem_v);

    PdfFont {
        name: name.to_string(),
        base_font,
        subtype,
        widths,
        default_width,
        to_unicode,
        encoding,
        is_standard,
        flags,
        italic_angle,
        weight,
        bytes_per_code,
        ascent,
        descent,
        font_bbox,
    }
}

/// True when the font descriptor lacks explicit /Ascent and /Descent.
fn desc_missing_ascent_descent(doc: &lopdf::Document, dict: &lopdf::Dictionary) -> bool {
    let Ok(fd_ref) = dict.get(b"FontDescriptor") else {
        return true;
    };
    let fd_obj = resolve_object(doc, fd_ref);
    let Ok(fd) = fd_obj.as_dict() else {
        return true;
    };
    fd.get(b"Ascent").is_err() && fd.get(b"Descent").is_err()
}

/// Read a numeric array from a dictionary key (resolving references).
fn read_number_array(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    key: &[u8],
) -> Option<Vec<f64>> {
    let obj = resolve_object(doc, dict.get(key).ok()?);
    let arr = obj.as_array().ok()?;
    let vals: Vec<f64> = arr
        .iter()
        .filter_map(|o| obj_to_f64(resolve_object(doc, o)))
        .collect();
    if vals.is_empty() {
        None
    } else {
        Some(vals)
    }
}

/// Resolve font widths from /Widths array.
fn resolve_widths(doc: &lopdf::Document, dict: &lopdf::Dictionary) -> HashMap<u32, f64> {
    let mut widths = HashMap::new();

    let first_char = dict
        .get(b"FirstChar")
        .ok()
        .and_then(|o| obj_to_i64(resolve_object(doc, o)))
        .unwrap_or(0) as u32;

    if let Ok(widths_ref) = dict.get(b"Widths") {
        let widths_obj = resolve_object(doc, widths_ref);
        if let Ok(arr) = widths_obj.as_array() {
            for (i, w) in arr.iter().enumerate() {
                if let Some(width) = obj_to_f64(resolve_object(doc, w)) {
                    widths.insert(first_char + i as u32, width);
                }
            }
        }
    }

    widths
}

/// Resolve CID font widths from /W array.
///
/// The /W array format: [cid [w1 w2 ...] cid_start cid_end w ...]
/// Two forms:
/// - `cid [w1, w2, w3, ...]` — consecutive CIDs starting at cid
/// - `cid_start cid_end w` — range of CIDs all with same width
fn resolve_cid_widths(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    widths: &mut HashMap<u32, f64>,
) {
    let w_obj = match dict.get(b"W") {
        Ok(o) => resolve_object(doc, o),
        Err(_) => return,
    };
    let w_arr = match w_obj.as_array() {
        Ok(a) => a,
        Err(_) => return,
    };

    let mut i = 0;
    while i < w_arr.len() {
        let first = resolve_object(doc, &w_arr[i]);
        if let Some(cid_start) = obj_to_i64(first) {
            let cid_start = cid_start as u32;
            i += 1;
            if i >= w_arr.len() {
                break;
            }
            let next = resolve_object(doc, &w_arr[i]);
            if let Ok(arr) = next.as_array() {
                // Form 1: cid [w1 w2 w3 ...]
                for (j, w) in arr.iter().enumerate() {
                    if let Some(width) = obj_to_f64(resolve_object(doc, w)) {
                        widths.insert(cid_start + j as u32, width);
                    }
                }
                i += 1;
            } else if let Some(cid_end) = obj_to_i64(next) {
                // Form 2: cid_start cid_end w
                let cid_end = cid_end as u32;
                i += 1;
                if i >= w_arr.len() {
                    break;
                }
                let w_val = resolve_object(doc, &w_arr[i]);
                if let Some(width) = obj_to_f64(w_val) {
                    for cid in cid_start..=cid_end {
                        widths.insert(cid, width);
                    }
                }
                i += 1;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
}

/// Resolve ToUnicode CMap.
fn resolve_tounicode(doc: &lopdf::Document, dict: &lopdf::Dictionary) -> HashMap<u32, String> {
    let mut mapping = HashMap::new();

    if let Ok(tounicode_ref) = dict.get(b"ToUnicode") {
        let tounicode_ref = match tounicode_ref {
            lopdf::Object::Reference(r) => *r,
            _ => return mapping,
        };

        if let Ok(stream) = doc.get_object(tounicode_ref) {
            if let Ok(stream) = stream.as_stream() {
                if let Ok(data) = stream.decompressed_content() {
                    parse_cmap(&data, &mut mapping);
                }
            }
        }
    }

    mapping
}

/// Parse /Encoding dictionary's /Differences array and add mappings to to_unicode.
///
/// The /Differences array format is: [code, name, name, ..., code, name, ...]
/// Numbers reset the current character code; names are applied sequentially.
fn resolve_encoding_differences(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    to_unicode: &mut HashMap<u32, String>,
) {
    let enc_obj = match dict.get(b"Encoding") {
        Ok(o) => resolve_object(doc, o),
        Err(_) => return,
    };

    let enc_dict = match enc_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return, // Not a dictionary (just a name like "WinAnsiEncoding")
    };

    let diffs_obj = match enc_dict.get(b"Differences") {
        Ok(o) => resolve_object(doc, o),
        Err(_) => return,
    };

    let diffs = match diffs_obj.as_array() {
        Ok(a) => a,
        Err(_) => return,
    };

    let mut current_code: u32 = 0;
    for item in diffs {
        let resolved = resolve_object(doc, item);
        match resolved {
            lopdf::Object::Integer(i) => {
                current_code = i as u32;
            }
            lopdf::Object::Name(ref name_bytes) => {
                let glyph_name = String::from_utf8_lossy(name_bytes).to_string();
                // Only add if not already mapped by ToUnicode (ToUnicode takes priority)
                if let std::collections::hash_map::Entry::Vacant(e) = to_unicode.entry(current_code)
                {
                    if let Some(unicode) = glyph_name_to_unicode(&glyph_name) {
                        e.insert(unicode);
                    }
                }
                current_code += 1;
            }
            _ => {}
        }
    }
}

/// Extract encoding from an embedded Type1 font program (/FontFile stream).
///
/// Type1 font programs contain an Encoding array defined in PostScript.
/// The pattern is: `dup <code> /<glyphname> put`
/// This function parses those lines and maps glyph names to Unicode.
fn resolve_type1_font_program_encoding(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    to_unicode: &mut HashMap<u32, String>,
) {
    // Get FontDescriptor
    let fd_obj = match dict.get(b"FontDescriptor") {
        Ok(o) => resolve_object(doc, o),
        Err(_) => return,
    };
    let fd = match fd_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return,
    };

    // Try /FontFile (Type1), /FontFile2 (TrueType), /FontFile3 (CFF/OpenType)
    let stream_data = None
        .or_else(|| get_font_file_data(doc, fd, b"FontFile"))
        .or_else(|| get_font_file_data(doc, fd, b"FontFile3"));

    let data = match stream_data {
        Some(d) => d,
        None => return,
    };

    // Parse the Type1 font program's Encoding vector
    // Look for patterns: dup <number> /<name> put
    let text = String::from_utf8_lossy(&data);

    // Check if this font uses StandardEncoding (no custom encoding)
    if text.contains("/Encoding StandardEncoding def") {
        return; // Standard encoding, handled by WinAnsi fallback
    }

    for line in text.lines() {
        let trimmed = line.trim();
        // Pattern: "dup 121 /dagger put" or "dup 3 /asteriskmath put"
        if !trimmed.starts_with("dup ") || !trimmed.ends_with(" put") {
            continue;
        }
        let inner = &trimmed[4..trimmed.len() - 4].trim();
        // Split: "121 /dagger"
        let parts: Vec<&str> = inner.splitn(2, ' ').collect();
        if parts.len() != 2 {
            continue;
        }
        let code: u32 = match parts[0].trim().parse() {
            Ok(c) => c,
            Err(_) => continue,
        };
        let glyph = parts[1].trim().trim_start_matches('/');
        if glyph.is_empty() || glyph == ".notdef" {
            continue;
        }
        // Only add if not already mapped by ToUnicode or Encoding/Differences
        if let std::collections::hash_map::Entry::Vacant(e) = to_unicode.entry(code) {
            if let Some(unicode) = glyph_name_to_unicode(glyph) {
                e.insert(unicode);
            }
        }
    }
}

/// Get decompressed data from a font file stream.
fn get_font_file_data(
    doc: &lopdf::Document,
    fd: &lopdf::Dictionary,
    key: &[u8],
) -> Option<Vec<u8>> {
    let ff_ref = fd.get(key).ok()?;
    let ff_id = match ff_ref {
        lopdf::Object::Reference(r) => *r,
        _ => return None,
    };
    let ff_obj = doc.get_object(ff_id).ok()?;
    let stream = ff_obj.as_stream().ok()?;
    stream.decompressed_content().ok()
}

/// Identity-H / CIDFont Unicode fallback from an embedded TrueType/OpenType cmap.
///
/// When `/ToUnicode` is absent, many Type0 fonts still embed a Unicode cmap in
/// `/FontFile2`. For Identity `CIDToGIDMap`, CID == glyph id.
fn resolve_cid_font_cmap_fallback(
    doc: &lopdf::Document,
    cid_font_dict: &lopdf::Dictionary,
    to_unicode: &mut HashMap<u32, String>,
) {
    // Prefer existing ToUnicode coverage — only fill vacant codes.
    let fd_obj = match cid_font_dict.get(b"FontDescriptor") {
        Ok(o) => resolve_object(doc, o),
        Err(_) => return,
    };
    let fd = match fd_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return,
    };

    let data = None
        .or_else(|| get_font_file_data(doc, fd, b"FontFile2"))
        .or_else(|| get_font_file_data(doc, fd, b"FontFile3"));
    let Some(font_bytes) = data else {
        return;
    };

    // CIDToGIDMap: Identity (default for Identity-H) or a stream of 2-byte GIDs.
    let cid_to_gid = resolve_cid_to_gid_map(doc, cid_font_dict);

    let Ok(face) = ttf_parser::Face::parse(&font_bytes, 0) else {
        return;
    };

    // Build glyph_id → Unicode from the best Unicode cmap.
    let mut gid_to_unicode: HashMap<u16, String> = HashMap::new();
    if let Some(subtable) = face
        .tables()
        .cmap
        .and_then(|c| c.subtables.into_iter().find(|s| s.is_unicode()))
    {
        subtable.codepoints(|cp| {
            if let Some(gid) = subtable.glyph_index(cp) {
                if let Some(ch) = char::from_u32(cp) {
                    gid_to_unicode
                        .entry(gid.0)
                        .or_insert_with(|| ch.to_string());
                }
            }
        });
    }

    if gid_to_unicode.is_empty() {
        return;
    }

    let max_cid = cid_to_gid
        .as_ref()
        .map(|m| m.len() as u32)
        .unwrap_or_else(|| face.number_of_glyphs() as u32);

    for cid in 0..max_cid {
        if to_unicode.contains_key(&cid) {
            continue;
        }
        let gid = match &cid_to_gid {
            Some(map) => map.get(cid as usize).copied().unwrap_or(cid as u16),
            None => cid as u16,
        };
        if let Some(uni) = gid_to_unicode.get(&gid) {
            to_unicode.insert(cid, uni.clone());
        }
    }
}

/// Parse `/CIDToGIDMap`. `None` means Identity (CID == GID).
fn resolve_cid_to_gid_map(doc: &lopdf::Document, cid_font_dict: &lopdf::Dictionary) -> Option<Vec<u16>> {
    let obj = cid_font_dict.get(b"CIDToGIDMap").ok()?;
    let resolved = resolve_object(doc, obj);
    match resolved {
        lopdf::Object::Name(n) if n.as_slice() == b"Identity" => None,
        lopdf::Object::Stream(stream) => {
            let data = stream.decompressed_content().ok()?;
            if data.len() < 2 {
                return None;
            }
            let mut map = Vec::with_capacity(data.len() / 2);
            for chunk in data.chunks_exact(2) {
                map.push(u16::from_be_bytes([chunk[0], chunk[1]]));
            }
            Some(map)
        }
        _ => None,
    }
}

/// Map a PostScript/PDF glyph name to its Unicode string representation.
///
/// Uses the full Adobe Glyph List (4281 entries) with fallbacks for
/// uniXXXX, uXXXXXX, and single-character names.
/// Ligatures are decomposed to plain text (e.g., "fi" → "fi" not U+FB01).
fn glyph_name_to_unicode(name: &str) -> Option<String> {
    // Override: decompose ligatures to plain text for readability
    match name {
        "fi" => return Some("fi".to_string()),
        "fl" => return Some("fl".to_string()),
        "ff" => return Some("ff".to_string()),
        "ffi" => return Some("ffi".to_string()),
        "ffl" => return Some("ffl".to_string()),
        "IJ" => return Some("IJ".to_string()),
        "ij" => return Some("ij".to_string()),
        _ => {}
    }

    // 1. Try direct lookup (AGL, TeX, uniXXXX, uXXXXXX, single-char)
    if let Some(s) = resolve_glyph_component(name) {
        return Some(s);
    }

    // 2. AGL Specification: strip variant suffix after first period
    //    e.g. "c.sc" → "c", "seven.oldstyle" → "seven", "A.swash" → "A"
    let base = if let Some(dot_pos) = name.find('.') {
        &name[..dot_pos]
    } else {
        name
    };

    if base != name {
        if let Some(s) = resolve_glyph_component(base) {
            return Some(s);
        }
    }

    // 3. AGL Specification: underscore ligature decomposition
    //    e.g. "f_l" → "fl", "T_h" → "Th", "f_f_i" → "ffi"
    if base.contains('_') {
        let mut result = String::new();
        for component in base.split('_') {
            if let Some(s) = resolve_glyph_component(component) {
                result.push_str(&s);
            } else {
                return None; // If any component fails, give up
            }
        }
        if !result.is_empty() {
            return Some(result);
        }
    }

    None
}

/// Resolve a single glyph name component to Unicode (no period/underscore decomposition).
fn resolve_glyph_component(name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }

    // Fast path: look up in the full AGL
    if let Some(s) = AGL_MAP.get(name) {
        return Some(s.clone());
    }

    // TeX-specific glyph names not in AGL (CMSY, CMMI, CMEX, etc.)
    if let Some(s) = TEX_GLYPH_MAP.get(name) {
        return Some((*s).to_string());
    }

    // Single-character glyph names (letter names match directly)
    if name.len() == 1 {
        return Some(name.to_string());
    }

    // "uniXXXX" format (Adobe convention for BMP codepoints)
    if let Some(hex) = name.strip_prefix("uni") {
        if hex.len() == 4 {
            if let Ok(cp) = u32::from_str_radix(hex, 16) {
                if let Some(c) = char::from_u32(cp) {
                    return Some(c.to_string());
                }
            }
        }
        // uniXXXXYYYY... — sequence of BMP codepoints in groups of 4
        if hex.len() > 4 && hex.len() % 4 == 0 {
            let mut s = String::new();
            for chunk in hex.as_bytes().chunks(4) {
                if let Ok(h) = std::str::from_utf8(chunk) {
                    if let Ok(cp) = u32::from_str_radix(h, 16) {
                        if let Some(c) = char::from_u32(cp) {
                            s.push(c);
                        }
                    }
                }
            }
            if !s.is_empty() {
                return Some(s);
            }
        }
    }

    // "uXXXXXX" format (Adobe convention for supplementary codepoints)
    if let Some(hex) = name.strip_prefix('u') {
        if (4..=6).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            if let Ok(cp) = u32::from_str_radix(hex, 16) {
                if let Some(c) = char::from_u32(cp) {
                    return Some(c.to_string());
                }
            }
        }
    }

    None
}

/// Parse a basic CMap for bfchar/bfrange mappings.
fn parse_cmap(data: &[u8], mapping: &mut HashMap<u32, String>) {
    let text = String::from_utf8_lossy(data);

    // Parse beginbfchar ... endbfchar blocks
    let mut in_bfchar = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("beginbfchar") {
            in_bfchar = true;
            continue;
        }
        if trimmed.contains("endbfchar") {
            in_bfchar = false;
            continue;
        }
        if in_bfchar {
            // Format: <XXXX> <YYYY>
            let parts: Vec<&str> = trimmed.split('>').collect();
            if parts.len() >= 2 {
                if let (Some(src), Some(dst)) =
                    (parse_hex_value(parts[0]), parse_hex_unicode(parts[1]))
                {
                    mapping.insert(src, dst);
                }
            }
        }
    }

    // Parse beginbfrange ... endbfrange blocks
    // Supports two formats:
    //   <start> <end> <dst>       — single hex value, incremented across range
    //   <start> <end> [<v1> <v2> ...] — array of Unicode values, one per code
    let mut in_bfrange = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("beginbfrange") {
            in_bfrange = true;
            continue;
        }
        if trimmed.contains("endbfrange") {
            in_bfrange = false;
            continue;
        }
        if in_bfrange {
            // Check if this line has an array format: <start> <end> [<v1> <v2> ...]
            if let Some(bracket_start) = trimmed.find('[') {
                // Array format
                let before_bracket = &trimmed[..bracket_start];
                let parts: Vec<&str> = before_bracket.split('>').collect();
                if parts.len() >= 2 {
                    if let (Some(start), Some(end)) =
                        (parse_hex_value(parts[0]), parse_hex_value(parts[1]))
                    {
                        // Extract values inside brackets [ ... ]
                        let bracket_end = trimmed.rfind(']').unwrap_or(trimmed.len());
                        let inside = &trimmed[bracket_start + 1..bracket_end];
                        let values: Vec<String> = inside
                            .split('>')
                            .filter_map(|s| {
                                let s = s.trim().trim_start_matches('<');
                                if s.is_empty() {
                                    None
                                } else {
                                    parse_hex_unicode_str(s)
                                }
                            })
                            .collect();
                        for (i, val) in values.iter().enumerate() {
                            let code = start + i as u32;
                            if code > end {
                                break;
                            }
                            mapping.insert(code, val.clone());
                        }
                    }
                }
            } else {
                // Standard format: <XXXX> <YYYY> <ZZZZ>
                let parts: Vec<&str> = trimmed.split('>').collect();
                if parts.len() >= 3 {
                    if let (Some(start), Some(end), Some(dst_start)) = (
                        parse_hex_value(parts[0]),
                        parse_hex_value(parts[1]),
                        parse_hex_value(parts[2]),
                    ) {
                        for code in start..=end {
                            let unicode_point = dst_start + (code - start);
                            if let Some(c) = char::from_u32(unicode_point) {
                                mapping.insert(code, c.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Parse a hex value from a CMap entry like "<0041".
fn parse_hex_value(s: &str) -> Option<u32> {
    let cleaned = s.trim().trim_start_matches('<').trim();
    if cleaned.is_empty() {
        return None;
    }
    u32::from_str_radix(cleaned, 16).ok()
}

/// Parse a hex Unicode value from a CMap entry like " <0041".
fn parse_hex_unicode(s: &str) -> Option<String> {
    let cleaned = s
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim();
    parse_hex_unicode_str(cleaned)
}

/// Parse a hex Unicode string from pre-cleaned hex digits like "0041" or "D800DC00".
fn parse_hex_unicode_str(cleaned: &str) -> Option<String> {
    if cleaned.is_empty() {
        return None;
    }

    // May be multi-byte Unicode
    let mut result = String::new();
    let bytes: Vec<&str> = cleaned
        .as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect();

    for hex_str in bytes {
        if let Ok(code_point) = u32::from_str_radix(hex_str, 16) {
            if let Some(c) = char::from_u32(code_point) {
                result.push(c);
            }
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Fields extracted from a `/FontDescriptor` dictionary.
struct FontDescriptorFields {
    flags: u32,
    italic_angle: f64,
    stem_v: f64,
    font_weight: Option<f64>,
    font_name: Option<String>,
    ascent: f64,
    descent: f64,
    font_bbox: [f64; 4],
}

/// Resolve font descriptor fields.
fn resolve_font_descriptor(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
) -> FontDescriptorFields {
    let mut fields = FontDescriptorFields {
        flags: 0,
        italic_angle: 0.0,
        stem_v: 0.0,
        font_weight: None,
        font_name: None,
        ascent: 800.0,
        descent: -200.0,
        font_bbox: [0.0, -200.0, 1000.0, 800.0],
    };

    if let Ok(fd_ref) = dict.get(b"FontDescriptor") {
        let fd_obj = resolve_object(doc, fd_ref);
        if let Ok(fd) = fd_obj.as_dict() {
            fields.flags = fd
                .get(b"Flags")
                .ok()
                .and_then(|o| obj_to_i64(resolve_object(doc, o)))
                .unwrap_or(0) as u32;

            fields.italic_angle = fd
                .get(b"ItalicAngle")
                .ok()
                .and_then(|o| obj_to_f64(resolve_object(doc, o)))
                .unwrap_or(0.0);

            fields.stem_v = fd
                .get(b"StemV")
                .ok()
                .and_then(|o| obj_to_f64(resolve_object(doc, o)))
                .unwrap_or(0.0);

            fields.font_weight = fd
                .get(b"FontWeight")
                .ok()
                .and_then(|o| obj_to_f64(resolve_object(doc, o)));

            fields.font_name = fd.get(b"FontName").ok().and_then(|o| {
                let resolved = resolve_object(doc, o);
                if let lopdf::Object::Name(n) = resolved {
                    Some(String::from_utf8_lossy(&n).to_string())
                } else {
                    None
                }
            });

            if let Some(vals) = read_number_array(doc, fd, b"FontBBox") {
                if vals.len() >= 4 {
                    fields.font_bbox = [vals[0], vals[1], vals[2], vals[3]];
                }
            }

            if let Ok(a_ref) = fd.get(b"Ascent") {
                if let Some(a) = obj_to_f64(resolve_object(doc, a_ref)) {
                    fields.ascent = a;
                }
            } else {
                fields.ascent = fields.font_bbox[3];
            }

            if let Ok(d_ref) = fd.get(b"Descent") {
                if let Some(d) = obj_to_f64(resolve_object(doc, d_ref)) {
                    fields.descent = d;
                }
            } else {
                fields.descent = fields.font_bbox[1];
            }
        }
    }

    fields
}

/// Resolve a PDF object reference.
fn resolve_object<'a>(doc: &'a lopdf::Document, obj: &'a lopdf::Object) -> lopdf::Object {
    match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).cloned().unwrap_or(lopdf::Object::Null),
        other => other.clone(),
    }
}

/// Convert a PDF object to f64.
fn obj_to_f64(obj: lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Integer(i) => Some(i as f64),
        lopdf::Object::Real(f) => Some(f),
        _ => None,
    }
}

/// Convert a PDF object to i64.
fn obj_to_i64(obj: lopdf::Object) -> Option<i64> {
    match obj {
        lopdf::Object::Integer(i) => Some(i),
        lopdf::Object::Real(f) => Some(f as i64),
        _ => None,
    }
}

/// Check if a font name is one of the standard 14 fonts.
fn is_standard_font(name: &str) -> bool {
    matches!(
        name,
        "Courier"
            | "Courier-Bold"
            | "Courier-Oblique"
            | "Courier-BoldOblique"
            | "Helvetica"
            | "Helvetica-Bold"
            | "Helvetica-Oblique"
            | "Helvetica-BoldOblique"
            | "Times-Roman"
            | "Times-Bold"
            | "Times-Italic"
            | "Times-BoldItalic"
            | "Symbol"
            | "ZapfDingbats"
    )
}

/// Get default glyph width for standard fonts.
fn standard_font_default_width(name: &str) -> f64 {
    if name.starts_with("Courier") {
        600.0
    } else {
        // Variable-width fonts — use a reasonable default
        500.0
    }
}

/// Decode a character code using MacRomanEncoding (Mac OS Roman).
///
/// Maps byte values 0x80-0xFF to the Mac OS Roman Unicode equivalents.
/// Used when a font's /Encoding is explicitly set to MacRomanEncoding.
fn decode_macroman(code: u32) -> String {
    if code < 128 {
        if let Some(c) = char::from_u32(code) {
            return c.to_string();
        }
    }
    let mapped = match code {
        0x80 => '\u{00C4}', // Ä
        0x81 => '\u{00C5}', // Å
        0x82 => '\u{00C7}', // Ç
        0x83 => '\u{00C9}', // É
        0x84 => '\u{00D1}', // Ñ
        0x85 => '\u{00D6}', // Ö
        0x86 => '\u{00DC}', // Ü
        0x87 => '\u{00E1}', // á
        0x88 => '\u{00E0}', // à
        0x89 => '\u{00E2}', // â
        0x8A => '\u{00E4}', // ä
        0x8B => '\u{00E3}', // ã
        0x8C => '\u{00E5}', // å
        0x8D => '\u{00E7}', // ç
        0x8E => '\u{00E9}', // é
        0x8F => '\u{00E8}', // è
        0x90 => '\u{00EA}', // ê
        0x91 => '\u{00EB}', // ë
        0x92 => '\u{00ED}', // í
        0x93 => '\u{00EC}', // ì
        0x94 => '\u{00EE}', // î
        0x95 => '\u{00EF}', // ï
        0x96 => '\u{00F1}', // ñ
        0x97 => '\u{00F3}', // ó
        0x98 => '\u{00F2}', // ò
        0x99 => '\u{00F4}', // ô
        0x9A => '\u{00F6}', // ö
        0x9B => '\u{00F5}', // õ
        0x9C => '\u{00FA}', // ú
        0x9D => '\u{00F9}', // ù
        0x9E => '\u{00FB}', // û
        0x9F => '\u{00FC}', // ü
        0xA0 => '\u{2020}', // †
        0xA1 => '\u{00B0}', // °
        0xA2 => '\u{00A2}', // ¢
        0xA3 => '\u{00A3}', // £
        0xA4 => '\u{00A7}', // §
        0xA5 => '\u{2022}', // •
        0xA6 => '\u{00B6}', // ¶
        0xA7 => '\u{00DF}', // ß
        0xA8 => '\u{00AE}', // ®
        0xA9 => '\u{00A9}', // ©
        0xAA => '\u{2122}', // ™
        0xAB => '\u{00B4}', // ´
        0xAC => '\u{00A8}', // ¨
        0xAD => '\u{2260}', // ≠
        0xAE => '\u{00C6}', // Æ
        0xAF => '\u{00D8}', // Ø
        0xB0 => '\u{221E}', // ∞
        0xB1 => '\u{00B1}', // ±
        0xB2 => '\u{2264}', // ≤
        0xB3 => '\u{2265}', // ≥
        0xB4 => '\u{00A5}', // ¥
        0xB5 => '\u{00B5}', // µ
        0xB6 => '\u{2202}', // ∂
        0xB7 => '\u{2211}', // ∑
        0xB8 => '\u{220F}', // ∏
        0xB9 => '\u{03C0}', // π
        0xBA => '\u{222B}', // ∫
        0xBB => '\u{00AA}', // ª
        0xBC => '\u{00BA}', // º
        0xBD => '\u{03A9}', // Ω
        0xBE => '\u{00E6}', // æ
        0xBF => '\u{00F8}', // ø
        0xC0 => '\u{00BF}', // ¿
        0xC1 => '\u{00A1}', // ¡
        0xC2 => '\u{00AC}', // ¬
        0xC3 => '\u{221A}', // √
        0xC4 => '\u{0192}', // ƒ
        0xC5 => '\u{2248}', // ≈
        0xC6 => '\u{2206}', // ∆
        0xC7 => '\u{00AB}', // «
        0xC8 => '\u{00BB}', // »
        0xC9 => '\u{2026}', // …
        0xCA => '\u{00A0}', // non-breaking space
        0xCB => '\u{00C0}', // À
        0xCC => '\u{00C3}', // Ã
        0xCD => '\u{00D5}', // Õ
        0xCE => '\u{0152}', // Œ
        0xCF => '\u{0153}', // œ
        0xD0 => '\u{2013}', // –
        0xD1 => '\u{2014}', // —
        0xD2 => '\u{201C}', // "
        0xD3 => '\u{201D}', // "
        0xD4 => '\u{2018}', // '
        0xD5 => '\u{2019}', // '
        0xD6 => '\u{00F7}', // ÷
        0xD7 => '\u{25CA}', // ◊
        0xD8 => '\u{00FF}', // ÿ
        0xD9 => '\u{0178}', // Ÿ
        0xDA => '\u{2044}', // ⁄
        0xDB => '\u{20AC}', // €
        0xDC => '\u{2039}', // ‹
        0xDD => '\u{203A}', // ›
        0xDE => '\u{FB01}', // fi ligature
        0xDF => '\u{FB02}', // fl ligature
        0xE0 => '\u{2021}', // ‡
        0xE1 => '\u{00B7}', // ·
        0xE2 => '\u{201A}', // ‚
        0xE3 => '\u{201E}', // „
        0xE4 => '\u{2030}', // ‰
        0xE5 => '\u{00C2}', // Â
        0xE6 => '\u{00CA}', // Ê
        0xE7 => '\u{00C1}', // Á
        0xE8 => '\u{00CB}', // Ë
        0xE9 => '\u{00C8}', // È
        0xEA => '\u{00CD}', // Í
        0xEB => '\u{00CE}', // Î
        0xEC => '\u{00CF}', // Ï
        0xED => '\u{00CC}', // Ì
        0xEE => '\u{00D3}', // Ó
        0xEF => '\u{00D4}', // Ô
        0xF0 => '\u{F8FF}', // Apple logo
        0xF1 => '\u{00D2}', // Ò
        0xF2 => '\u{00DA}', // Ú
        0xF3 => '\u{00DB}', // Û
        0xF4 => '\u{00D9}', // Ù
        0xF5 => '\u{0131}', // ı
        0xF6 => '\u{02C6}', // ˆ
        0xF7 => '\u{02DC}', // ˜
        0xF8 => '\u{00AF}', // ¯
        0xF9 => '\u{02D8}', // ˘
        0xFA => '\u{02D9}', // ˙
        0xFB => '\u{02DA}', // ˚
        0xFC => '\u{00B8}', // ¸
        0xFD => '\u{02DD}', // ˝
        0xFE => '\u{02DB}', // ˛
        0xFF => '\u{02C7}', // ˇ
        _ => {
            return char::from_u32(code)
                .map(|c| c.to_string())
                .unwrap_or_default();
        }
    };
    mapped.to_string()
}

/// Decode a character code using WinAnsiEncoding (Windows-1252 superset).
fn decode_winansi(code: u32) -> String {
    if code < 128 {
        // ASCII range
        if let Some(c) = char::from_u32(code) {
            return c.to_string();
        }
    }
    // Windows-1252 special range 0x80-0x9F
    let mapped = match code {
        0x80 => '\u{20AC}', // Euro sign
        0x82 => '\u{201A}', // Single low-9 quotation mark
        0x83 => '\u{0192}', // Latin small letter f with hook
        0x84 => '\u{201E}', // Double low-9 quotation mark
        0x85 => '\u{2026}', // Horizontal ellipsis
        0x86 => '\u{2020}', // Dagger
        0x87 => '\u{2021}', // Double dagger
        0x88 => '\u{02C6}', // Modifier letter circumflex accent
        0x89 => '\u{2030}', // Per mille sign
        0x8A => '\u{0160}', // Latin capital letter S with caron
        0x8B => '\u{2039}', // Single left-pointing angle quotation mark
        0x8C => '\u{0152}', // Latin capital ligature OE
        0x8E => '\u{017D}', // Latin capital letter Z with caron
        0x91 => '\u{2018}', // Left single quotation mark
        0x92 => '\u{2019}', // Right single quotation mark
        0x93 => '\u{201C}', // Left double quotation mark
        0x94 => '\u{201D}', // Right double quotation mark
        0x95 => '\u{2022}', // Bullet
        0x96 => '\u{2013}', // En dash
        0x97 => '\u{2014}', // Em dash
        0x98 => '\u{02DC}', // Small tilde
        0x99 => '\u{2122}', // Trade mark sign
        0x9A => '\u{0161}', // Latin small letter s with caron
        0x9B => '\u{203A}', // Single right-pointing angle quotation mark
        0x9C => '\u{0153}', // Latin small ligature oe
        0x9E => '\u{017E}', // Latin small letter z with caron
        0x9F => '\u{0178}', // Latin capital letter Y with diaeresis
        // 0xA0-0xFF: direct Unicode mapping (Latin-1 Supplement)
        c @ 0xA0..=0xFF => {
            return char::from_u32(c)
                .map(|ch| ch.to_string())
                .unwrap_or_default();
        }
        _ => {
            return char::from_u32(code)
                .map(|c| c.to_string())
                .unwrap_or_default();
        }
    };
    mapped.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_font() {
        let font = PdfFont::default_font("F1");
        assert_eq!(font.name, "F1");
        assert!((font.default_width - 600.0).abs() < 1e-10);
    }

    #[test]
    fn test_bold_detection() {
        let mut font = PdfFont::default_font("F1");
        font.base_font = "Helvetica-Bold".to_string();
        assert!(font.is_bold());

        font.base_font = "Helvetica".to_string();
        assert!(!font.is_bold());
    }

    #[test]
    fn test_decode_winansi() {
        assert_eq!(decode_winansi(65), "A");
        assert_eq!(decode_winansi(0x93), "\u{201C}");
    }

    #[test]
    fn test_parse_hex_value() {
        assert_eq!(parse_hex_value("<0041"), Some(0x41));
        assert_eq!(parse_hex_value("<00FF"), Some(0xFF));
    }

    #[test]
    fn test_standard_font() {
        assert!(is_standard_font("Helvetica"));
        assert!(is_standard_font("Courier-Bold"));
        assert!(!is_standard_font("ArialMT"));
    }

    #[test]
    fn type3_skia_widths_normalized_to_per_mille() {
        use lopdf::{dictionary, Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        // Minimal empty CharProc (glyph drawing not needed for metrics).
        let proc_id = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
        let fd_id = doc.add_object(dictionary! {
            "Type" => "FontDescriptor",
            "FontName" => "AAAA+.SFNS-Regular_wght2580000",
            "FontWeight" => 600,
            "StemV" => 262,
            "Flags" => 4,
            "ItalicAngle" => 0,
        });
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type3",
            "FontMatrix" => vec![
                Object::Real(1.0 / 2048.0),
                Object::Integer(0),
                Object::Integer(0),
                Object::Real(-1.0 / 2048.0),
                Object::Integer(0),
                Object::Integer(0),
            ],
            "FirstChar" => 0,
            "LastChar" => 3,
            "Widths" => vec![
                Object::Integer(2048),       // em-wide glyph → 1000 per-mille
                Object::Real(544.00195),     // space-like
                Object::Integer(1334),       // ~651 per-mille
                Object::Integer(0),
            ],
            "FontBBox" => vec![91.into(), 495.into(), 1949.into(), (-1951).into()],
            "FontDescriptor" => fd_id,
            "CharProcs" => dictionary! { "g0" => proc_id },
            "Encoding" => dictionary! {
                "Type" => "Encoding",
                "Differences" => vec![Object::Integer(0), Object::Name(b"g0".to_vec())],
            },
        });

        let font_obj = doc.get_object(font_id).unwrap().as_dict().unwrap();
        // Clone dict because resolve needs owned-style access through doc refs
        let font = resolve_font_dict(&doc, "F4", font_obj);

        assert_eq!(font.subtype, "Type3");
        assert!(
            font.base_font.contains("SFNS"),
            "expected descriptor FontName fallback, got {}",
            font.base_font
        );
        assert!(
            (font.weight - 600.0).abs() < 1e-6,
            "expected FontWeight=600 over StemV, got {}",
            font.weight
        );
        let w0 = font.glyph_width(0);
        assert!(
            (w0 - 1000.0).abs() < 1.0,
            "2048 glyph-space units → 1000 per-mille, got {w0}"
        );
        let w1 = font.glyph_width(1);
        let expected_space = 544.00195 * 1000.0 / 2048.0;
        assert!(
            (w1 - expected_space).abs() < 0.1,
            "space width got {w1}, expected ~{expected_space}"
        );
    }

    /// End-to-end: Skia-like Type3 PDF must preserve the word space in "Hi there".
    #[test]
    fn type3_skia_pdf_preserves_word_space() {
        use crate::api::config::ProcessingConfig;
        use crate::convert_bytes;
        use crate::output::markdown::to_markdown;
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let proc_id = doc.add_object(Stream::new(dictionary! {}, Vec::new()));

        let fd_id = doc.add_object(dictionary! {
            "Type" => "FontDescriptor",
            "FontName" => "AAAA+.SFNS-Regular",
            "FontWeight" => 400,
            "StemV" => 80,
            "Flags" => 32,
        });

        // Widths in 1/2048 glyph space (Skia). After normalize: *1000/2048.
        // Codes: H i space t h e r e
        let widths: Vec<Object> = [1200, 500, 544, 600, 700, 650, 550, 650]
            .into_iter()
            .map(Object::Integer)
            .collect();

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type3",
            "FontMatrix" => vec![
                Object::Real(1.0 / 2048.0),
                0.into(),
                0.into(),
                Object::Real(-1.0 / 2048.0),
                0.into(),
                0.into(),
            ],
            "FirstChar" => 0,
            "LastChar" => 7,
            "Widths" => widths,
            "FontBBox" => vec![0.into(), 0.into(), 2048.into(), (-2048).into()],
            "FontDescriptor" => fd_id,
            "CharProcs" => dictionary! { "g0" => proc_id },
            // AGL glyph names so Differences → Unicode without ToUnicode.
            "Encoding" => dictionary! {
                "Type" => "Encoding",
                "Differences" => vec![
                    Object::Integer(0),
                    Object::Name(b"H".to_vec()),
                    Object::Name(b"i".to_vec()),
                    Object::Name(b"space".to_vec()),
                    Object::Name(b"t".to_vec()),
                    Object::Name(b"h".to_vec()),
                    Object::Name(b"e".to_vec()),
                    Object::Name(b"r".to_vec()),
                    Object::Name(b"e".to_vec()),
                ],
            },
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });

        // One Tj per glyph (Skia style). Space glyph is filtered by
        // TextLine::value(); needs_space must re-insert it from the gap,
        // which only works when Type3 widths are normalized.
        let mut ops = vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), Object::Real(24.0)]),
            Operation::new("Td", vec![72.into(), 700.into()]),
        ];
        for code in [0u8, 1, 2, 3, 4, 5, 6, 7] {
            ops.push(Operation::new(
                "Tj",
                vec![Object::String(vec![code], lopdf::StringFormat::Hexadecimal)],
            ));
        }
        ops.push(Operation::new("ET", vec![]));

        let content_id = doc.add_object(Stream::new(
            dictionary! {},
            Content { operations: ops }.encode().unwrap(),
        ));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();

        let mut config = ProcessingConfig::default();
        config.raster_table_ocr = false;
        config.image_output = crate::api::config::ImageOutput::Off;
        let parsed = convert_bytes(&bytes, "skia-type3.pdf", &config).unwrap();
        let md = to_markdown(&parsed).unwrap();
        assert!(
            md.contains("Hi there"),
            "Type3 width normalization must preserve word space; got: {md:?}"
        );
    }

    #[test]
    fn identity_h_ascii_cid_fallback_without_tounicode() {
        // Type0 Identity-H, no ToUnicode: ASCII-range CIDs must still decode.
        use lopdf::{dictionary, Document, Object};

        let mut doc = Document::with_version("1.5");
        let desc_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => "Dummy",
            "CIDSystemInfo" => dictionary! {
                "Registry" => Object::string_literal("Adobe"),
                "Ordering" => Object::string_literal("Identity"),
                "Supplement" => 0,
            },
            "DW" => 1000,
        });
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "Dummy",
            "Encoding" => "Identity-H",
            "DescendantFonts" => vec![desc_id.into()],
        });
        let font_obj = doc.get_object(font_id).unwrap().as_dict().unwrap().clone();
        let font = resolve_font_dict(&doc, "F1", &font_obj);
        assert_eq!(font.bytes_per_code, 2);
        assert_eq!(font.decode_char(0x0041), "A");
        assert_eq!(font.decode_char(0x0020), " ");
        // High unmapped CID must not WinAnsi-mojibake.
        assert_eq!(font.decode_char(0x1234), "\u{FFFD}");
    }

    /// Optional local-only check against a Skia Chrome PDF on disk.
    #[test]
    fn type3_real_skia_pdf_word_ratio_env_gated() {
        let path = match std::env::var("EDGEPARSE_SKIA_PDF") {
            Ok(p) => std::path::PathBuf::from(p),
            Err(_) => return, // skip quietly when unset
        };
        if !path.exists() {
            return;
        }
        use crate::api::config::ProcessingConfig;
        use crate::convert;
        use crate::output::markdown::to_markdown;

        let mut config = ProcessingConfig::default();
        config.raster_table_ocr = false;
        config.image_output = crate::api::config::ImageOutput::Off;
        let doc = convert(&path, &config).unwrap();
        let md = to_markdown(&doc).unwrap();
        let words = md.split_whitespace().count();
        assert!(
            words > 2000,
            "expected dense word-spaced markdown, got {words} words"
        );
        assert!(
            !md.contains("W e ") && !md.contains("W ikipedia"),
            "stray W fragments still present"
        );
    }

    #[test]
    fn test_glyph_name_to_unicode_ligatures() {
        assert_eq!(glyph_name_to_unicode("fi"), Some("fi".to_string()));
        assert_eq!(glyph_name_to_unicode("fl"), Some("fl".to_string()));
        assert_eq!(glyph_name_to_unicode("ff"), Some("ff".to_string()));
        assert_eq!(glyph_name_to_unicode("ffi"), Some("ffi".to_string()));
        assert_eq!(glyph_name_to_unicode("ffl"), Some("ffl".to_string()));
    }

    #[test]
    fn test_glyph_name_to_unicode_common() {
        assert_eq!(glyph_name_to_unicode("percent"), Some("%".to_string()));
        assert_eq!(glyph_name_to_unicode("ampersand"), Some("&".to_string()));
        assert_eq!(glyph_name_to_unicode("parenleft"), Some("(".to_string()));
        assert_eq!(
            glyph_name_to_unicode("endash"),
            Some("\u{2013}".to_string())
        );
        assert_eq!(glyph_name_to_unicode("A"), Some("A".to_string()));
        assert_eq!(glyph_name_to_unicode("uni0041"), Some("A".to_string()));
    }

    #[test]
    fn test_glyph_name_to_unicode_unknown() {
        assert_eq!(glyph_name_to_unicode("nonexistent_glyph_xyz"), None);
    }

    #[test]
    fn test_glyph_name_to_unicode_agl_extended() {
        // Characters only available via the full AGL, not the old hand-coded list
        assert_eq!(
            glyph_name_to_unicode("Dcroat"),
            Some("\u{0110}".to_string())
        );
        assert_eq!(
            glyph_name_to_unicode("dcroat"),
            Some("\u{0111}".to_string())
        );
        assert_eq!(
            glyph_name_to_unicode("Emacron"),
            Some("\u{0112}".to_string())
        );
        assert_eq!(
            glyph_name_to_unicode("afii10017"),
            Some("\u{0410}".to_string())
        ); // Cyrillic А
        assert_eq!(
            glyph_name_to_unicode("afii57636"),
            Some("\u{20AA}".to_string())
        ); // New Sheqel sign
           // Multi-codepoint entry (Hebrew dalet + hataf patah)
        assert_eq!(
            glyph_name_to_unicode("dalethatafpatah"),
            Some("\u{05D3}\u{05B2}".to_string())
        );
    }

    #[test]
    fn test_glyph_name_to_unicode_uni_formats() {
        // uniXXXX format
        assert_eq!(glyph_name_to_unicode("uni0041"), Some("A".to_string()));
        assert_eq!(glyph_name_to_unicode("uni00E9"), Some("é".to_string()));
        // uniXXXXYYYY (sequence of BMP codepoints)
        assert_eq!(glyph_name_to_unicode("uni00410042"), Some("AB".to_string()));
        // uXXXXXX format (supplementary)
        assert_eq!(
            glyph_name_to_unicode("u1F600"),
            Some("\u{1F600}".to_string())
        );
    }

    #[test]
    fn test_parse_cmap_bfrange_array() {
        // bfrange with array format: <start> <end> [<v1> <v2> <v3>]
        let cmap_data = b"beginbfrange\n<0001> <0003> [<0041> <0042> <0043>]\nendbfrange\n";
        let mut mapping = HashMap::new();
        parse_cmap(cmap_data, &mut mapping);
        assert_eq!(mapping.get(&1), Some(&"A".to_string()));
        assert_eq!(mapping.get(&2), Some(&"B".to_string()));
        assert_eq!(mapping.get(&3), Some(&"C".to_string()));
    }

    #[test]
    fn test_parse_cmap_bfrange_incremented() {
        // bfrange with incrementing hex: <start> <end> <dst>
        let cmap_data = b"beginbfrange\n<0041> <0043> <0061>\nendbfrange\n";
        let mut mapping = HashMap::new();
        parse_cmap(cmap_data, &mut mapping);
        assert_eq!(mapping.get(&0x41), Some(&"a".to_string()));
        assert_eq!(mapping.get(&0x42), Some(&"b".to_string()));
        assert_eq!(mapping.get(&0x43), Some(&"c".to_string()));
    }

    #[test]
    fn test_decode_winansi_extended() {
        assert_eq!(decode_winansi(0x82), "\u{201A}"); // Single low-9 quotation
        assert_eq!(decode_winansi(0x83), "\u{0192}"); // f with hook
        assert_eq!(decode_winansi(0x8A), "\u{0160}"); // S with caron
        assert_eq!(decode_winansi(0x8C), "\u{0152}"); // OE ligature
        assert_eq!(decode_winansi(0x99), "\u{2122}"); // Trademark
        assert_eq!(decode_winansi(0x9C), "\u{0153}"); // oe ligature
        assert_eq!(decode_winansi(0xA0), "\u{00A0}"); // NBSP
        assert_eq!(decode_winansi(0xE9), "\u{00E9}"); // é
    }

    #[test]
    fn test_tex_glyph_names() {
        // CMSY font glyph names (TeX-specific, not in AGL)
        assert_eq!(
            glyph_name_to_unicode("asteriskmath"),
            Some("\u{2217}".to_string())
        ); // ∗
        assert_eq!(
            glyph_name_to_unicode("diamondmath"),
            Some("\u{22C4}".to_string())
        ); // ⋄
        assert_eq!(
            glyph_name_to_unicode("minusplus"),
            Some("\u{2213}".to_string())
        ); // ∓
        assert_eq!(
            glyph_name_to_unicode("circleminus"),
            Some("\u{2296}".to_string())
        ); // ⊖
        assert_eq!(
            glyph_name_to_unicode("circledot"),
            Some("\u{2299}".to_string())
        ); // ⊙
        assert_eq!(
            glyph_name_to_unicode("follows"),
            Some("\u{227B}".to_string())
        ); // ≻
        assert_eq!(
            glyph_name_to_unicode("lessmuch"),
            Some("\u{226A}".to_string())
        ); // ≪
        assert_eq!(
            glyph_name_to_unicode("greatermuch"),
            Some("\u{226B}".to_string())
        ); // ≫
        assert_eq!(
            glyph_name_to_unicode("latticetop"),
            Some("\u{22A4}".to_string())
        ); // ⊤
        assert_eq!(
            glyph_name_to_unicode("mapsto"),
            Some("\u{21A6}".to_string())
        ); // ↦
           // These should still resolve via AGL
        assert_eq!(
            glyph_name_to_unicode("dagger"),
            Some("\u{2020}".to_string())
        ); // †
        assert_eq!(
            glyph_name_to_unicode("daggerdbl"),
            Some("\u{2021}".to_string())
        ); // ‡
        assert_eq!(
            glyph_name_to_unicode("braceleft"),
            Some("\u{007B}".to_string())
        ); // {
        assert_eq!(
            glyph_name_to_unicode("braceright"),
            Some("\u{007D}".to_string())
        ); // }
    }
}
