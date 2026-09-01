use nom::Parser;
use nom::{
    bytes::complete::tag,
    character::complete::{anychar, hex_digit1, line_ending, none_of, space0},
    combinator::{map_opt, map_res, recognize},
    multi::{many0, many1, many_till},
    IResult,
};

use crate::common::{TexSymbolType, OPERATOR_LIMITS};

#[derive(Debug)]
pub struct Line<'a> {
    pub codepoint: u32,
    pub command_name: &'a str,
    pub tex_symbol_type: TexSymbolType,
    pub description: &'a str,
}

pub fn parse_file(input: &str) -> IResult<&str, Vec<Line<'_>>> {
    let (input, _) = many0(parse_comment).parse(input)?;
    let (input, _) = many0(line_ending).parse(input)?;
    nom::combinator::map(many_till(parse_line, line_ending), |(result, _)| result).parse(input)
}

fn parse_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = tag("%").parse(input)?;
    nom::combinator::map(many_till(anychar, line_ending), |_| ()).parse(input)
}

fn parse_line(input: &str) -> IResult<&str, Line<'_>> {
    let (input, _) = tag("\\UnicodeMathSymbol{\"").parse(input)?;
    let (input, codepoint) = map_res(hex_digit1, |hex_digits: &str| {
        u32::from_str_radix(hex_digits, 16)
    })
    .parse(input)?;
    let (input, _) = tag("}{\\").parse(input)?;
    let (input, command_name) = recognize(many1(none_of(" }"))).parse(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = tag("}{\\").parse(input)?;
    let (input, mut tex_symbol_type) = map_opt(
        recognize(many1(none_of(" }"))),
        TexSymbolType::from_tex_name,
    )
    .parse(input)?;
    let (input, _) = tag("}{").parse(input)?;
    let (input, description) = recognize(many1(none_of("}"))).parse(input)?;
    if let TexSymbolType::Operator(limits) = &mut tex_symbol_type {
        *limits = OPERATOR_LIMITS.contains(&command_name);
    }

    let (input, _) = many_till(anychar, line_ending).parse(input)?;

    Ok((
        input,
        Line {
            codepoint,
            command_name,
            tex_symbol_type,
            description,
        },
    ))
}
