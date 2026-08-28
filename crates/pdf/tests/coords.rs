//! Cross-backend property: the SVG and PDF backends place every glyph and rule at the
//! same coordinates (PDF is y-up with the page origin at the bbox corner, SVG is y-down
//! with the viewBox at the bbox corner). Both are parsed back from the emitted bytes.

#[path = "../../../tests/corpus/mod.rs"]
mod corpus;

use latex_wasi_core::{Font, FontSet, Options, RenderTree};
use latex_wasi_pdf::{to_pdf, PdfOptions};
use latex_wasi_svg::{to_svg, SvgOptions};

/// Glyph origins from the SVG, in SVG user space.
fn svg_glyphs(svg: &str) -> Vec<Point> {
    svg.lines()
        .filter(|l| l.starts_with("<use "))
        .map(|l| {
            let t = &l[l.find("translate(").unwrap() + 10..];
            let t = &t[..t.find(')').unwrap()];
            let mut it = t.split(' ').map(|v| v.parse::<f64>().unwrap());
            (it.next().unwrap(), it.next().unwrap())
        })
        .collect()
}

/// Rects from the SVG.
fn svg_rects(svg: &str) -> Vec<Box4> {
    svg.lines()
        .filter(|l| l.starts_with("<rect "))
        .map(|l| {
            let attr = |k: &str| {
                let s = &l[l.find(&format!("{k}=\"")).unwrap() + k.len() + 2..];
                s[..s.find('"').unwrap()].parse::<f64>().unwrap()
            };
            (attr("x"), attr("y"), attr("width"), attr("height"))
        })
        .collect()
}

type Point = (f64, f64);
type Box4 = (f64, f64, f64, f64);

/// Tokenises the (uncompressed) content stream: returns glyph origins (accumulated `Td`,
/// one per `Tj`) and rectangles from `re`.
fn pdf_ops(pdf: &[u8]) -> (Vec<Point>, Vec<Box4>) {
    let text = String::from_utf8_lossy(pdf);
    // The content stream is the first stream object (object 4), before any font programs.
    let start = text.find("stream\n").unwrap() + 7;
    let end = text[start..].find("endstream").unwrap() + start;
    let content = &text[start..end];
    let mut glyphs = Vec::new();
    let mut rects = Vec::new();
    let (mut lx, mut ly) = (0.0, 0.0);
    let mut stack: Vec<f64> = Vec::new();
    for tok in content.split_whitespace() {
        match tok {
            "Td" => {
                ly += stack.pop().unwrap();
                lx += stack.pop().unwrap();
            }
            "re" => {
                let h = stack.pop().unwrap();
                let w = stack.pop().unwrap();
                let y = stack.pop().unwrap();
                let x = stack.pop().unwrap();
                rects.push((x, y, w, h));
            }
            t if t.starts_with('<') => glyphs.push((lx, ly)),
            t => {
                if let Ok(v) = t.parse::<f64>() {
                    stack.push(v);
                } else {
                    stack.clear();
                }
            }
        }
    }
    (glyphs, rects)
}

fn has_outline(font: &Font<'_>, gid: u16) -> bool {
    struct Any(bool);
    impl ttf_parser::OutlineBuilder for Any {
        fn move_to(&mut self, _: f32, _: f32) {
            self.0 = true;
        }
        fn line_to(&mut self, _: f32, _: f32) {}
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {}
        fn close(&mut self) {}
    }
    let mut a = Any(false);
    font.font().outline_glyph(ttf_parser::GlyphId(gid), &mut a);
    a.0
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 2e-3
}

#[test]
fn svg_and_pdf_agree_on_every_glyph_and_rule() {
    for (_, font_file) in corpus::FONTS {
        let bytes = std::fs::read(corpus::font_path(font_file)).unwrap();
        let font = Font::parse(&bytes).unwrap();
        for (name, tex) in corpus::CORPUS {
            let tree: RenderTree =
                latex_wasi_core::render(tex, &FontSet::single(&font), &Options::default()).unwrap();
            let svg = to_svg(&tree, &[&font], &SvgOptions::default()).unwrap();
            let pdf = to_pdf(&tree, &[&font], &PdfOptions::default()).unwrap();

            let (pdf_glyphs, pdf_rects) = pdf_ops(&pdf);
            assert_eq!(
                pdf_glyphs.len(),
                tree.glyphs.len(),
                "{font_file}/{name}: pdf glyph count"
            );
            let svg_glyphs = svg_glyphs(&svg);
            let drawn: Vec<&latex_wasi_core::GlyphInstance> = tree
                .glyphs
                .iter()
                .filter(|g| has_outline(&font, g.gid))
                .collect();
            assert_eq!(
                svg_glyphs.len(),
                drawn.len(),
                "{font_file}/{name}: svg glyph count"
            );

            // PDF page origin is the bbox's bottom-left; SVG viewBox origin is its top-left.
            let (x0, y0, y1) = (tree.bbox.x_min, tree.bbox.y_min, tree.bbox.y_max);
            for (g, (px, py)) in tree.glyphs.iter().zip(&pdf_glyphs) {
                assert!(
                    close(g.x - x0, *px) && close(y1 - g.y, *py),
                    "{font_file}/{name}: pdf glyph at {px},{py} vs tree {},{}",
                    g.x,
                    g.y
                );
            }
            for (g, (sx, sy)) in drawn.iter().zip(&svg_glyphs) {
                assert!(
                    close(g.x, *sx) && close(g.y, *sy),
                    "{font_file}/{name}: svg glyph at {sx},{sy} vs tree {},{}",
                    g.x,
                    g.y
                );
            }
            // Same glyphs in both, expressed in one space.
            let mut pdf_iter = tree
                .glyphs
                .iter()
                .zip(&pdf_glyphs)
                .filter(|(g, _)| has_outline(&font, g.gid));
            for (sx, sy) in &svg_glyphs {
                let (_, (px, py)) = pdf_iter.next().unwrap();
                assert!(
                    close(sx - x0, *px) && close(y1 - sy, *py),
                    "{font_file}/{name}: svg/pdf glyph mismatch"
                );
            }

            let svg_rects = svg_rects(&svg);
            assert_eq!(
                svg_rects.len(),
                pdf_rects.len(),
                "{font_file}/{name}: rule count"
            );
            assert_eq!(svg_rects.len(), tree.rules.len());
            for ((sx, sy, sw, sh), (px, py, pw, ph)) in svg_rects.iter().zip(&pdf_rects) {
                assert!(
                    close(sx - x0, *px)
                        && close(y1 - (sy + sh), *py)
                        && close(*sw, *pw)
                        && close(*sh, *ph),
                    "{font_file}/{name}: rule mismatch"
                );
            }
            let _ = y0;
        }
    }
}

#[test]
fn pdf_is_deterministic_and_embeds_a_subset() {
    let bytes = std::fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap();
    let font = Font::parse(&bytes).unwrap();
    let tree = latex_wasi_core::render(
        corpus::CORPUS[0].1,
        &FontSet::single(&font),
        &Options::default(),
    )
    .unwrap();
    let a = to_pdf(&tree, &[&font], &PdfOptions::default()).unwrap();
    let b = to_pdf(&tree, &[&font], &PdfOptions::default()).unwrap();
    assert_eq!(a, b);
    let text = String::from_utf8_lossy(&a);
    assert!(
        text.contains("/Subtype /CIDFontType0C"),
        "CFF font must be embedded as CIDFontType0C"
    );
    assert!(
        text.contains("+STIXTwoMath-Regular"),
        "subset-tagged PostScript name"
    );
    assert!(text.contains("/ToUnicode"));
    assert!(
        a.len() < bytes.len() / 10,
        "subset must be far smaller than the font ({} vs {})",
        a.len(),
        bytes.len()
    );
}

#[test]
fn two_fonts_become_two_embedded_subsets() {
    let stix = std::fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap();
    let xits = std::fs::read(corpus::font_path("XITSMath-Regular.otf")).unwrap();
    let fonts = [Font::parse(&stix).unwrap(), Font::parse(&xits).unwrap()];
    let set = FontSet::new(&fonts, [0, 0, 1, 1]).unwrap();
    let tree = latex_wasi_core::render(r"x^{2} + y_{i}", &set, &Options::default()).unwrap();
    assert!(tree.glyphs.iter().any(|g| g.font == 1));
    let refs: Vec<&Font<'_>> = fonts.iter().collect();
    let pdf = to_pdf(&tree, &refs, &PdfOptions::default()).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert!(
        text.contains("/F0 ") && text.contains("/F1 "),
        "both font resources used"
    );
    assert!(text.contains("+STIXTwoMath-Regular") && text.contains("+XITSMath-Regular"));
    assert_eq!(text.matches("/Subtype /CIDFontType0C").count(), 2);
    // The SVG side agrees on coordinates for the mixed-font tree too.
    let svg = to_svg(&tree, &refs, &SvgOptions::default()).unwrap();
    let (pdf_glyphs, _) = pdf_ops(&pdf);
    let svg_glyphs = svg_glyphs(&svg);
    assert_eq!(pdf_glyphs.len(), svg_glyphs.len());
    for ((px, py), (sx, sy)) in pdf_glyphs.iter().zip(&svg_glyphs) {
        assert!(close(sx - tree.bbox.x_min, *px) && close(tree.bbox.y_max - sy, *py));
    }
}
