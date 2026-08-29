//! The `metrics` format agrees with the SVG header and with core.

#[path = "../../../tests/corpus/mod.rs"]
mod corpus;

use base64::Engine;
use latex_math_core::{Font, FontSet, Options};

fn request(tex: &str, format: &str, padding: f64) -> Vec<u8> {
    let font = std::fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&font);
    let json = format!(
        r#"{{"tex": {tex:?}, "format": "{format}", "padding": {padding}, "fonts": ["{b64}"]}}"#
    );
    latex_math_wasm::handle(json.as_bytes(), &[]).unwrap()
}

#[test]
fn metrics_match_core_and_svg_header() {
    let bytes = std::fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap();
    let font = Font::parse(&bytes).unwrap();
    for (_, tex) in corpus::CORPUS {
        let out = String::from_utf8(request(tex, "metrics", 2.0)).unwrap();
        let tree =
            latex_math_core::render(tex, &FontSet::single(&font), &Options::default()).unwrap();
        let expected =
            latex_math_core::metrics(&tree, &FontSet::single(&font), &Options::default(), 2.0);
        assert_eq!(out, expected.to_json(), "{tex}");

        // The SVG says the same thing in its header (SVG numbers have trailing zeros
        // trimmed, so compare numerically).
        let svg = String::from_utf8(request(tex, "svg", 2.0)).unwrap();
        let head = svg.lines().next().unwrap();
        let attr = |key: &str| -> f64 {
            let start = head.find(key).unwrap() + key.len();
            let end = head[start..].find(['"', 'p']).unwrap();
            head[start..start + end].parse().unwrap()
        };
        let close = |a: f64, b: f64| (a - b).abs() < 1e-3;
        assert!(close(attr(" width=\""), expected.width), "{tex}: {head}");
        assert!(close(attr(" height=\""), expected.height), "{tex}: {head}");
        assert!(
            close(attr("vertical-align:-"), expected.depth),
            "{tex}: {head}"
        );
    }
}
