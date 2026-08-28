//! Render tree → PDF.
//!
//! Glyphs are real text: one Type0/CID font per used font, embedded as a subset
//! (`CIDFontType0C` for CFF outlines, `FontFile2` for TrueType), CIDs = subset glyph ids,
//! a ToUnicode CMap where the font's cmap knows the glyph. The content stream is only
//! `Tf`/`Td`/`Tj` for glyphs and `re f` for rules. Output is deterministic: no dates, no
//! random ids, subset tags derived from the glyph set.

use std::collections::{BTreeMap, BTreeSet};

use latex_math_core::{Font, RenderTree};
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use subsetter::GlyphRemapper;

/// PDF output options.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfOptions {
    /// Extra space around the bounding box, in user units (PDF points).
    pub padding: f64,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions { padding: 0.0 }
    }
}

/// Errors from PDF generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfError {
    /// A glyph instance referenced a font index outside `fonts`.
    MissingFont(usize),
    /// The subsetter rejected the font.
    Subset(usize, String),
    /// The subset font has neither a `CFF ` nor a `glyf` table.
    NoOutlines(usize),
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfError::MissingFont(i) => write!(
                f,
                "render tree references font #{i}, which was not supplied"
            ),
            PdfError::Subset(i, e) => write!(f, "font #{i}: subsetting failed: {e}"),
            PdfError::NoOutlines(i) => write!(f, "font #{i}: no CFF or glyf outlines"),
        }
    }
}

impl std::error::Error for PdfError {}

/// Page coordinates: origin bottom-left, y up.
struct PageMap {
    x_off: f64,
    y_top: f64,
    width: f64,
    height: f64,
}

impl PageMap {
    fn new(tree: &RenderTree, padding: f64) -> Self {
        PageMap {
            x_off: padding - tree.bbox.x_min,
            y_top: tree.bbox.y_max + padding,
            width: tree.bbox.width() + 2.0 * padding,
            height: tree.bbox.height() + 2.0 * padding,
        }
    }
    fn x(&self, x: f64) -> f64 {
        x + self.x_off
    }
    fn y(&self, y: f64) -> f64 {
        self.y_top - y
    }
}

/// One embedded font.
struct Embedded {
    index: usize,
    /// Original glyph ids in subset order; subset gid `n` (n ≥ 1) is `gids[n - 1]`, 0 is `.notdef`.
    gids: Vec<u16>,
    remapper: GlyphRemapper,
}

impl Embedded {
    fn original_gid(&self, new: u16) -> u16 {
        if new == 0 {
            0
        } else {
            self.gids[usize::from(new) - 1]
        }
    }
}

/// Writes `tree` as a single-page PDF. `fonts[i]` must be the font glyph instances with `font == i` refer to.
pub fn to_pdf(
    tree: &RenderTree,
    fonts: &[&Font<'_>],
    options: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    // Glyph sets per font, in font-index order.
    let mut used: BTreeMap<usize, BTreeSet<u16>> = BTreeMap::new();
    for g in &tree.glyphs {
        if g.font >= fonts.len() {
            return Err(PdfError::MissingFont(g.font));
        }
        used.entry(g.font).or_default().insert(g.gid);
    }
    let embedded: Vec<Embedded> = used
        .iter()
        .map(|(&index, gids)| {
            let mut remapper = GlyphRemapper::new();
            let gids: Vec<u16> = gids.iter().copied().filter(|&g| g != 0).collect();
            for &gid in &gids {
                remapper.remap(gid);
            }
            Embedded {
                index,
                gids,
                remapper,
            }
        })
        .collect();

    let page = PageMap::new(tree, options.padding);

    let mut pdf = Pdf::new();
    let mut next_ref = 1;
    let mut alloc = || {
        let r = Ref::new(next_ref);
        next_ref += 1;
        r
    };
    let catalog_id = alloc();
    let pages_id = alloc();
    let page_id = alloc();
    let content_id = alloc();
    let info_id = alloc();

    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id).kids([page_id]).count(1);
    pdf.document_info(info_id).producer(TextStr("latex-math"));

    let mut font_refs = Vec::new();
    for _ in &embedded {
        font_refs.push(alloc());
    }
    {
        let mut p = pdf.page(page_id);
        p.media_box(Rect::new(0.0, 0.0, page.width as f32, page.height as f32));
        p.parent(pages_id);
        p.contents(content_id);
        let mut res = p.resources();
        let mut fd = res.fonts();
        for (e, r) in embedded.iter().zip(&font_refs) {
            fd.pair(Name(font_name(e.index).as_bytes()), *r);
        }
        fd.finish();
        res.finish();
        p.finish();
    }

    // Content stream, written by hand: only Tf/Td/Tj and re/f, hex glyph strings, fixed
    // 3-decimal coordinates (pdf-writer's `Str` would pick literal-with-octal-escapes).
    let mut content = String::new();
    if !tree.glyphs.is_empty() {
        content.push_str("BT\n");
        let mut current: Option<(usize, f64)> = None;
        let (mut lx, mut ly) = (0.0f64, 0.0f64);
        for g in &tree.glyphs {
            if current != Some((g.font, g.size)) {
                content.push_str(&format!("/{} {} Tf\n", font_name(g.font), fixed(g.size)));
                current = Some((g.font, g.size));
            }
            // Td is relative; accumulate from the rounded values so the emitted deltas
            // sum to exactly the rounded absolute position (no drift across glyphs).
            let (px, py) = (round3(page.x(g.x)), round3(page.y(g.y)));
            content.push_str(&format!("{} {} Td\n", fixed(px - lx), fixed(py - ly)));
            (lx, ly) = (px, py);
            let e = embedded
                .iter()
                .find(|e| e.index == g.font)
                .expect("font collected above");
            let cid = e.remapper.get(g.gid).expect("glyph collected above");
            content.push_str(&format!("<{cid:04x}> Tj\n"));
        }
        content.push_str("ET\n");
    }
    for r in &tree.rules {
        content.push_str(&format!(
            "{} {} {} {} re f\n",
            fixed(page.x(r.x)),
            fixed(page.y(r.y + r.height)),
            fixed(r.width),
            fixed(r.height)
        ));
    }
    pdf.stream(content_id, content.as_bytes());

    // Fonts.
    for (e, &type0_id) in embedded.iter().zip(&font_refs) {
        let font = fonts[e.index];
        let cid_id = alloc();
        let desc_id = alloc();
        let data_id = alloc();
        let cmap_id = alloc();
        write_font(
            &mut pdf, font, e, type0_id, cid_id, desc_id, data_id, cmap_id,
        )?;
    }

    Ok(pdf.finish())
}

fn font_name(index: usize) -> String {
    format!("F{index}")
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Three decimals, trailing zeros trimmed, no negative zero.
fn fixed(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

#[allow(clippy::too_many_arguments)]
fn write_font(
    pdf: &mut Pdf,
    font: &Font<'_>,
    e: &Embedded,
    type0_id: Ref,
    cid_id: Ref,
    desc_id: Ref,
    data_id: Ref,
    cmap_id: Ref,
) -> Result<(), PdfError> {
    let face = font.font();
    let data = face.raw_face().data;
    let upem = f64::from(face.units_per_em());
    let to_pdf_units = |v: f64| (v / upem * 1000.0) as f32;

    let subset = subsetter::subset(data, 0, &e.remapper)
        .map_err(|err| PdfError::Subset(e.index, err.to_string()))?;
    let raw = ttf_parser::RawFace::parse(&subset, 0)
        .map_err(|err| PdfError::Subset(e.index, err.to_string()))?;
    let cff = raw
        .table(ttf_parser::Tag::from_bytes(b"CFF "))
        .map(|t| t.to_vec());
    let is_cff = cff.is_some();
    if !is_cff && raw.table(ttf_parser::Tag::from_bytes(b"glyf")).is_none() {
        return Err(PdfError::NoOutlines(e.index));
    }

    // Names.
    let ps_name = postscript_name(face).unwrap_or_else(|| format!("Font{}", e.index));
    let base_font = format!("{}+{}", subset_tag(&e.gids), ps_name);
    let system_info = SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };

    pdf.type0_font(type0_id)
        .base_font(Name(base_font.as_bytes()))
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid_id)
        .to_unicode(cmap_id);

    // Widths, indexed by new glyph id.
    let num = e.remapper.num_gids();
    let widths: Vec<f32> = (0..num)
        .map(|new| {
            let old = e.original_gid(new);
            let adv = face
                .glyph_hor_advance(ttf_parser::GlyphId(old))
                .unwrap_or(0);
            to_pdf_units(f64::from(adv))
        })
        .collect();
    {
        let mut cid = pdf.cid_font(cid_id);
        cid.subtype(if is_cff {
            CidFontType::Type0
        } else {
            CidFontType::Type2
        })
        .base_font(Name(base_font.as_bytes()))
        .system_info(system_info)
        .font_descriptor(desc_id)
        .default_width(0.0);
        if !is_cff {
            cid.cid_to_gid_map_predefined(Name(b"Identity"));
        }
        cid.widths().consecutive(0, widths.iter().copied());
        cid.finish();
    }

    // Descriptor.
    let bbox = face.global_bounding_box();
    let italic_angle = face.italic_angle();
    let mut flags = FontFlags::SYMBOLIC;
    if italic_angle != 0.0 {
        flags |= FontFlags::ITALIC;
    }
    let weight = f32::from(face.weight().to_number());
    let stem_v = 10.0 + 0.244 * (weight - 50.0);
    {
        let mut desc = pdf.font_descriptor(desc_id);
        desc.name(Name(base_font.as_bytes()))
            .flags(flags)
            .bbox(Rect::new(
                to_pdf_units(f64::from(bbox.x_min)),
                to_pdf_units(f64::from(bbox.y_min)),
                to_pdf_units(f64::from(bbox.x_max)),
                to_pdf_units(f64::from(bbox.y_max)),
            ))
            .italic_angle(italic_angle)
            .ascent(to_pdf_units(f64::from(face.ascender())))
            .descent(to_pdf_units(f64::from(face.descender())))
            .cap_height(to_pdf_units(f64::from(
                face.capital_height().unwrap_or(face.ascender()),
            )))
            .stem_v(stem_v);
        if is_cff {
            desc.font_file3(data_id);
        } else {
            desc.font_file2(data_id);
        }
        desc.finish();
    }

    // Font program.
    match &cff {
        Some(cff) => {
            let mut s = pdf.stream(data_id, cff);
            s.pair(Name(b"Subtype"), Name(b"CIDFontType0C"));
            s.finish();
        }
        None => {
            let mut s = pdf.stream(data_id, &subset);
            s.pair(Name(b"Length1"), subset.len() as i32);
            s.finish();
        }
    }

    // ToUnicode: new gid -> codepoint, from the font's Unicode cmap (smallest codepoint per glyph).
    let mut gid_to_char: BTreeMap<u16, char> = BTreeMap::new();
    if let Some(cmap) = face.tables().cmap {
        for sub in cmap.subtables {
            if !sub.is_unicode() {
                continue;
            }
            sub.codepoints(|cp| {
                if let (Some(gid), Some(ch)) = (sub.glyph_index(cp), char::from_u32(cp)) {
                    if e.remapper.get(gid.0).is_some() {
                        let entry = gid_to_char.entry(gid.0).or_insert(ch);
                        if ch < *entry {
                            *entry = ch;
                        }
                    }
                }
            });
        }
    }
    let mut cmap = UnicodeCmap::<u16>::new(Name(b"Custom"), system_info);
    for (gid, ch) in &gid_to_char {
        cmap.pair(e.remapper.get(*gid).expect("filtered above"), *ch);
    }
    pdf.cmap(cmap_id, &cmap.finish());

    Ok(())
}

fn postscript_name(face: &ttf_parser::Face<'_>) -> Option<String> {
    face.names()
        .into_iter()
        .filter(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
        .find_map(|n| n.to_string())
        .map(|s| {
            s.chars()
                .filter(|c| c.is_ascii_graphic() && *c != '/' && *c != '+')
                .collect()
        })
}

/// Six uppercase letters derived from the subset's glyph set (PDF 32000-1 §9.6.4).
fn subset_tag(gids: &[u16]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &old in gids {
        h ^= u64::from(old);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let mut tag = String::with_capacity(6);
    for _ in 0..6 {
        tag.push(char::from(b'A' + (h % 26) as u8));
        h /= 26;
    }
    tag
}
