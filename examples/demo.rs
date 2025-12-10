use wasmtime::{Engine, Instance, Module, Store}; // Importamos Instance
use wasm_obs_agent::{TelemetryObserver, WasmObserver};
use crossbeam_channel;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Engine y módulo Wasm
    let engine = Engine::default();
    let wasm_bytes = wat::parse_str(r#"
        (module
            (func $add (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.add)
            (export "add" (func $add))
        )
    "#)?;
    let module = Module::new(&engine, wasm_bytes)?;

    // 2. Canal de telemetría
    let (sender, receiver) = crossbeam_channel::unbounded();

    // 3. Observer
    let observer = TelemetryObserver::new(sender);

    // 4. Store normal de wasmtime
    // El segundo argumento es la "data" del store, que es donde se guarda el observer.
    let mut store = Store::new(&engine, observer); 

    // 5. Instancia normal (con la API actual)
    // Usamos Instance::new en lugar del método obsoleto module.instantiate
    let instance = Instance::new(&mut store, &module, &[])?; 

    // 6. Consumir telemetría en background
    std::thread::spawn(move || {
        for span in receiver {
            println!("[TELEMETRY] Función: {}, Duración: {:?}ns", 
                span.function_name,
                span.end_time_ns.unwrap_or(0) - span.start_time_ns
            );
        }
    });

    // 7. Llamar a función
    let add = instance.get_func(&mut store, "add")
        .ok_or("Función no encontrada")?;
    
    let mut results = [wasmtime::Val::I32(0)];
    add.call(&mut store, &[wasmtime::Val::I32(5), wasmtime::Val::I32(3)], &mut results)?;
    
    println!("Resultado: {}", results[0].unwrap_i32());

    Ok(())
}
