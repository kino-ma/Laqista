use chrono::{DateTime, Utc};
use nom::{
    character::complete::{i64 as text_i64, u32 as text_u32},
    error::{ErrorKind, ParseError},
    Err as NomErr, IResult,
};

use super::radeon::RadeonMetrics;

pub enum MetricsParseError<I> {
    Timestamp { secs: i64, nsecs: u32 },
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

// exmaple output: 1529693401.317127: gpu 0.00%, ee 0.00%, vgt 0.00%, ta 0.00%, sx 0.00%, sh 0.00%, spi 0.00%, sc 0.00%, pa 0.00%, db 0.00%, cb 0.00%, vram 0.04% 2.06mb, gtt 0.04% 2.56mb

pub struct Utilization {
    name: String,
    // util is utilization in %.
    // min: 0 (= 0.00%), max: 10000 (= 100.00%)
    util: u16,
}

pub fn radeon_top(input: &str) -> Result<&str, RadeonMetrics> {
    let (input, ts) = timestamp(input)?;

    let metrics = RadeonMetrics { timestamp: ts };

    Ok((input, metrics))
}

fn timestamp(input: &str) -> Result<&str, DateTime<Utc>> {
    let (input, secs) = text_i64(input)?;
    let (input, nsecs) = text_u32(input)?;

    let dt = DateTime::from_timestamp(secs, nsecs)
        .ok_or(NomErr::Error(MetricsParseError::Timestamp { secs, nsecs }))?;

    Ok((input, dt))
}
