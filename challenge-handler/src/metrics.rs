use lazy_static::lazy_static;
use prometheus::{IntGauge, Registry};

pub struct Metrics {
    pub detected_batch_index: IntGauge,
    pub chunks_len: IntGauge,
    pub prover_status: IntGauge,
}

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    pub static ref METRICS: Metrics = Metrics {
        detected_batch_index: IntGauge::new("detected_batch_index", "detected batch index").expect("metric can be created"),
        chunks_len: IntGauge::new("chunks_len", "chunks len").expect("metric can be created"),
        prover_status: IntGauge::new("prover_status", "prover status").expect("metric can be created"),
    };
}
