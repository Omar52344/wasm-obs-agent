use wasmtime::{Engine, Module, Store}; // ← Store estaba ausente
use wasm_obs_agent::{TelemetryObserver, WasmObserver, exporter::run_otlp_exporter}; 
use crossbeam_channel;
use wasm_obs_agent::wrapper::ObservedInstance;

// Añade la macro tokio::main para permitir main async
#[tokio::main] 
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Demo: Auto-instrumentación sin cambiar el Wasm");

    // 1. Setup normal de wasmtime
    let engine = Engine::default();
    let wasm_bytes = wat::parse_str(r#"
        (module
            (func $add (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.add)
            (export "add" (func $add))
            
            (func $multiply (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.mul)
            (export "multiply" (func $multiply))
        )
    "#)?;
    let module = Module::new(&engine, wasm_bytes)?;
    let mut store = Store::new(&engine, ());

    // 2. Canal y observer
    // Usa tokio::sync::mpsc::unbounded_channel() y el import necesario
    use tokio::sync::mpsc; 
    let (sender, receiver) = mpsc::unbounded_channel();
    let observer = TelemetryObserver::new(sender);

    // 3. Instancia instrumentada
    // Asumiendo que run_otlp_exporter está en el módulo exporter de tu crate:
    use wasm_obs_agent::exporter::run_otlp_exporter; 
    let instance = ObservedInstance::new(&mut store, &module, observer)?;
    
    tokio::spawn(run_otlp_exporter(
        receiver,
        "http://localhost:4317".to_string(), // tu collector OTLP
    ));
    
    // 4. Background telemetry
    // WARNING: Esto consumirá el 'receiver' del canal secundario. 
    // Si usas el exportador OTLP arriba, este hilo ya no recibirá mensajes.
    /* 
    std::thread::spawn(move || {
        for span in receiver { // 'receiver' es ahora un UnboundedReceiver<WasmSpan> de tokio
            println!("[📡 TELEMETRY] {}: {} ns", 
                span.function_name,
                span.end_time_ns.unwrap_or(0) - span.start_time_ns
            );
        }
    });
    */

    // 5. Uso normal
    let add = instance.get_func(&mut store, "add").unwrap();
    let mut results = [wasmtime::Val::I32(0)];
    add.call(&mut store, &[wasmtime::Val::I32(5), wasmtime::Val::I32(3)], &mut results)?;
    println!("➕ add(5,3) = {}", results[0].unwrap_i32());

    let multiply = instance.get_func(&mut store, "multiply").unwrap();
    multiply.call(&mut store, &[wasmtime::Val::I32(4), wasmtime::Val::I32(7)], &mut results)?;
    println!("✖️ multiply(4,7) = {}", results[0].unwrap_i32());

    println!("\n✅ ¡Ninguna función fue modificada manualmente!");
    
    // Da tiempo al exportador async para enviar los spans antes de que main termine.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    Ok(())
}
