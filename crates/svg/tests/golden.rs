//! Golden-file tests: every corpus formula × every test font → SVG, byte-compared with
//! `tests/golden/<font>/<name>.svg`.
//!
//! On mismatch, expected/actual/diff PNGs are rasterised with resvg into
//! `target/visual-diff/<font>/` so the regression can be seen, and the test fails.
//! Run with `UPDATE_GOLDEN=1` to (re)write the golden files.

#[path = "../../../tests/corpus/mod.rs"]
mod corpus;

use std::fs;
use std::path::PathBuf;

use latex_math_core::{Font, FontSet, Options};
use latex_math_svg::{to_svg, SvgOptions};

fn render(font: &Font<'_>, tex: &str) -> String {
    let tree = latex_math_core::render(tex, &FontSet::single(font), &Options::default()).unwrap();
    to_svg(&tree, &[font], &SvgOptions::default()).unwrap()
}

fn rasterize(svg: &str) -> resvg::tiny_skia::Pixmap {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
    let scale = 4.0f32;
    let size = tree.size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(
        (size.width() * scale).ceil() as u32 + 2,
        (size.height() * scale).ceil() as u32 + 2,
    )
    .unwrap();
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(1.0, 1.0),
        &mut pixmap.as_mut(),
    );
    pixmap
}

fn write_visual_diff(dir: &PathBuf, name: &str, expected: &str, actual: &str) {
    fs::create_dir_all(dir).unwrap();
    let a = rasterize(expected);
    let b = rasterize(actual);
    a.save_png(dir.join(format!("{name}-expected.png")))
        .unwrap();
    b.save_png(dir.join(format!("{name}-actual.png"))).unwrap();
    let w = a.width().max(b.width());
    let h = a.height().max(b.height());
    let mut diff = resvg::tiny_skia::Pixmap::new(w, h).unwrap();
    diff.fill(resvg::tiny_skia::Color::WHITE);
    let px = |p: &resvg::tiny_skia::Pixmap, x: u32, y: u32| -> u8 {
        if x < p.width() && y < p.height() {
            p.pixel(x, y).map(|c| c.red()).unwrap_or(255)
        } else {
            255
        }
    };
    let data = diff.data_mut();
    for y in 0..h {
        for x in 0..w {
            let (ea, ab) = (px(&a, x, y), px(&b, x, y));
            let i = ((y * w + x) * 4) as usize;
            let (r, g, b) = match ea.cmp(&ab) {
                std::cmp::Ordering::Equal => (ea, ea, ea),
                std::cmp::Ordering::Less => (255, 0, 0), // present in expected, missing in actual
                std::cmp::Ordering::Greater => (0, 0, 255), // new in actual
            };
            data[i] = r;
            data[i + 1] = g;
            data[i + 2] = b;
            data[i + 3] = 255;
        }
    }
    diff.save_png(dir.join(format!("{name}-diff.png"))).unwrap();
}

#[test]
fn golden_svg_corpus() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let root = corpus::repo_root();
    let mut failures = Vec::new();
    for (font_name, font_file) in corpus::FONTS {
        let bytes = fs::read(corpus::font_path(font_file)).unwrap();
        let font = Font::parse(&bytes).unwrap();
        let golden_dir = root.join("tests/golden").join(font_name);
        fs::create_dir_all(&golden_dir).unwrap();
        for (name, tex) in corpus::CORPUS {
            let actual = render(&font, tex);
            let path = golden_dir.join(format!("{name}.svg"));
            if update {
                fs::write(&path, &actual).unwrap();
                continue;
            }
            match fs::read_to_string(&path) {
                Ok(expected) if expected == actual => {}
                Ok(expected) => {
                    let diff_dir = root.join("target/visual-diff").join(font_name);
                    write_visual_diff(&diff_dir, name, &expected, &actual);
                    failures.push(format!(
                        "{font_name}/{name}: differs (see {})",
                        diff_dir.display()
                    ));
                }
                Err(_) => failures.push(format!(
                    "{font_name}/{name}: no golden file (run with UPDATE_GOLDEN=1)"
                )),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "golden mismatches:\n{}",
        failures.join("\n")
    );
}

#[test]
fn output_is_deterministic() {
    let bytes = fs::read(corpus::font_path("STIXTwoMath-Regular.otf")).unwrap();
    let font = Font::parse(&bytes).unwrap();
    for (_, tex) in corpus::CORPUS {
        let a = render(&font, tex);
        let b = render(&font, tex);
        assert_eq!(a, b);
    }
}

#[test]
fn every_corpus_svg_rasterizes_and_is_not_blank() {
    let bytes = fs::read(corpus::font_path("latinmodern-math.otf")).unwrap();
    let font = Font::parse(&bytes).unwrap();
    for (name, tex) in corpus::CORPUS {
        let svg = render(&font, tex);
        let pm = rasterize(&svg);
        let dark = pm.pixels().iter().filter(|p| p.red() < 128).count();
        assert!(dark > 100, "{name}: rasterized to a blank image");
    }
}
