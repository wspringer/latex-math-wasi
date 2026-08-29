//! PNG output: right size, not blank, deterministic, and scale behaves.

#[path = "../../../tests/corpus/mod.rs"]
mod corpus;

use latex_math_core::{Font, FontSet, Options};
use latex_math_png::{pixel_size, to_png, PngOptions};
use latex_math_svg::{to_svg, SvgOptions};

fn font_bytes() -> Vec<u8> {
    std::fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap()
}

/// Decodes width/height from the IHDR chunk (bytes 16..24 of any PNG).
fn png_dims(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let be = |i: usize| u32::from_be_bytes(png[i..i + 4].try_into().unwrap());
    (be(16), be(20))
}

/// The viewBox size of an SVG, from its `width="…" height="…"` attributes.
fn svg_dims(svg: &str) -> (f64, f64) {
    let attr = |name: &str| -> f64 {
        let key = format!("{name}=\"");
        let start = svg.find(&key).unwrap() + key.len();
        let end = svg[start..].find('"').unwrap();
        svg[start..start + end].parse().unwrap()
    };
    (attr("width"), attr("height"))
}

fn render(tex: &str, font: &Font<'_>, scale: f64) -> (Vec<u8>, String) {
    let tree = latex_math_core::render(tex, &FontSet::single(font), &Options::default()).unwrap();
    let svg_options = SvgOptions {
        padding: 2.0,
        ..SvgOptions::default()
    };
    let png_options = PngOptions {
        scale,
        ..PngOptions::default()
    };
    let png = to_png(&tree, &[font], &svg_options, &png_options).unwrap();
    let svg = to_svg(&tree, &[font], &svg_options).unwrap();
    (png, svg)
}

#[test]
fn png_matches_svg_size_and_is_not_blank() {
    let bytes = font_bytes();
    let font = Font::parse(&bytes).unwrap();
    for (_, tex) in corpus::CORPUS {
        for scale in [1.0, 2.5] {
            let (png, svg) = render(tex, &font, scale);
            let (w, h) = svg_dims(&svg);
            assert_eq!(
                png_dims(&png),
                pixel_size(w, h, scale).unwrap(),
                "{tex} @ {scale}"
            );
        }
        // Decoding the PNG properly would need a decoder; resvg re-reads its own output
        // only via usvg (SVG), so check "not blank" the cheap way: a transparent PNG of
        // this size compresses to far fewer bytes than one with glyphs in it.
        let (png, _) = render(tex, &font, 1.0);
        let (w, h) = png_dims(&png);
        let blank = resvg::tiny_skia::Pixmap::new(w, h)
            .unwrap()
            .encode_png()
            .unwrap();
        assert!(png.len() > blank.len(), "{tex}: PNG looks blank");
    }
}

#[test]
fn png_is_deterministic() {
    let bytes = font_bytes();
    let font = Font::parse(&bytes).unwrap();
    let (a, _) = render(r"\frac{a}{b} + \sqrt{x^2}", &font, 2.0);
    let (b, _) = render(r"\frac{a}{b} + \sqrt{x^2}", &font, 2.0);
    assert_eq!(a, b);
}

#[test]
fn background_is_applied() {
    let bytes = font_bytes();
    let font = Font::parse(&bytes).unwrap();
    let tree = latex_math_core::render("x", &FontSet::single(&font), &Options::default()).unwrap();
    let svg_options = SvgOptions::default();
    let transparent = to_png(&tree, &[&font], &svg_options, &PngOptions::default()).unwrap();
    let white = to_png(
        &tree,
        &[&font],
        &svg_options,
        &PngOptions {
            background: Some([255, 255, 255, 255]),
            ..PngOptions::default()
        },
    )
    .unwrap();
    assert_ne!(transparent, white);
    assert_eq!(png_dims(&transparent), png_dims(&white));
}

#[test]
fn bad_scale_is_rejected() {
    let bytes = font_bytes();
    let font = Font::parse(&bytes).unwrap();
    let tree = latex_math_core::render("x", &FontSet::single(&font), &Options::default()).unwrap();
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let r = to_png(
            &tree,
            &[&font],
            &SvgOptions::default(),
            &PngOptions {
                scale,
                ..PngOptions::default()
            },
        );
        assert!(
            matches!(r, Err(latex_math_png::PngError::BadSize)),
            "scale {scale}"
        );
    }
}
