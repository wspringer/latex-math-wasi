//! This is a collection of tools used for converting ParseNodes into LayoutNodes.

use crate::dimensions::units::{Em, FUnit, Px};
use crate::dimensions::{AnyUnit, Unit};
use crate::font::{Direction, Glyph, MathFont, VariantGlyph};

use super::builders;
use super::engine::{LayoutContext, LayoutEngine};
use super::Style;
use super::{LayoutGlyph, LayoutNode, LayoutVariant};
use crate::error::LayoutResult;
use crate::parser::nodes::Rule;

pub trait AsLayoutNode<'f, F> {
    fn as_layout(
        &self,
        engine: &LayoutEngine<'f, F>,
        context: LayoutContext,
    ) -> LayoutResult<LayoutNode<'f, F>>;
}

impl<'f, F> AsLayoutNode<'f, F> for Glyph<'f, F> {
    fn as_layout(
        &self,
        engine: &LayoutEngine<'f, F>,
        context: LayoutContext,
    ) -> LayoutResult<LayoutNode<'f, F>> {
        Ok(LayoutNode {
            height: self.height().to_px(engine, context),
            width: self.advance.to_px(engine, context),
            depth: self.depth().to_px(engine, context),
            node: LayoutVariant::Glyph(LayoutGlyph {
                font: self.font,
                gid: self.gid,
                size: Unit::<Em>::new(1.0).to_px(engine, context),
                attachment: self.attachment.to_px(engine, context),
                italics: self.italics.to_px(engine, context),
                offset: Unit::ZERO,
            }),
        })
    }
}

impl<'f, F> AsLayoutNode<'f, F> for Rule {
    fn as_layout(
        &self,
        engine: &LayoutEngine<'f, F>,
        context: LayoutContext,
    ) -> LayoutResult<LayoutNode<'f, F>> {
        Ok(LayoutNode {
            node: LayoutVariant::Rule,
            width: self.width.to_px(engine, context),
            height: self.height.to_px(engine, context),
            depth: Unit::ZERO,
        })
    }
}

impl<'f, F: MathFont> AsLayoutNode<'f, F> for VariantGlyph {
    fn as_layout(
        &self,
        engine: &LayoutEngine<'f, F>,
        context: LayoutContext,
    ) -> LayoutResult<LayoutNode<'f, F>> {
        match *self {
            VariantGlyph::Replacement(gid) => {
                let glyph = engine.font_at(context.style).glyph_from_gid(gid)?;
                glyph.as_layout(engine, context)
            }

            VariantGlyph::Constructable(dir, ref parts) => match dir {
                Direction::Vertical => {
                    let mut contents = builders::VBox::new();
                    for instr in parts.iter().rev() {
                        let glyph = engine.font_at(context.style).glyph_from_gid(instr.gid)?;
                        contents.add_node(glyph.as_layout(engine, context)?);
                        if instr.overlap != 0 {
                            let overlap = Unit::<FUnit>::new(instr.overlap.into());
                            let kern = -(overlap + glyph.depth()).to_px(engine, context);
                            contents.add_node(LayoutNode::vert_kern(kern))
                        }
                    }

                    Ok(contents.build())
                }

                Direction::Horizontal => {
                    let mut contents = builders::HBox::new();
                    for instr in parts {
                        let glyph = engine.font_at(context.style).glyph_from_gid(instr.gid)?;
                        if instr.overlap != 0 {
                            let kern =
                                -Unit::<FUnit>::new(instr.overlap.into()).to_px(engine, context);
                            contents.add_node(LayoutNode::horiz_kern(kern));
                        }
                        contents.add_node(glyph.as_layout(engine, context)?);
                    }

                    Ok(contents.build())
                }
            },
        }
    }
}

impl<F> LayoutEngine<'_, F> {
    fn scale_factor(&self, style: Style) -> f64 {
        self.scale_at(style)
    }

    /// Font units of the level's font → user units, at the level's font size and scale.
    fn scale_font_unit(&self, length: Unit<FUnit>, context: LayoutContext) -> Unit<Px> {
        length * (context.font_size / self.metrics_at(context.style).units_per_em()).unlift()
    }

    /// User units → font units of the level's font, such that a glyph of that many font
    /// units, laid out at this level (font size × level scale), spans `length`.
    pub(super) fn to_font(&self, length: Unit<Px>, context: LayoutContext) -> Unit<FUnit> {
        let font_size = context.font_size.scale(self.scale_at(context.style));
        length * (self.metrics_at(context.style).units_per_em() / font_size).unlift()
    }
}
pub trait ToPx {
    fn to_px<F>(self, engine: &LayoutEngine<F>, context: LayoutContext) -> Unit<Px>;
}

impl ToPx for Unit<FUnit> {
    fn to_px<F>(self, engine: &LayoutEngine<F>, context: LayoutContext) -> Unit<Px> {
        engine
            .scale_font_unit(self, context)
            .scale(engine.scale_factor(context.style))
    }
}

impl ToPx for Unit<Px> {
    fn to_px<F>(self, engine: &LayoutEngine<F>, context: LayoutContext) -> Unit<Px> {
        self.scale(engine.scale_factor(context.style))
    }
}
impl ToPx for Unit<Em> {
    fn to_px<F>(self, engine: &LayoutEngine<F>, context: LayoutContext) -> Unit<Px> {
        (self * context.font_size).scale(engine.scale_factor(context.style))
    }
}
impl ToPx for AnyUnit {
    fn to_px<F>(self, engine: &LayoutEngine<F>, context: LayoutContext) -> Unit<Px> {
        let length = match self {
            AnyUnit::Em(em) => Unit::<Em>::new(em) * context.font_size,
            AnyUnit::Px(px) => Unit::<Px>::new(px),
        };
        length.scale(engine.scale_factor(context.style))
    }
}
