use std::time::SystemTime;

// exmaple output: 1529693401.317127: gpu 0.00%, ee 0.00%, vgt 0.00%, ta 0.00%, sx 0.00%, sh 0.00%, spi 0.00%, sc 0.00%, pa 0.00%, db 0.00%, cb 0.00%, vram 0.04% 2.06mb, gtt 0.04% 2.56mb

pub struct MetricsMonitor {}

#[derive(Clone, Debug)]
pub struct MetricsWindow {
    start: SystemTime,
    end: SystemTime,
    metrics: RadeonMetrics,
}

#[derive(Clone, Debug)]
pub struct RadeonMetrics {
    pub gpu: f64,
    pub ee: f64,
    pub vgt: f64,
    pub ta: f64,
    pub sx: f64,
    pub sh: f64,
    pub spi: f64,
    pub sc: f64,
    pub pa: f64,
    pub db: f64,
    pub cb: f64,
    pub vram: f64,
    pub git: f64,
}
