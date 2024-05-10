use std::{char, collections::HashMap, num::ParseIntError};

use chrono::{DateTime, Utc};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{alpha1, i64 as text_i64, u32 as text_u32, u64 as text_u64},
    error::{ErrorKind, ParseError},
    multi::separated_list1,
    Err as NomErr, IResult,
};

use super::radeon::RadeonMetrics;

pub enum MetricsParseError<I> {
    Timestamp { secs: i64, nsecs: u32 },
    Int(ParseIntError),
    Nom(I, ErrorKind),
}
type Result<I, O> = IResult<I, O, MetricsParseError<I>>;

impl<I> ParseError<I> for MetricsParseError<I> {
    fn append(_input: I, _kind: nom::error::ErrorKind, other: Self) -> Self {
        other
    }

    fn from_error_kind(input: I, kind: ErrorKind) -> Self {
        Self::Nom(input, kind)
    }
}

// exmaple output: 1715302360.857296: bus 06, gpu 5.00%, ee 0.00%, vgt 0.83%, ta 5.00%, sx 5.00%, sh 0.00%, spi 5.00%, sc 5.00%, pa 0.83%, db 5.00%, cb 5.00%, vram 19.57% 400.73mb, gtt 2.08% 42.61mb, mclk inf% 0.355ghz, sclk 38.53% 0.328ghz

pub struct ResourceUtilization {
    name: String,
    // util is utilization in %.
    // min: 0 (= 0.00%), max: 10000 (= 100.00%)
    util: Utilization,
}

pub enum Utilization {
    Id {
        id: u64,
    },
    Percent {
        ratio: u64,
        abs: AbsoluteUtilization,
    },
}

pub enum AbsoluteUtilization {
    None,
    Mb(u64),
    Ghz(u64),
}

pub fn radeon_top(input: &str) -> Result<&str, RadeonMetrics> {
    let (input, ts) = timestamp(input)?;
    let (input, _colon) = tag(": ")(input)?;
    let (input, map) = utilization_map(input)?;

    let metrics = RadeonMetrics {} // desiriaslize;

    Ok((input, metrics))
}

fn timestamp(input: &str) -> Result<&str, DateTime<Utc>> {
    let (input, secs) = text_i64(input)?;
    let (input, nsecs) = text_u32(input)?;

    let (input, _) = tag(": ")(input)?;

    let dt = DateTime::from_timestamp(secs, nsecs)
        .ok_or(NomErr::Error(MetricsParseError::Timestamp { secs, nsecs }))?;

    Ok((input, dt))
}

fn utilization_map(input: &str) -> Result<&str, HashMap<String, ResourceUtilization>> {
    let (input, list) = separated_list1(tag(", "), resource_utilzation)(input)?;

    let map = list.into_iter().map(|u| (u.name.clone(), u)).collect();

    Ok((input, map))
}

fn resource_utilzation(input: &str) -> Result<&str, ResourceUtilization> {
    let (input, name) = alpha1(input)?;
    let (input, _) = space(input)?;
    let (input, util) = utilization(input)?;

    let out = ResourceUtilization {
        name: name.to_owned(),
        util,
    };

    Ok((input, out))
}

fn utilization(input: &str) -> Result<&str, Utilization> {
    alt((utilization_percent, utilization_id))(input)
}

fn utilization_id(input: &str) -> Result<&str, Utilization> {
    let (input, id) = hex(input)?;
    let util = Utilization::Id { id: id as _ };
    Ok((input, util))
}

fn utilization_percent(input: &str) -> Result<&str, Utilization> {
    let (input, ratio) = frac_u64(input)?;
    let (input, _percent) = tag("%")(input)?;
    let (input, abs) = absolute_utilization(input)?;

    let util = Utilization::Percent { ratio, abs };

    Ok((input, util))
}

fn absolute_utilization(input: &str) -> Result<&str, AbsoluteUtilization> {
    alt((
        absolute_utilization_mb,
        absolute_utilization_ghz,
        absolute_utilization_none,
    ))(input)
}

fn absolute_utilization_mb(input: &str) -> Result<&str, AbsoluteUtilization> {
    let (input, mb) = frac_u64(input)?;
    let (input, _) = tag("mb")(input)?;
    Ok((input, AbsoluteUtilization::Mb(mb)))
}

fn absolute_utilization_ghz(input: &str) -> Result<&str, AbsoluteUtilization> {
    let (input, ghz) = frac_u64(input)?;
    let (input, _) = tag("ghz")(input)?;
    Ok((input, AbsoluteUtilization::Ghz(ghz)))
}

fn absolute_utilization_none(input: &str) -> Result<&str, AbsoluteUtilization> {
    Ok((input, AbsoluteUtilization::None))
}

fn frac_u64(input: &str) -> Result<&str, u64> {
    let (input, int) = text_u64(input)?;
    let (input, _) = dot(input)?;
    let (input, frac) = text_u64(input)?;

    let num = (int << 8) & frac;

    Ok((input, num))
}

fn hex(input: &str) -> Result<&str, u64> {
    let (input, value) = take_while1(is_hex_digit)(input)?;
    let parsed =
        u64::from_str_radix(value, 16).map_err(|e| NomErr::Error(MetricsParseError::Int(e)))?;

    Ok((input, parsed))
}

fn is_hex_digit(c: char) -> bool {
    c.is_digit(16)
}

fn space(input: &str) -> Result<&str, ()> {
    let (input, _) = tag(" ")(input)?;
    Ok((input, ()))
}

fn dot(input: &str) -> Result<&str, ()> {
    let (input, _) = tag(".")(input)?;
    Ok((input, ()))
}
