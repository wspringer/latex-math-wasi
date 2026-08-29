//! Fill colour: device spaces and a Separation spot colour.

#[path = "../../../tests/corpus/mod.rs"]
mod corpus;

use latex_math_core::{Font, FontSet, Options};
use latex_math_pdf::{to_pdf, Color, PdfError, PdfOptions};

fn pdf_with(color: Color) -> Result<Vec<u8>, PdfError> {
    let bytes = std::fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap();
    let font = Font::parse(&bytes).unwrap();
    let tree =
        latex_math_core::render(r"\frac{a}{b}", &FontSet::single(&font), &Options::default())
            .unwrap();
    to_pdf(
        &tree,
        &[&font],
        &PdfOptions {
            padding: 1.0,
            color,
            ..PdfOptions::default()
        },
    )
}

/// The page content stream is the first stream in the file.
fn content(pdf: &[u8]) -> String {
    let text = String::from_utf8_lossy(pdf);
    let start = text.find("stream\n").unwrap() + 7;
    let end = text[start..].find("endstream").unwrap() + start;
    text[start..end].to_string()
}

#[test]
fn default_is_100_percent_k() {
    let pdf = pdf_with(Color::default()).unwrap();
    assert!(content(&pdf).starts_with("0 0 0 1 k\n"));
}

#[test]
fn device_spaces_set_the_fill_before_any_drawing() {
    let gray = pdf_with(Color::Gray(0.25)).unwrap();
    assert!(content(&gray).starts_with("0.25 g\n"));
    let rgb = pdf_with(Color::Rgb([1.0, 0.0, 0.5])).unwrap();
    assert!(content(&rgb).starts_with("1 0 0.5 rg\n"));
    let cmyk = pdf_with(Color::Cmyk([1.0, 0.44, 0.0, 0.0])).unwrap();
    let c = content(&cmyk);
    assert!(c.starts_with("1 0.44 0 0 k\n"));
    // The rules (fraction bar) come after the text object and use the same fill.
    assert!(c.contains("re f"), "{c}");
    assert_eq!(
        c.matches(" k\n").count(),
        1,
        "one fill colour for glyphs and rules"
    );
}

#[test]
fn spot_colour_is_a_separation_with_a_cmyk_alternate() {
    let pdf = pdf_with(Color::Spot {
        name: "PANTONE 300 C".into(),
        tint: 0.8,
        cmyk: [1.0, 0.44, 0.0, 0.0],
    })
    .unwrap();
    let c = content(&pdf);
    assert!(c.starts_with("/CS0 cs 0.8 scn\n"), "{c}");
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("/Separation"), "no Separation colour space");
    assert!(
        text.contains("/PANTONE#20300#20C"),
        "colorant name should be escaped as a PDF name"
    );
    assert!(text.contains("/DeviceCMYK"), "alternate space missing");
    assert!(text.contains("/FunctionType 2"), "tint transform missing");
    assert!(
        text.contains("/ColorSpace"),
        "page resources lack the colour space"
    );
}

#[test]
fn bad_colours_are_rejected() {
    assert!(matches!(
        pdf_with(Color::Cmyk([0.0, 0.0, 0.0, 1.5])),
        Err(PdfError::BadColor(_))
    ));
    assert!(matches!(
        pdf_with(Color::Gray(-0.1)),
        Err(PdfError::BadColor(_))
    ));
    assert!(matches!(
        pdf_with(Color::Spot {
            name: String::new(),
            tint: 1.0,
            cmyk: [0.0; 4]
        }),
        Err(PdfError::BadColor(_))
    ));
}

#[test]
fn colour_does_not_move_anything() {
    let a = pdf_with(Color::default()).unwrap();
    let b = pdf_with(Color::Rgb([0.0, 0.0, 1.0])).unwrap();
    let strip = |s: String| s.lines().skip(1).collect::<Vec<_>>().join("\n");
    assert_eq!(strip(content(&a)), strip(content(&b)));
}

#[test]
fn palette_switches_colour_per_glyph_and_shares_separations() {
    use std::collections::BTreeMap;
    let bytes = std::fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap();
    let font = Font::parse(&bytes).unwrap();
    let options = Options {
        palette: vec!["accent".into(), "muted".into()],
        ..Options::default()
    };
    let tree = latex_math_core::render(
        r"\color{accent}{x} y \color{accent}{\frac{a}{b}} \color{muted}{z} \color{red}{w}",
        &FontSet::single(&font),
        &options,
    )
    .unwrap();
    let mut palette = BTreeMap::new();
    palette.insert(
        "accent".to_string(),
        Color::Spot {
            name: "PANTONE 300 C".into(),
            tint: 1.0,
            cmyk: [1.0, 0.44, 0.0, 0.0],
        },
    );
    palette.insert("muted".to_string(), Color::Cmyk([0.0, 0.0, 0.0, 0.5]));
    let pdf = to_pdf(
        &tree,
        &[&font],
        &PdfOptions {
            padding: 0.0,
            color: Color::default(),
            palette,
        },
    )
    .unwrap();
    let c = content(&pdf);
    let ops: Vec<&str> = c
        .lines()
        .filter(|l| l.ends_with(" k") || l.ends_with(" scn") || l.ends_with(" rg"))
        .collect();
    assert_eq!(
        ops,
        [
            "0 0 0 1 k",     // document colour
            "/CS0 cs 1 scn", // x
            "0 0 0 1 k",     // y
            "/CS0 cs 1 scn", // a, b
            "0 0 0 0.5 k",   // z
            "1 0 0 rg",      // w (CSS red → DeviceRGB)
            "/CS0 cs 1 scn", // the fraction bar, after ET
        ],
        "{c}"
    );
    let text = String::from_utf8_lossy(&pdf);
    assert_eq!(
        text.matches("/Separation").count(),
        1,
        "one Separation for one spot"
    );
    assert!(!text.contains("/CS1"), "the same spot must reuse CS0");
}

#[test]
fn palette_name_missing_from_pdf_palette_is_an_error() {
    let bytes = std::fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap();
    let font = Font::parse(&bytes).unwrap();
    let options = Options {
        palette: vec!["accent".into()],
        ..Options::default()
    };
    let tree =
        latex_math_core::render(r"\color{accent}{x}", &FontSet::single(&font), &options).unwrap();
    assert!(matches!(
        to_pdf(&tree, &[&font], &PdfOptions::default()),
        Err(PdfError::UnknownColor(_))
    ));
}
