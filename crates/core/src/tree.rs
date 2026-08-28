//! The flat render tree: positioned glyphs and rules, nothing else.

use crate::font::common::GlyphId;
use crate::font::MathFont;
use crate::render::{Backend, Cursor, FontBackend, GraphicsBackend, RGBA};

/// One glyph to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphInstance {
    /// Index into the font list the tree was rendered with.
    pub font: usize,
    /// Glyph id in that font.
    pub gid: u16,
    /// Baseline origin, user units, y down.
    pub x: f64,
    /// Baseline origin, user units, y down.
    pub y: f64,
    /// Em size in user units at which to draw the glyph (font size × any style scaling).
    pub size: f64,
}

/// A filled rectangle (fraction bars, radical rules, `\rule`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rule {
    /// Left edge.
    pub x: f64,
    /// Top edge (y down).
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// Bounding box in user units, y down.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BBox {
    /// Left.
    pub x_min: f64,
    /// Top.
    pub y_min: f64,
    /// Right.
    pub x_max: f64,
    /// Bottom.
    pub y_max: f64,
}

impl BBox {
    /// Width.
    pub fn width(&self) -> f64 {
        self.x_max - self.x_min
    }
    /// Height.
    pub fn height(&self) -> f64 {
        self.y_max - self.y_min
    }
}

/// The output of layout.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderTree {
    /// Glyphs in drawing order.
    pub glyphs: Vec<GlyphInstance>,
    /// Rules in drawing order.
    pub rules: Vec<Rule>,
    /// Advance width of the formula box.
    pub width: f64,
    /// Distance from baseline to the top of the formula box (positive up).
    pub height: f64,
    /// Distance from baseline to the bottom of the formula box (negative when below baseline).
    pub depth: f64,
    /// Tight box around every drawn outline and rule, unioned with the formula box.
    pub bbox: BBox,
}

/// A [`Backend`] that records glyphs and rules into a [`RenderTree`].
pub(crate) struct TreeBackend<'a, F> {
    fonts: Vec<&'a F>,
    tree: RenderTree,
}

impl<'a, F> TreeBackend<'a, F> {
    pub(crate) fn new(font: &'a F) -> Self {
        TreeBackend {
            fonts: vec![font],
            tree: RenderTree::default(),
        }
    }

    pub(crate) fn finish(self) -> RenderTree {
        self.tree
    }

    fn font_index(&mut self, font: &F) -> usize {
        match self.fonts.iter().position(|f| std::ptr::eq(*f, font)) {
            Some(i) => i,
            None => unreachable!("glyph from a font that was not passed to layout"),
        }
    }
}

impl<F: MathFont> FontBackend<F> for TreeBackend<'_, F> {
    fn symbol(&mut self, pos: Cursor, gid: GlyphId, scale: f64, font: &F) {
        let font = self.font_index(font);
        self.tree.glyphs.push(GlyphInstance {
            font,
            gid: gid.into(),
            x: pos.x,
            y: pos.y,
            size: scale,
        });
    }
}

impl<F> GraphicsBackend for TreeBackend<'_, F> {
    fn rule(&mut self, pos: Cursor, width: f64, height: f64) {
        self.tree.rules.push(Rule {
            x: pos.x,
            y: pos.y,
            width,
            height,
        });
    }
    // Colour is deliberately not part of the render tree (see NOTES.md, M1).
    fn begin_color(&mut self, _color: RGBA) {}
    fn end_color(&mut self) {}
}

impl<F: MathFont> Backend<F> for TreeBackend<'_, F> {}
