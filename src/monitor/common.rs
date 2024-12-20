use sysinfo::System;

pub fn collect_cpu_usage(sys: &mut System) -> f64 {
    sys.refresh_cpu_all();
    let cpus = sys.cpus();
    cpus.iter().map(|cpu| cpu.cpu_usage() as f64).sum::<f64>() / cpus.len() as f64
}
