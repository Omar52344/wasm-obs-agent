use wasmtime::{Engine, Module, Store};
use wasm_obs_agent::{TelemetryObserver, WasmObserver};
use wasm_obs_agent::wrapper::ObservedInstance;
use tokio::sync::mpsc;
use wasm_obs_agent::exporter::run_otlp_exporter; // Asegúrate de que esta importación sea correcta

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
    // Creamos el canal MPSC (Multi-Producer, Single-Consumer)
    let (sender, receiver) = mpsc::unbounded_channel();
    let observer = TelemetryObserver::new(sender.clone()); // Usamos .clone() para mantener el 'sender' original vivo

    // 3. Instancia instrumentada
    let instance = ObservedInstance::new(&mut store, &module, observer)?;
    
    // Iniciamos el exportador OTLP en una tarea de Tokio separada
    let exporter_handle = tokio::spawn(run_otlp_exporter(
        receiver,
        "http://localhost:4317".to_string(), // tu collector OTLP
    ));
    
    // 4. Uso normal del Wasm
    let add = instance.get_func(&mut store, "add").unwrap();
    let mut results = [wasmtime::Val::I32(0)];
    add.call(&mut store, &[wasmtime::Val::I32(5), wasmtime::Val::I32(3)], &mut results)?;
    println!("➕ add(5,3) = {}", results[0].unwrap_i32());

    let multiply = instance.get_func(&mut store, "multiply").unwrap();
    multiply.call(&mut store, &[wasmtime::Val::I32(4), wasmtime::Val::I32(7)], &mut results)?;
    println!("✖️ multiply(4,7) = {}", results[0].unwrap_i32());

    /*while 1==1 {
        println!("✅ último span exportado");
        let multiply = instance.get_func(&mut store, "multiply").unwrap();
        multiply.call(&mut store, &[wasmtime::Val::I32(9), wasmtime::Val::I32(12)], &mut results)?;
        println!("✖️ multiply(9,12) = {}", results[0].unwrap_i32());
    }*/
    

    println!("\n✅ ¡Ninguna función fue modificada manualmente!");
    
    // --- Solución al error "oneshot canceled" ---
    
    // A. Cerramos el canal de envío (sender). 
    // Esto es vital. Le indica al 'receiver' en 'run_otlp_exporter' que ya no vendrán más mensajes.
    // drop(observer); // <-- ELIMINADO: 'observer' ya fue movido al crear 'instance'.
    drop(instance);   // Liberamos la instancia (y sus handles a funciones)
    drop(sender);     // Liberamos el sender de main
    drop(store);      // Liberamos el Store (y las closures con los clones del sender) 

    match exporter_handle.await {
        Ok(_) => println!("✅ Exportador OTLP finalizado limpiamente."),
        Err(e) => eprintln!("⚠️ Error esperando la tarea del exportador: {}", e),
    }

    // B. Esperamos a que la tarea del exportador termine su bucle 'while let Some(span)' 
    // y vacíe su buffer de OpenTelemetry.
    //exporter_handle.await?;
  
    println!("🛑 Exportador cerrado. Adiós.");
    // C. El shutdown ya fue manejado internamente por el exportador con un timeout seguro.
    // No llamamos a shutdown_tracer_provider() aquí para evitar bloqueos si el collector falla.
    println!("✅ Programa finalizado.");
    Ok(())
}
