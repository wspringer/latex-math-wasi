//! The image box and metrics: what a consumer needs to place output inline.

use latex_math_core::{metrics, Font, FontSet, Options, RenderTree};

fn stix() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fonts/STIXTwoMath-Regular.otf"
    ))
    .unwrap()
}

fn render(tex: &str, font: &Font<'_>) -> RenderTree {
    latex_math_core::render(tex, &FontSet::single(font), &Options::default()).unwrap()
}

#[test]
fn image_box_is_bbox_plus_padding_with_baseline_inside() {
    let bytes = stix();
    let font = Font::parse(&bytes).unwrap();
    let tree = render(r"\frac{a}{b}", &font);
    let b = tree.image_box(2.0);
    assert_eq!(b.x, tree.bbox.x_min - 2.0);
    assert_eq!(b.y, tree.bbox.y_min - 2.0);
    assert_eq!(b.width, tree.bbox.width() + 4.0);
    assert_eq!(b.height, tree.bbox.height() + 4.0);
    // The baseline is y = 0; the bottom edge is bbox.y_max + padding below it.
    assert_eq!(b.depth, tree.bbox.y_max + 2.0);
    assert!((b.ascent() + b.depth - b.height).abs() < 1e-12);
    // A fraction hangs below the baseline and rises above it.
    assert!(b.depth > 2.0 && b.ascent() > 2.0);
}

#[test]
fn a_lone_x_sits_on_the_baseline() {
    let bytes = stix();
    let font = Font::parse(&bytes).unwrap();
    let m = metrics(
        &render("x", &font),
        &FontSet::single(&font),
        &Options::default(),
        0.0,
    );
    // x has no descender: the bottom of the image is (within overshoot) the baseline.
    assert!(m.depth.abs() < 0.5, "depth {}", m.depth);
    // ... and its top is about one x-height up.
    let ex = m.ex.expect("STIX Two Math records an x-height");
    assert!(
        (m.ascent - ex).abs() < 1.0,
        "ascent {} vs ex {ex}",
        m.ascent
    );
    assert_eq!(m.em, 16.0);
}

#[test]
fn metrics_json_is_fixed_precision_and_ordered() {
    let bytes = stix();
    let font = Font::parse(&bytes).unwrap();
    let m = metrics(
        &render("y", &font),
        &FontSet::single(&font),
        &Options::default(),
        1.0,
    );
    let json = m.to_json();
    let keys: Vec<&str> = json
        .split('"')
        .filter(|s| s.chars().all(|c| c.is_ascii_lowercase()) && !s.is_empty())
        .collect();
    assert_eq!(keys, ["width", "height", "depth", "ascent", "em", "ex"]);
    assert!(json.contains(r#""em":16.000"#), "{json}");
    assert!(
        json.contains(&format!(r#""depth":{:.3}"#, m.depth)),
        "{json}"
    );
}

#[test]
fn ex_comes_from_the_text_font_of_the_set() {
    let stix_bytes = stix();
    let stix_font = Font::parse(&stix_bytes).unwrap();
    let xits_bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fonts/XITSMath-Regular.otf"
    ))
    .unwrap();
    let xits_font = Font::parse(&xits_bytes).unwrap();
    let fonts = [stix_font, xits_font];
    // display = STIX, text = XITS: `ex` must be XITS's x-height.
    let set = FontSet::new(&fonts, [0, 1, 1, 1]).unwrap();
    let tree = latex_math_core::render("x", &set, &Options::default()).unwrap();
    let m = metrics(&tree, &set, &Options::default(), 0.0);
    assert_eq!(m.ex, fonts[1].x_height_em().map(|x| x * 16.0));
    assert_ne!(fonts[0].x_height_em(), fonts[1].x_height_em());
}
