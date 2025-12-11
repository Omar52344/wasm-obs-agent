use wasmtime::{Engine, Module, Store};
use wasm_obs_agent::{TelemetryObserver, WasmObserver};
use wasm_obs_agent::wrapper::ObservedInstance;
use tokio::sync::mpsc;
use wasm_obs_agent::exporter::run_otlp_exporter; 
use tokio::sync::oneshot;
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
    // --> Aquí se define 'module' y entra en scope <--
    let module = Module::new(&engine, wasm_bytes)?; 
    let mut store = Store::new(&engine, ());

    // 2. Canal y observer
    let (sender, receiver) = mpsc::unbounded_channel();
    let observer = TelemetryObserver::new(sender.clone()); 

    // 3. Instancia instrumentada
    // 'module' es accesible aquí y el error desaparecerá:
    let (ready_tx, ready_rx) = oneshot::channel();
    let instance = ObservedInstance::new(&mut store, &module, observer)?;
    
    
    // Iniciamos el exportador OTLP en una tarea de Tokio separada
   let exporter_handle = tokio::spawn(run_otlp_exporter(
        receiver,
        // Usamos localhost ahora que hemos verificado el mapeo de puertos y telnet
        "http://localhost:4318".to_string(), 
        ready_tx
    ));
    ready_rx.await.expect("No se pudo sincronizar con la tarea del exportador.");
    println!("🛠️ Exportador OTLP sincronizado y listo. Generando spans...");
    // 4. Uso normal del Wasm
    let add = instance.get_func(&mut store, "add").unwrap();
    let mut results = [wasmtime::Val::I32(0)];
    add.call(&mut store, &[wasmtime::Val::I32(5), wasmtime::Val::I32(3)], &mut results)?;
    println!("➕ add(5,3) = {}", results[0].unwrap_i32());

    let multiply = instance.get_func(&mut store, "multiply").unwrap();
    multiply.call(&mut store, &[wasmtime::Val::I32(4), wasmtime::Val::I32(7)], &mut results)?;
    println!("✖️ multiply(4,7) = {}", results[0].unwrap_i32());

    let test = instance.get_func(&mut store, "add").unwrap();
    test.call(&mut store, &[wasmtime::Val::I32(999), wasmtime::Val::I32(1)], &mut results)?;
    println!("🔍 test span enviado");

    println!("\n✅ ¡Ninguna función fue modificada manualmente!");
    
    // --- CIERRE ROBUSTO ---
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    // Forzamos la caída de las variables que tienen copias del 'sender' ANTES del .await:
    drop(instance); 
    drop(store);    
    drop(sender);   

    // Ahora esperamos al exportador, quien ya sabe que no hay más datos.
    match exporter_handle.await {
        Ok(_) => println!("✅ Exportador OTLP finalizado limpiamente."),
        Err(e) => eprintln!("⚠️ Error esperando la tarea del exportador: {}", e),
    }
  
    println!("✅ Programa finalizado.");
    Ok(())
}
