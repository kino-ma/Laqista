use std::time::SystemTime;

use nom::{character::complete::u64 as text_u64, IResult};

use super::radeon::RadeonMetrics;

// exmaple output: 1529693401.317127: gpu 0.00%, ee 0.00%, vgt 0.00%, ta 0.00%, sx 0.00%, sh 0.00%, spi 0.00%, sc 0.00%, pa 0.00%, db 0.00%, cb 0.00%, vram 0.04% 2.06mb, gtt 0.04% 2.56mb

pub struct Utilization {
    name: String,
    // util is utilization in %.
    // min: 0 (= 0.00%), max: 10000 (= 100.00%)
    util: u16,
}

pub fn radeon_top(input: &str) -> IResult<&str, RadeonMetrics> {
    Ok(())
}

fn time(input: &str) -> IResult<&str, SystemTime> {
    let (input, secs) = text_u64(input)?;
    let (input, nanos) = text_u64(input)?;

    SystemTime::new(secs, nanos)
}
