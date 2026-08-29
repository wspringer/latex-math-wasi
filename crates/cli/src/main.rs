//! `latex-math`: LaTeX math in, SVG (or PDF) out.

use std::io::{Read, Write};
use std::process::ExitCode;

use latex_math_core::{Font, FontSet, Options, Style};
use latex_math_pdf::{to_pdf, Color, PdfOptions};
use latex_math_png::{to_png, PngOptions};
use latex_math_svg::{to_svg, SvgOptions};

const USAGE: &str = "\
usage: latex-math --font FILE [--font FILE ...] [options] [FORMULA]

Renders a LaTeX math fragment. Reads FORMULA from stdin when not given.

options:
  --font FILE        OpenType font with a MATH table. Repeatable: one font is used at
                     every math level; two are [display+text, script+scriptscript];
                     three are [display+text, script, scriptscript]; four are
                     [display, text, script, scriptscript].
  --levels D,T,S,SS  explicit font index (0-based, into the --font list) per level
  --format FMT       svg, pdf, png, or metrics (JSON: width, height, depth, ascent,
                     em, ex in user units; default svg)
  --size N           em size in user units (default 16)
  --style display|text
  --padding N        space around the formula, user units (default 0)
  --scale N          png only: device pixels per user unit (default 1)
  --color SPEC       fill colour. pdf: gray:K | rgb:R,G,B | cmyk:C,M,Y,K |
                     spot:NAME:TINT:C,M,Y,K (components 0-1; default cmyk:0,0,0,1).
                     svg/png: gray:K, rgb:R,G,B or #rrggbb (default #000)
  -o, --output FILE  write here instead of stdout
  -h, --help
";

struct Args {
    fonts: Vec<String>,
    format: String,
    size: f64,
    style: Style,
    padding: f64,
    scale: f64,
    color: Option<String>,
    output: Option<String>,
    formula: Option<String>,
    levels: Option<[usize; 4]>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        fonts: Vec::new(),
        format: "svg".into(),
        size: 16.0,
        style: Style::Display,
        padding: 0.0,
        scale: 1.0,
        color: None,
        output: None,
        formula: None,
        levels: None,
    };
    let mut it = std::env::args().skip(1);
    let value = |flag: &str, it: &mut dyn Iterator<Item = String>| {
        it.next().ok_or_else(|| format!("{flag} needs a value"))
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--font" => args.fonts.push(value("--font", &mut it)?),
            "--format" => args.format = value("--format", &mut it)?,
            "--size" => {
                args.size = value("--size", &mut it)?
                    .parse()
                    .map_err(|_| "--size must be a number".to_string())?
            }
            "--padding" => {
                args.padding = value("--padding", &mut it)?
                    .parse()
                    .map_err(|_| "--padding must be a number".to_string())?
            }
            "--color" => args.color = Some(value("--color", &mut it)?),
            "--scale" => {
                args.scale = value("--scale", &mut it)?
                    .parse()
                    .map_err(|_| "--scale must be a number".to_string())?
            }
            "--style" => {
                args.style = match value("--style", &mut it)?.as_str() {
                    "display" => Style::Display,
                    "text" => Style::Text,
                    other => return Err(format!("unknown style {other}")),
                }
            }
            "--levels" => {
                let v = value("--levels", &mut it)?;
                let parts: Vec<usize> = v
                    .split(',')
                    .map(|p| {
                        p.trim()
                            .parse()
                            .map_err(|_| format!("--levels: bad index {p:?}"))
                    })
                    .collect::<Result<_, _>>()?;
                let arr: [usize; 4] = parts
                    .try_into()
                    .map_err(|_| "--levels needs exactly four indices".to_string())?;
                args.levels = Some(arr);
            }
            "-o" | "--output" => args.output = Some(value("-o", &mut it)?),
            "-h" | "--help" => return Err(String::new()),
            s if s.starts_with('-') && s.len() > 1 => return Err(format!("unknown option {s}")),
            _ => {
                if args.formula.is_some() {
                    return Err("only one formula".into());
                }
                args.formula = Some(a);
            }
        }
    }
    if args.fonts.is_empty() {
        return Err("at least one --font is required".into());
    }
    Ok(args)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let formula = match args.formula {
        Some(f) => f,
        None => {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| e.to_string())?;
            s
        }
    };
    let formula = formula.trim();

    let font_bytes: Vec<Vec<u8>> = args
        .fonts
        .iter()
        .map(|p| std::fs::read(p).map_err(|e| format!("{p}: {e}")))
        .collect::<Result<_, _>>()?;
    let fonts: Vec<Font<'_>> = font_bytes
        .iter()
        .zip(&args.fonts)
        .map(|(b, p)| Font::parse(b).map_err(|e| format!("{p}: {e}")))
        .collect::<Result<_, _>>()?;

    let options = Options {
        font_size: args.size,
        style: args.style,
    };
    let levels = args.levels.unwrap_or(match fonts.len() {
        1 => [0, 0, 0, 0],
        2 => [0, 0, 1, 1],
        3 => [0, 0, 1, 2],
        _ => [0, 1, 2, 3],
    });
    let set = FontSet::new(&fonts, levels).map_err(|e| format!("{e:?}"))?;
    let tree = latex_math_core::render(formula, &set, &options).map_err(|e| format!("{e:?}"))?;

    let refs: Vec<&Font<'_>> = fonts.iter().collect();
    let color = args.color.as_deref().map(parse_color).transpose()?;
    let bytes: Vec<u8> = match args.format.as_str() {
        "svg" => {
            let svg_options = SvgOptions {
                padding: args.padding,
                fill: svg_fill(color.as_ref())?,
                ..SvgOptions::default()
            };
            to_svg(&tree, &refs, &svg_options)
                .map_err(|e| e.to_string())?
                .into_bytes()
        }
        "pdf" => {
            let pdf_options = PdfOptions {
                padding: args.padding,
                color: color.clone().unwrap_or_default(),
            };
            to_pdf(&tree, &refs, &pdf_options).map_err(|e| e.to_string())?
        }
        "png" => {
            let svg_options = SvgOptions {
                padding: args.padding,
                fill: svg_fill(color.as_ref())?,
                ..SvgOptions::default()
            };
            let png_options = PngOptions {
                scale: args.scale,
                ..PngOptions::default()
            };
            to_png(&tree, &refs, &svg_options, &png_options).map_err(|e| e.to_string())?
        }
        "metrics" => latex_math_core::metrics(&tree, &set, &options, args.padding)
            .to_json()
            .into_bytes(),
        other => return Err(format!("unknown format {other}")),
    };

    match args.output {
        Some(path) => std::fs::write(&path, bytes).map_err(|e| format!("{path}: {e}")),
        None => std::io::stdout()
            .write_all(&bytes)
            .map_err(|e| e.to_string()),
    }
}

/// `gray:K`, `rgb:R,G,B`, `cmyk:C,M,Y,K`, `spot:NAME:TINT:C,M,Y,K`, or `#rrggbb`.
fn parse_color(spec: &str) -> Result<Color, String> {
    let nums = |s: &str, n: usize| -> Result<Vec<f64>, String> {
        let v: Vec<f64> = s
            .split(',')
            .map(|p| p.trim().parse::<f64>())
            .collect::<Result<_, _>>()
            .map_err(|_| format!("--color: bad number in {spec:?}"))?;
        if v.len() != n {
            return Err(format!("--color: expected {n} components in {spec:?}"));
        }
        Ok(v)
    };
    if let Some(hex) = spec.strip_prefix('#') {
        if hex.len() != 6 {
            return Err(format!("--color: expected #rrggbb, got {spec:?}"));
        }
        let byte = |i: usize| -> Result<f64, String> {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map(|b| f64::from(b) / 255.0)
                .map_err(|_| format!("--color: bad hex in {spec:?}"))
        };
        return Ok(Color::Rgb([byte(0)?, byte(2)?, byte(4)?]));
    }
    let (kind, rest) = spec
        .split_once(':')
        .ok_or_else(|| format!("--color: expected kind:components, got {spec:?}"))?;
    match kind {
        "gray" => Ok(Color::Gray(nums(rest, 1)?[0])),
        "rgb" => {
            let v = nums(rest, 3)?;
            Ok(Color::Rgb([v[0], v[1], v[2]]))
        }
        "cmyk" => {
            let v = nums(rest, 4)?;
            Ok(Color::Cmyk([v[0], v[1], v[2], v[3]]))
        }
        "spot" => {
            // NAME may itself contain ':'; TINT and C,M,Y,K are the last two fields.
            let (name_tint, cmyk) = rest
                .rsplit_once(':')
                .ok_or_else(|| format!("--color: spot needs NAME:TINT:C,M,Y,K, got {spec:?}"))?;
            let (name, tint) = name_tint
                .rsplit_once(':')
                .ok_or_else(|| format!("--color: spot needs NAME:TINT:C,M,Y,K, got {spec:?}"))?;
            let v = nums(cmyk, 4)?;
            Ok(Color::Spot {
                name: name.to_string(),
                tint: nums(tint, 1)?[0],
                cmyk: [v[0], v[1], v[2], v[3]],
            })
        }
        other => Err(format!("--color: unknown kind {other:?}")),
    }
}

/// SVG/PNG can only carry sRGB: gray and rgb map to `#rrggbb`, cmyk and spot are refused.
fn svg_fill(color: Option<&Color>) -> Result<String, String> {
    let hex = |c: [f64; 3]| {
        let b = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
    };
    match color {
        None => Ok(SvgOptions::default().fill),
        Some(Color::Gray(g)) => Ok(hex([*g, *g, *g])),
        Some(Color::Rgb(c)) => Ok(hex(*c)),
        Some(Color::Cmyk(_)) | Some(Color::Spot { .. }) => {
            Err("--color: cmyk and spot colours are only possible with --format pdf".into())
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) if msg.is_empty() => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("error: {msg}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
