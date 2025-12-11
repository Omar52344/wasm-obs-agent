pub mod instrument;
pub mod wrapper;
pub mod exporter; 
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use std::sync::Mutex;
use std::collections::HashMap;

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
    sender: UnboundedSender<WasmSpan>,
    pending_starts: Mutex<HashMap<Uuid, u64>>,  // Nuevo: almacena starts por runtime_id
}

impl TelemetryObserver {
    pub fn new(sender: UnboundedSender<WasmSpan>) -> Arc<dyn WasmObserver> {
        Arc::new(Self {
            sender,
            pending_starts: Mutex::new(HashMap::new()),
        })
    }
}


impl WasmObserver for TelemetryObserver {
    fn on_func_enter(&self, runtime_id: Uuid, func_name: &str) {
        let start_ns = now_ns();
        self.pending_starts.lock().unwrap().insert(runtime_id, start_ns);
        // Opcional: envía span start si quieres, pero recomiendo no para simplicidad
        // let span = WasmSpan { id: runtime_id, runtime_id, function_name: func_name.to_string(), start_time_ns: start_ns, end_time_ns: None, ... };
        // self.sender.send(span);
    }

    fn on_func_exit(&self, runtime_id: Uuid, func_name: &str, duration_ns: u64) {
        if let Some(start_ns) = self.pending_starts.lock().unwrap().remove(&runtime_id) {
            let end_ns = start_ns + duration_ns;  // Usa duration para precisión monotonic
            let span = WasmSpan {
                id: Uuid::new_v4(),  // O usa runtime_id como id si quieres correlación simple
                runtime_id,  // Usa el pasado
                function_name: func_name.to_string(),
                start_time_ns: start_ns,
                end_time_ns: Some(end_ns),
                memory_bytes: 0,
                status: SpanStatus::Completed,
            };
            let _ = self.sender.send(span);
        } else {
            eprintln!("⚠️ No se encontró start para runtime_id: {}", runtime_id);  // Log error si mismatch
        }
    }
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}