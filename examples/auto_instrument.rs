use wasmtime::{Engine, Module, Store}; // ← Store estaba ausente
use wasm_obs_agent::{TelemetryObserver, WasmObserver};
use crossbeam_channel;
use wasm_obs_agent::wrapper::ObservedInstance;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let (sender, receiver) = crossbeam_channel::unbounded();
    let observer = TelemetryObserver::new(sender);

    // 3. Instancia instrumentada
    let instance = ObservedInstance::new(&mut store, &module, observer)?;

    // 4. Background telemetry
    std::thread::spawn(move || {
        for span in receiver {
            println!("[📡 TELEMETRY] {}: {} ns", 
                span.function_name,
                span.end_time_ns.unwrap_or(0) - span.start_time_ns
            );
        }
    });

    // 5. Uso normal
    let add = instance.get_func(&mut store, "add").unwrap();
    let mut results = [wasmtime::Val::I32(0)];
    add.call(&mut store, &[wasmtime::Val::I32(5), wasmtime::Val::I32(3)], &mut results)?;
    println!("➕ add(5,3) = {}", results[0].unwrap_i32());

    let multiply = instance.get_func(&mut store, "multiply").unwrap();
    multiply.call(&mut store, &[wasmtime::Val::I32(4), wasmtime::Val::I32(7)], &mut results)?;
    println!("✖️ multiply(4,7) = {}", results[0].unwrap_i32());

    println!("\n✅ ¡Ninguna función fue modificada manualmente!");
    Ok(())
}