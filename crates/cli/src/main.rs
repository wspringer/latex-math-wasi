//! `latex-math`: LaTeX math in, SVG (or PDF) out.

use std::io::{Read, Write};
use std::process::ExitCode;

use latex_math_core::{Font, FontSet, Options, Style};
use latex_math_pdf::{to_pdf, PdfOptions};
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
  --format svg|pdf   output format (default svg)
  --size N           em size in user units (default 16)
  --style display|text
  --padding N        space around the formula, user units (default 0)
  -o, --output FILE  write here instead of stdout
  -h, --help
";

struct Args {
    fonts: Vec<String>,
    format: String,
    size: f64,
    style: Style,
    padding: f64,
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
    let bytes: Vec<u8> = match args.format.as_str() {
        "svg" => {
            let svg_options = SvgOptions {
                padding: args.padding,
                ..SvgOptions::default()
            };
            to_svg(&tree, &refs, &svg_options)
                .map_err(|e| e.to_string())?
                .into_bytes()
        }
        "pdf" => {
            let pdf_options = PdfOptions {
                padding: args.padding,
            };
            to_pdf(&tree, &refs, &pdf_options).map_err(|e| e.to_string())?
        }
        other => return Err(format!("unknown format {other}")),
    };

    match args.output {
        Some(path) => std::fs::write(&path, bytes).map_err(|e| format!("{path}: {e}")),
        None => std::io::stdout()
            .write_all(&bytes)
            .map_err(|e| e.to_string()),
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
