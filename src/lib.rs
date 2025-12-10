pub mod instrument;
pub mod wrapper;

use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmSpan {
    pub id: Uuid,
    pub runtime_id: Uuid,
    pub function_name: String,
    pub start_time_ns: u64,
    pub end_time_ns: Option<u64>,
    pub memory_bytes: usize,
    pub status: SpanStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanStatus {
    Started,
    Completed,
    Failed(String),
}

pub trait WasmObserver: Send + Sync {
    fn on_func_enter(&self, runtime_id: Uuid, func_name: &str);
    fn on_func_exit(&self, runtime_id: Uuid, func_name: &str, duration_ns: u64);
}

pub struct TelemetryObserver {
    runtime_id: Uuid,
    sender: crossbeam_channel::Sender<WasmSpan>,
}

impl TelemetryObserver {
    pub fn new(sender: crossbeam_channel::Sender<WasmSpan>) -> Arc<dyn WasmObserver> {
        Arc::new(Self {
            runtime_id: Uuid::new_v4(),
            sender,
        })
    }
}

impl WasmObserver for TelemetryObserver {
    fn on_func_enter(&self, _runtime_id: Uuid, func_name: &str) {
        let span = WasmSpan {
            id: Uuid::new_v4(),
            runtime_id: self.runtime_id,
            function_name: func_name.to_string(),
            start_time_ns: now_ns(),
            end_time_ns: None,
            memory_bytes: 0,
            status: SpanStatus::Started,
        };
        let _ = self.sender.send(span);
    }

    fn on_func_exit(&self, _runtime_id: Uuid, func_name: &str, duration_ns: u64) {
        let span = WasmSpan {
            id: Uuid::new_v4(),
            runtime_id: self.runtime_id,
            function_name: func_name.to_string(),
            start_time_ns: now_ns() - duration_ns,
            end_time_ns: Some(now_ns()),
            memory_bytes: 0,
            status: SpanStatus::Completed,
        };
        let _ = self.sender.send(span);
    }
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}