use serde::Serialize;

#[derive(Debug, PartialEq, Serialize)]
pub struct PowerMetrics {
    pub gpu: Gpu,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Gpu {
    pub freq_mhz: f64,
    pub dvfm_states: Vec<DvfmState>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct DvfmState {
    pub freq_mhz: u16,
}

impl Gpu {
    pub fn max_frequency(&self) -> u16 {
        self.dvfm_states
            .iter()
            .map(|state| state.freq_mhz)
            .max()
            .unwrap()
    }

    pub(crate) fn min_frequency(&self) -> u16 {
        self.dvfm_states
            .iter()
            .map(|state| state.freq_mhz)
            .min()
            .unwrap()
    }

    pub fn utilization_ratio(&self) -> f64 {
        let min = self.min_frequency() as f64;
        let max = self.max_frequency() as f64;
        ((self.freq_mhz - min).max(0.0) / (max - min).max(1.0))
            .max(0.0)
            .min(1.0)
    }
}
