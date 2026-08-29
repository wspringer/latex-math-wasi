//! Colour in SVG: per-glyph fill from the palette or a literal.

#[path = "../../../tests/corpus/mod.rs"]
mod corpus;

use latex_math_core::{Font, FontSet, Options};
use latex_math_svg::{to_svg, SvgError, SvgOptions};
use std::collections::BTreeMap;

fn svg(tex: &str, palette: &[(&str, &str)]) -> Result<String, SvgError> {
    let bytes = std::fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap();
    let font = Font::parse(&bytes).unwrap();
    let options = Options {
        palette: palette.iter().map(|(n, _)| n.to_string()).collect(),
        ..Options::default()
    };
    let tree = latex_math_core::render(tex, &FontSet::single(&font), &options).unwrap();
    let svg_options = SvgOptions {
        palette: palette
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect::<BTreeMap<_, _>>(),
        ..SvgOptions::default()
    };
    to_svg(&tree, &[&font], &svg_options)
}

#[test]
fn palette_and_css_fills() {
    let out = svg(
        r"\color{accent}{x} \color{red}{y} z",
        &[("accent", "#1f5fbf")],
    )
    .unwrap();
    let uses: Vec<&str> = out.lines().filter(|l| l.starts_with("<use")).collect();
    assert_eq!(uses.len(), 3);
    assert!(uses[0].ends_with(r##" fill="#1f5fbf"/>"##), "{}", uses[0]);
    assert!(uses[1].ends_with(r##" fill="#ff0000"/>"##), "{}", uses[1]);
    assert!(!uses[2].contains("fill="), "{}", uses[2]);
}

#[test]
fn missing_palette_entry_is_an_error() {
    // The name is in the core palette (so it renders) but not in the SVG palette.
    let bytes = std::fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap();
    let font = Font::parse(&bytes).unwrap();
    let options = Options {
        palette: vec!["accent".into()],
        ..Options::default()
    };
    let tree =
        latex_math_core::render(r"\color{accent}{x}", &FontSet::single(&font), &options).unwrap();
    assert_eq!(
        to_svg(&tree, &[&font], &SvgOptions::default()).err(),
        Some(SvgError::UnknownColor("accent".into()))
    );
}

#[test]
fn uncoloured_output_is_unchanged() {
    let out = svg("x", &[]).unwrap();
    let uses: Vec<&str> = out.lines().filter(|l| l.starts_with("<use")).collect();
    assert!(uses.iter().all(|l| !l.contains("fill=")), "{out}");
}
