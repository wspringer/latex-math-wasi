//! LaTeX math → positioned render tree, using OpenType MATH fonts.
//!
//! The parser and layout engine are derived from [KenyC/ReX](https://github.com/KenyC/ReX)
//! (MIT, see `LICENSE-ReX`), itself a fork of ReTeX/ReX. This crate strips ReX down to
//! parsing + layout, takes fonts as bytes, and flattens the layout into a [`RenderTree`]
//! of glyph instances and rules that the `svg` and `pdf` crates consume.
//!
//! No `std::fs`, no font discovery, no network.

#[macro_use]
mod macros;

pub mod dimensions;
pub mod error;
pub mod font;
mod geometry;
pub mod layout;
pub mod parser;
pub mod render;
pub mod tree;

pub use error::Error;
pub use font::backend::ttf_parser::TtfMathFont as Font;
pub use layout::Style;
pub use tree::{GlyphInstance, RenderTree, Rule};

use layout::engine::LayoutBuilder;
use render::Renderer;

/// Layout options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Em size in user units. Every coordinate in the render tree is in these units.
    pub font_size: f64,
    /// Starting math style (`Display` for `$$…$$`, `Text` for `$…$`).
    pub style: Style,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            font_size: 16.0,
            style: Style::Display,
        }
    }
}

/// Parses `tex` and lays it out with `font`, returning a flat render tree.
///
/// The tree's origin is the baseline start of the formula; y grows downward.
pub fn render(tex: &str, font: &Font<'_>, options: &Options) -> Result<RenderTree, Error> {
    let nodes = parser::parse(tex)?;
    let layout = LayoutBuilder::new(font)
        .font_size(options.font_size)
        .style(options.style)
        .layout(&nodes)?;
    let size = layout.size();
    let bbox = layout.full_bounding_box();
    let mut backend = tree::TreeBackend::new(font);
    Renderer::new().render(&layout, &mut backend);
    let mut tree = backend.finish();
    tree.width = size.width;
    tree.height = size.height;
    tree.depth = size.depth;
    tree.bbox = tree::BBox {
        x_min: bbox.x_min,
        y_min: bbox.y_min,
        x_max: bbox.x_max,
        y_max: bbox.y_max,
    };
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STIX: &[u8] = include_bytes!("../../../tests/fonts/STIXTwoMath-Regular.otf");

    /// If the font's coverage of mathematical alphanumeric characters is exhaustive in all styles,
    /// then the library should not fail parsing and laying out on any of these.
    /// Test for bugs like <https://github.com/KenyC/ReX/issues/6>.
    #[test]
    fn all_alphanumeric_style_combinations_must_work() {
        let font = Font::parse(STIX).unwrap();
        let alphanumeric: Vec<char> = (0..0x7F_u32)
            .filter_map(char::from_u32)
            .filter(|c| c.is_alphanumeric())
            .collect();
        for env in [
            None,
            Some("mathcal"),
            Some("mathrm"),
            Some("mathfrak"),
            Some("mathbb"),
        ] {
            for character in &alphanumeric {
                let formula = match env {
                    Some(env) => format!(r"\{env}{{{character}}}"),
                    None => character.to_string(),
                };
                render(&formula, &font, &Options::default())
                    .unwrap_or_else(|e| panic!("{formula}: {e:?}"));
            }
        }
    }

    #[test]
    fn simple_formula_produces_glyphs_and_a_rule() {
        let font = Font::parse(STIX).unwrap();
        let tree = render(r"\frac{a}{b}", &font, &Options::default()).unwrap();
        assert_eq!(tree.glyphs.len(), 2);
        assert_eq!(tree.rules.len(), 1);
        assert!(tree.glyphs.iter().all(|g| g.font == 0));
        // numerator above the baseline, denominator below
        assert!(tree.glyphs[0].y < 0.0 && tree.glyphs[1].y > 0.0);
    }
}
