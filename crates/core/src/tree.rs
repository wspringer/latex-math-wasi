//! The flat render tree: positioned glyphs and rules, nothing else.

use crate::font::common::GlyphId;
use crate::font::MathFont;
use crate::render::{Backend, Cursor, FontBackend, GraphicsBackend, Paint, RGBA};

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
    /// Index into [`RenderTree::paints`], or `None` for the document colour.
    pub paint: Option<usize>,
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
    /// Index into [`RenderTree::paints`], or `None` for the document colour.
    pub paint: Option<usize>,
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
    /// The colours `\color` scopes resolved to, in first-use order. `Named` entries are
    /// palette names for the backend to look up; `Rgba` entries are literals or CSS names.
    pub paints: Vec<Paint>,
}

/// A [`Backend`] that records glyphs and rules into a [`RenderTree`].
pub(crate) struct TreeBackend<'a, F> {
    fonts: Vec<&'a F>,
    tree: RenderTree,
    palette: &'a [String],
    /// Open colour scopes, innermost last: the paint index, or `None` while invisible.
    stack: Vec<Scope>,
    /// Colour names that are neither in the palette nor CSS names.
    unknown: Vec<String>,
}

#[derive(Clone, Copy)]
enum Scope {
    Paint(usize),
    Invisible,
}

impl<'a, F> TreeBackend<'a, F> {
    pub(crate) fn new(fonts: Vec<&'a F>, palette: &'a [String]) -> Self {
        TreeBackend {
            fonts,
            tree: RenderTree::default(),
            palette,
            stack: Vec::new(),
            unknown: Vec::new(),
        }
    }

    pub(crate) fn finish(self) -> Result<RenderTree, String> {
        match self.unknown.into_iter().next() {
            Some(name) => Err(name),
            None => Ok(self.tree),
        }
    }

    fn current(&self) -> Option<Scope> {
        self.stack.last().copied()
    }

    fn paint_index(&mut self, paint: Paint) -> usize {
        match self.tree.paints.iter().position(|p| *p == paint) {
            Some(i) => i,
            None => {
                self.tree.paints.push(paint);
                self.tree.paints.len() - 1
            }
        }
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
        let paint = match self.current() {
            Some(Scope::Invisible) => return, // `\phantom`: takes space, draws nothing
            Some(Scope::Paint(i)) => Some(i),
            None => None,
        };
        let font = self.font_index(font);
        self.tree.glyphs.push(GlyphInstance {
            font,
            gid: gid.into(),
            x: pos.x,
            y: pos.y,
            size: scale,
            paint,
        });
    }
}

impl<F> GraphicsBackend for TreeBackend<'_, F> {
    fn rule(&mut self, pos: Cursor, width: f64, height: f64) {
        let paint = match self.current() {
            Some(Scope::Invisible) => return,
            Some(Scope::Paint(i)) => Some(i),
            None => None,
        };
        self.tree.rules.push(Rule {
            x: pos.x,
            y: pos.y,
            width,
            height,
            paint,
        });
    }

    /// Resolves the colour: palette names stay names (the backend decides what they
    /// are), anything else must be a CSS name and becomes a literal. A fully transparent
    /// literal opens an invisible scope.
    fn begin_color(&mut self, color: &Paint) {
        let resolved = match color {
            Paint::Named(name) if self.palette.iter().any(|p| p.as_str() == &**name) => {
                Paint::Named(name.clone())
            }
            Paint::Named(name) => match RGBA::from_name(name) {
                Some(rgba) => Paint::Rgba(rgba),
                None => {
                    self.unknown.push(name.to_string());
                    // Keep the scope balanced; the error surfaces in `finish`.
                    Paint::Rgba(RGBA(0, 0, 0, 0xff))
                }
            },
            Paint::Rgba(rgba) => Paint::Rgba(*rgba),
        };
        let scope = match resolved {
            Paint::Rgba(RGBA(_, _, _, 0)) => Scope::Invisible,
            paint => Scope::Paint(self.paint_index(paint)),
        };
        self.stack.push(scope);
    }

    fn end_color(&mut self) {
        self.stack.pop();
    }
}

impl<F: MathFont> Backend<F> for TreeBackend<'_, F> {}

/// The rectangle an output document covers — the tight [`BBox`] plus `padding` on every
/// side — and where the baseline sits inside it. User units; y grows downwards and the
/// baseline is `y = 0` in tree coordinates. Every backend (SVG, PDF, PNG) sizes its
/// document from this, so their boxes agree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageBox {
    /// Left edge, tree coordinates.
    pub x: f64,
    /// Top edge, tree coordinates.
    pub y: f64,
    /// Total width.
    pub width: f64,
    /// Total height.
    pub height: f64,
    /// Distance from the baseline down to the bottom edge.
    pub depth: f64,
}

impl ImageBox {
    /// Distance from the top edge down to the baseline.
    pub fn ascent(&self) -> f64 {
        self.height - self.depth
    }
}

impl RenderTree {
    /// The document rectangle for this tree with `padding` user units around the bbox.
    pub fn image_box(&self, padding: f64) -> ImageBox {
        ImageBox {
            x: self.bbox.x_min - padding,
            y: self.bbox.y_min - padding,
            width: self.bbox.width() + 2.0 * padding,
            height: self.bbox.height() + 2.0 * padding,
            depth: self.bbox.y_max + padding,
        }
    }
}
