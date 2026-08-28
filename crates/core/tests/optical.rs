//! Optical-size font sets: a level with its own font is laid out from that font at full
//! size, with the parent level's constants deciding where the child goes.

use latex_wasi_core::{Font, FontSet, Options, RenderTree};

fn font_bytes(file: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fonts")
            .join(file),
    )
    .unwrap()
}

fn render(tex: &str, set: &FontSet<'_, '_>) -> RenderTree {
    latex_wasi_core::render(tex, set, &Options::default()).unwrap()
}

fn constant(font: &Font<'_>, pick: impl Fn(&ttf_parser::math::Constants<'_>) -> i16) -> f64 {
    let face = font.font();
    f64::from(pick(&face.tables().math.unwrap().constants.unwrap()))
        / f64::from(face.units_per_em())
}

#[test]
fn single_font_scales_scripts_by_script_percent_scale_down() {
    let stix = font_bytes("STIXTwoMath-Regular.otf");
    let fonts = [Font::parse(&stix).unwrap()];
    let tree = render(r"x^{2^{3}}", &FontSet::new(&fonts, [0; 4]).unwrap());
    let sizes: Vec<f64> = tree.glyphs.iter().map(|g| g.size).collect();
    // STIX Two: ScriptPercentScaleDown 70, ScriptScriptPercentScaleDown 55.
    for (got, want) in sizes.iter().zip([16.0, 16.0 * 0.70, 16.0 * 0.55]) {
        assert!((got - want).abs() < 1e-9, "{sizes:?}");
    }
    assert!(tree.glyphs.iter().all(|g| g.font == 0));
}

#[test]
fn distinct_script_font_is_drawn_at_the_text_fonts_script_scale() {
    let stix = font_bytes("STIXTwoMath-Regular.otf");
    let xits = font_bytes("XITSMath-Regular.otf");
    let fonts = [Font::parse(&stix).unwrap(), Font::parse(&xits).unwrap()];
    // display+text from STIX, script+scriptscript from XITS
    let tree = render(r"x^{2^{3}}", &FontSet::new(&fonts, [0, 0, 1, 1]).unwrap());
    let (x, two, three) = (&tree.glyphs[0], &tree.glyphs[1], &tree.glyphs[2]);
    assert_eq!((x.font, x.size), (0, 16.0));
    // The script glyphs come from XITS, but an optical cut keeps its em, so the level's
    // scale still applies: STIX Two's ScriptPercentScaleDown (70), not XITS's (75).
    assert_eq!(two.font, 1);
    assert!((two.size - 16.0 * 0.70).abs() < 1e-9, "{}", two.size);
    assert_eq!(three.font, 1);
    assert!((three.size - 16.0 * 0.55).abs() < 1e-9, "{}", three.size);
}

#[test]
fn explicit_scales_override_the_math_table() {
    let stix = font_bytes("STIXTwoMath-Regular.otf");
    let xits = font_bytes("XITSMath-Regular.otf");
    let fonts = [Font::parse(&stix).unwrap(), Font::parse(&xits).unwrap()];
    let set = FontSet::new(&fonts, [0, 0, 1, 1])
        .unwrap()
        .with_scales([1.0, 1.0, 0.7, 0.5]);
    let tree = render(r"x^{2^{3}}", &set);
    let sizes: Vec<f64> = tree.glyphs.iter().map(|g| g.size).collect();
    for (got, want) in sizes.iter().zip([16.0, 11.2, 8.0]) {
        assert!((got - want).abs() < 1e-9, "{sizes:?}");
    }
}

#[test]
fn script_glyph_outline_is_the_other_fonts_outline_not_a_scaled_copy() {
    let stix = font_bytes("STIXTwoMath-Regular.otf");
    let xits = font_bytes("XITSMath-Regular.otf");
    let fonts = [Font::parse(&stix).unwrap(), Font::parse(&xits).unwrap()];
    let tree = render(r"x^{x}", &FontSet::new(&fonts, [0, 0, 1, 1]).unwrap());
    let base = tree.glyphs[0];
    let sup = tree.glyphs[1];
    assert_eq!(base.font, 0);
    assert_eq!(sup.font, 1);
    // The superscript's glyph id resolves in XITS to the math-italic x, and its outline
    // differs from STIX's: this is another design, not a scaled copy.
    let outline = |font: &Font<'_>, gid: u16| {
        struct P(Vec<(f32, f32)>);
        impl ttf_parser::OutlineBuilder for P {
            fn move_to(&mut self, x: f32, y: f32) {
                self.0.push((x, y));
            }
            fn line_to(&mut self, x: f32, y: f32) {
                self.0.push((x, y));
            }
            fn quad_to(&mut self, _: f32, _: f32, x: f32, y: f32) {
                self.0.push((x, y));
            }
            fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, x: f32, y: f32) {
                self.0.push((x, y));
            }
            fn close(&mut self) {}
        }
        let mut p = P(Vec::new());
        font.font().outline_glyph(ttf_parser::GlyphId(gid), &mut p);
        p.0
    };
    let xits_x = fonts[1].font().glyph_index('\u{1D465}').unwrap().0;
    assert_eq!(sup.gid, xits_x);
    assert_ne!(outline(&fonts[1], sup.gid), outline(&fonts[0], base.gid));
    // The advance the layout used is XITS's, at full size.
    let xits_adv = f64::from(
        fonts[1]
            .font()
            .glyph_hor_advance(ttf_parser::GlyphId(sup.gid))
            .unwrap(),
    );
    let upem = f64::from(fonts[1].font().units_per_em());
    let expected_width = xits_adv / upem * 16.0 * 0.70; // STIX Two script scale
    assert!(
        (tree.width - (sup.x + expected_width)).abs() < 1e-9,
        "width {} sup.x {} size {} expected advance {}",
        tree.width,
        sup.x,
        sup.size,
        expected_width
    );
}

#[test]
fn superscript_placement_comes_from_the_parent_font() {
    let stix = font_bytes("STIXTwoMath-Regular.otf");
    let xits = font_bytes("XITSMath-Regular.otf");
    let fonts = [
        Font::parse(&stix).unwrap(),
        Font::parse(&xits).unwrap(),
        Font::parse(&stix).unwrap(),
    ];
    // Parent STIX, script XITS.
    let a = render(r"x^{1}", &FontSet::new(&fonts, [0, 0, 1, 1]).unwrap());
    // Parent STIX, script a second STIX instance (same glyphs via another font object).
    let b = render(r"x^{1}", &FontSet::new(&fonts, [0, 0, 2, 2]).unwrap());
    // Parent XITS, script STIX.
    let c = render(r"x^{1}", &FontSet::new(&fonts, [1, 1, 0, 0]).unwrap());
    let sup_y = |t: &RenderTree| t.glyphs[1].y;
    // The superscript "1" has no depth in either font, so the shift is decided entirely
    // by the parent's constants (SuperscriptShiftUp, SuperscriptBaselineDropMax,
    // SuperscriptBottomMin) and the base glyph — the script font must not matter.
    assert!(
        (sup_y(&a) - sup_y(&b)).abs() < 1e-9,
        "{} vs {}",
        sup_y(&a),
        sup_y(&b)
    );
    assert!(
        (sup_y(&a) - sup_y(&c)).abs() > 0.1,
        "different parents must place differently"
    );
    // And it is STIX's SuperscriptShiftUp that wins here.
    let expected = -constant(&fonts[0], |c| c.superscript_shift_up().value) * 16.0;
    assert!(
        (sup_y(&a) - expected).abs() < 1e-9,
        "{} vs {}",
        sup_y(&a),
        expected
    );
}

#[test]
fn fraction_in_a_script_uses_the_script_fonts_axis() {
    // A fraction laid out at script level takes AxisHeight and FractionRuleThickness
    // from the script font (Latin Modern: axis 250, rule 40) even though the parent is STIX.
    let stix = font_bytes("STIXTwoMath-Regular.otf");
    let lm = font_bytes("latinmodern-math.otf");
    let fonts = [Font::parse(&stix).unwrap(), Font::parse(&lm).unwrap()];
    let tree = render(
        r"x^{\frac{a}{b}}",
        &FontSet::new(&fonts, [0, 0, 1, 1]).unwrap(),
    );
    let rule = tree.rules[0];
    // Latin Modern's rule thickness, at the script scale of the text font (STIX Two: 70%).
    let thickness = constant(&fonts[1], |c| c.fraction_rule_thickness().value) * 16.0 * 0.70;
    assert!(
        (rule.height - thickness).abs() < 1e-9,
        "{} vs {}",
        rule.height,
        thickness
    );
    // Same formula with the parent's font at script level yields STIX's thickness instead.
    let tree2 = render(r"x^{\frac{a}{b}}", &FontSet::new(&fonts, [0; 4]).unwrap());
    let stix_thickness = constant(&fonts[0], |c| c.fraction_rule_thickness().value) * 16.0 * 0.70;
    assert!((tree2.rules[0].height - stix_thickness).abs() < 1e-9);
}

#[test]
fn invalid_font_sets_are_rejected() {
    let stix = font_bytes("STIXTwoMath-Regular.otf");
    let fonts = [Font::parse(&stix).unwrap()];
    assert!(FontSet::new(&fonts, [0, 0, 0, 1]).is_err());
    assert!(FontSet::new(&[], [0; 4]).is_err());
}
