//! `\color` scopes: palette names, CSS names, `\phantom`, and errors.

use latex_math_core::{Error, Font, FontSet, Options, Paint, RenderTree, RGBA};

fn stix() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fonts/STIXTwoMath-Regular.otf"
    ))
    .unwrap()
}

fn render(tex: &str, font: &Font<'_>, palette: &[&str]) -> Result<RenderTree, Error> {
    let options = Options {
        palette: palette.iter().map(|s| s.to_string()).collect(),
        ..Options::default()
    };
    latex_math_core::render(tex, &FontSet::single(font), &options)
}

#[test]
fn palette_names_stay_names_for_the_backend() {
    let bytes = stix();
    let font = Font::parse(&bytes).unwrap();
    let tree = render(r"\color{accent}{x}y", &font, &["accent"]).unwrap();
    assert_eq!(tree.paints, vec![Paint::Named("accent".into())]);
    assert_eq!(tree.glyphs.len(), 2);
    assert_eq!(tree.glyphs[0].paint, Some(0));
    assert_eq!(tree.glyphs[1].paint, None);
}

#[test]
fn css_names_become_literals_and_palette_shadows_them() {
    let bytes = stix();
    let font = Font::parse(&bytes).unwrap();
    let tree = render(r"\color{red}{x} \blue{y}", &font, &[]).unwrap();
    assert_eq!(
        tree.paints,
        vec![
            Paint::Rgba(RGBA(0xff, 0, 0, 0xff)),
            Paint::Rgba(RGBA(0, 0, 0xff, 0xff))
        ]
    );
    let shadowed = render(r"\color{red}{x}", &font, &["red"]).unwrap();
    assert_eq!(shadowed.paints, vec![Paint::Named("red".into())]);
}

#[test]
fn unknown_names_are_an_error() {
    let bytes = stix();
    let font = Font::parse(&bytes).unwrap();
    assert_eq!(
        render(r"\color{accent}{x}", &font, &[]).err(),
        Some(Error::UnknownColor("accent".into()))
    );
}

#[test]
fn phantom_takes_space_but_draws_nothing() {
    let bytes = stix();
    let font = Font::parse(&bytes).unwrap();
    let visible = render("xy", &font, &[]).unwrap();
    let phantom = render(r"\phantom{x}y", &font, &[]).unwrap();
    assert_eq!(phantom.glyphs.len(), 1, "the phantom must not be drawn");
    assert_eq!(
        phantom.width, visible.width,
        "but it must take the same space"
    );
    assert_eq!(phantom.glyphs[0].x, visible.glyphs[1].x);
    // A fraction bar inside a phantom is invisible too.
    let bar = render(r"\phantom{\frac{a}{b}}", &font, &[]).unwrap();
    assert!(bar.glyphs.is_empty() && bar.rules.is_empty());
}

#[test]
fn nested_scopes_innermost_wins_and_rules_are_coloured() {
    let bytes = stix();
    let font = Font::parse(&bytes).unwrap();
    let tree = render(
        r"\color{accent}{\frac{a}{\color{muted}{b}}}",
        &font,
        &["accent", "muted"],
    )
    .unwrap();
    let accent = tree
        .paints
        .iter()
        .position(|p| *p == Paint::Named("accent".into()))
        .unwrap();
    let muted = tree
        .paints
        .iter()
        .position(|p| *p == Paint::Named("muted".into()))
        .unwrap();
    assert_eq!(tree.glyphs[0].paint, Some(accent)); // a
    assert_eq!(tree.glyphs[1].paint, Some(muted)); // b
    assert_eq!(tree.rules[0].paint, Some(accent)); // the bar
}
