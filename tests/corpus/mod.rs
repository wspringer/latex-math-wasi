//! The golden-file corpus: ReX's README samples (they cover fractions, radicals, nested
//! scripts, large operators with limits, and `\left…\right` delimiter growth), plus the
//! four formulas from ReX's TeX-comparison suite.
//!
//! Shared between test crates via `#[path]`.

pub const CORPUS: &[(&str, &str)] = &[
    (
        "quadratic_formula",
        r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
    ),
    (
        "double_angle_sine",
        r"\sin(\theta + \phi) = \sin(\theta)\cos(\phi) + \sin(\phi)\cos(\theta)",
    ),
    (
        "divergence_theorem",
        r"\int_D (\nabla \cdot F)\,\mathrm{d}V = \int_{\partial D} F \cdot n\,\mathrm{d}S",
    ),
    (
        "standard_deviation",
        r"\sigma = \sqrt{ \frac{1}{N} \sum_{i=1}^N (x_i - \mu)^2 }",
    ),
    (
        "fourier_inverse",
        r"f(x) = \int_{-\infty}^{\infty} \hat f(\xi) e^{2\pi i \xi x}\,\mathrm{d}\xi",
    ),
    (
        "cauchy_schwarz",
        r"\left\vert \sum_k a_kb_k \right\vert \leq \left(\sum_k a_k^2\right)^{\frac12}\left(\sum_k b_k^2\right)^{\frac12}",
    ),
    (
        "exponent",
        r"e = \lim_{n \to \infty} \left(1 + \frac{1}{n}\right)^n",
    ),
    (
        "ramanujan_identity",
        r"\frac{1}{\pi} = \frac{2\sqrt{2}}{9801} \sum_{k=0}^\infty \frac{ (4k)! (1103+26390k) }{ (k!)^4 396^{4k} }",
    ),
    (
        "surprising_identity",
        r"\int_{-\infty}^{\infty} \frac{\sin(x)}{x}\,\mathrm{d}x = \int_{-\infty}^{\infty}\frac{\sin^2(x)}{x^2}\,\mathrm{d}x",
    ),
    (
        "ramanujan_gem",
        r"\frac{1}{\left(\sqrt{\phi\sqrt5} - \phi\right) e^{\frac{2}{5}\pi}} = 1 + \frac{e^{-2\pi}}{1 + \frac{e^{-4\pi}}{1 + \frac{e^{-6\pi}}{1 + \frac{e^{-8\pi}}{1 + \cdots}}}}",
    ),
    (
        "cauchy_gem",
        r"f^{(n)}(z) = \frac{n!}{2\pi i} \oint \frac{f(\xi)}{(\xi - z)^{n+1}}\,\mathrm{d}\xi",
    ),
    ("many_scripts", r"x^{x^{x^x_x}_{x^x_x}}_{x^{x^x_x}_{x^x_x}}"),
    (
        "quartic",
        r"\mathop{\overbrace{c_4x^4 + c_3x^3 + c_2x^2 + c_1x + c_0}}\limits^{\gray{\mathrm{Quartic}}}",
    ),
    ("fun_identity", r"3^3 + 4^4 + 3^3 + 5^5 = 3435"),
    ("array", r"\begin{array}{r@{=}l}2 & 1\\ 3 & 4\end{array}"),
    (
        "iint_sqrt",
        r"\iint \sqrt{1 + f^2(x,t,t)}\,\mathrm{d}x\mathrm{d}y\mathrm{d}t = \sum \xi(t)",
    ),
    ("norm", r"\Vert f \Vert_2 = \sqrt{\int f^2(x)\,\mathrm{d}x}"),
    (
        "scripts_brace",
        r"\left.x^{x^{x^x_x}_{x^x_x}}_{x^{x^x_x}_{x^x_x}}\right\rbrace \mathrm{wat?}",
    ),
];

/// Free MATH fonts used by every committed test. Different MATH tables populate
/// different things, which is why all three are exercised.
pub const FONTS: &[(&str, &str)] = &[
    ("stix2", "STIXTwoMath-Regular.otf"),
    ("xits", "XITSMath-Regular.otf"),
    ("lm", "latinmodern-math.otf"),
];

pub fn font_path(file: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fonts")
        .join(file)
}

pub fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
