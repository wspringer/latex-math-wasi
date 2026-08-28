//! Render tree → SVG.
//!
//! Every glyph outline is emitted once as a `<path>` in `<defs>` and placed with `<use>`.
//! Output is deterministic: fixed decimal precision, definitions sorted by (font, glyph id),
//! no hash-map iteration anywhere.

use std::collections::BTreeSet;
use std::fmt::Write;

use latex_math_core::{Font, RenderTree};

/// SVG output options.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgOptions {
    /// Decimal places for coordinates and transforms.
    pub precision: usize,
    /// Extra space around the bounding box, in user units.
    pub padding: f64,
    /// Fill colour for glyphs and rules (any SVG paint).
    pub fill: String,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions {
            precision: 3,
            padding: 0.0,
            fill: "#000".to_string(),
        }
    }
}

/// Errors from SVG generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgError {
    /// A glyph instance referenced a font index outside `fonts`.
    MissingFont(usize),
}

impl std::fmt::Display for SvgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SvgError::MissingFont(i) => write!(
                f,
                "render tree references font #{i}, which was not supplied"
            ),
        }
    }
}

impl std::error::Error for SvgError {}

/// Writes `tree` as an SVG document. `fonts[i]` must be the font glyph instances with `font == i` refer to.
pub fn to_svg(
    tree: &RenderTree,
    fonts: &[&Font<'_>],
    options: &SvgOptions,
) -> Result<String, SvgError> {
    for g in &tree.glyphs {
        if g.font >= fonts.len() {
            return Err(SvgError::MissingFont(g.font));
        }
    }
    let p = options.precision;
    let fmt = |v: f64| format_fixed(v, p);

    let x = tree.bbox.x_min - options.padding;
    let y = tree.bbox.y_min - options.padding;
    let w = tree.bbox.width() + 2.0 * options.padding;
    let h = tree.bbox.height() + 2.0 * options.padding;

    let mut out = String::new();
    writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="{} {} {} {}" width="{}" height="{}">"#,
        fmt(x), fmt(y), fmt(w), fmt(h), fmt(w), fmt(h)
    )
    .unwrap();

    // Definitions, sorted, one per distinct (font, gid).
    let used: BTreeSet<(usize, u16)> = tree.glyphs.iter().map(|g| (g.font, g.gid)).collect();
    let mut defined: BTreeSet<(usize, u16)> = BTreeSet::new();
    if !used.is_empty() {
        out.push_str("<defs>\n");
        for &(font, gid) in &used {
            let face = fonts[font].font();
            let mut path = PathSink {
                d: String::new(),
                precision: 2,
            };
            if face
                .outline_glyph(ttf_parser::GlyphId(gid), &mut path)
                .is_some()
                && !path.d.is_empty()
            {
                writeln!(out, r#"<path id="g{font}-{gid}" d="{}"/>"#, path.d).unwrap();
                defined.insert((font, gid));
            }
        }
        out.push_str("</defs>\n");
    }

    writeln!(out, r#"<g fill="{}">"#, options.fill).unwrap();
    for g in &tree.glyphs {
        if !defined.contains(&(g.font, g.gid)) {
            continue; // empty outline (e.g. space)
        }
        let upem = f64::from(fonts[g.font].font().units_per_em());
        let scale = g.size / upem;
        writeln!(
            out,
            r##"<use xlink:href="#g{}-{}" transform="translate({} {}) scale({})"/>"##,
            g.font,
            g.gid,
            fmt(g.x),
            fmt(g.y),
            format_fixed(scale, 6)
        )
        .unwrap();
    }
    for r in &tree.rules {
        writeln!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}"/>"#,
            fmt(r.x),
            fmt(r.y),
            fmt(r.width),
            fmt(r.height)
        )
        .unwrap();
    }
    out.push_str("</g>\n</svg>\n");
    Ok(out)
}

/// Fixed-precision decimal with trailing zeros removed and no negative zero.
fn format_fixed(v: f64, precision: usize) -> String {
    let s = format!("{v:.precision$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

/// Collects a glyph outline as SVG path data, flipping y (font units go up, SVG goes down).
struct PathSink {
    d: String,
    precision: usize,
}

impl PathSink {
    fn n(&self, v: f32) -> String {
        format_fixed(f64::from(v), self.precision)
    }
}

impl ttf_parser::OutlineBuilder for PathSink {
    fn move_to(&mut self, x: f32, y: f32) {
        write!(self.d, "M{} {}", self.n(x), self.n(-y)).unwrap();
    }
    fn line_to(&mut self, x: f32, y: f32) {
        write!(self.d, "L{} {}", self.n(x), self.n(-y)).unwrap();
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        write!(
            self.d,
            "Q{} {} {} {}",
            self.n(x1),
            self.n(-y1),
            self.n(x),
            self.n(-y)
        )
        .unwrap();
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        write!(
            self.d,
            "C{} {} {} {} {} {}",
            self.n(x1),
            self.n(-y1),
            self.n(x2),
            self.n(-y2),
            self.n(x),
            self.n(-y)
        )
        .unwrap();
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_formatting() {
        assert_eq!(format_fixed(1.0, 3), "1");
        assert_eq!(format_fixed(-0.0001, 3), "0");
        assert_eq!(format_fixed(2.5, 3), "2.5");
        assert_eq!(format_fixed(-1.23456, 3), "-1.235");
        assert_eq!(format_fixed(100.0, 0), "100");
    }
}
