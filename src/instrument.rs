use wasmtime::{Caller, Func, Store, Module, Instance, Extern, Val, ValType};
use std::collections::HashMap;
use std::sync::Arc;
use crate::WasmObserver;

/// Toma una función Wasm y retorna una versión instrumentada con el mismo comportamiento
pub fn instrument_function<T>(
    store: &mut Store<T>,
    original_func: Func,
    observer: Arc<dyn WasmObserver>,
    func_name: String,
) -> Func 
where
    T: Send + 'static,
{
    // Obtenemos la firma de la función (parámetros y retorno)
    let func_ty = original_func.ty(&store);
    let param_types: Vec<ValType> = func_ty.params().collect();
    let result_types: Vec<ValType> = func_ty.results().collect();

    // Creamos una función wrapper con la MISMA FIRMA
    let wrapper = move |mut caller: Caller<'_, T>, params: &[Val], results: &mut [Val]| {
        let runtime_id = uuid::Uuid::new_v4();
        
        // PRE-CALL: Notificamos al observer
        observer.on_func_enter(runtime_id, &func_name);

        let start = std::time::Instant::now();

        // 🔥 LLAMADA ORIGINAL A LA FUNCIÓN WASM
        let call_result = original_func.call(&mut caller, params, results);

        let duration_ns = start.elapsed().as_nanos() as u64;

        // POST-CALL: Notificamos
        observer.on_func_exit(runtime_id, &func_name, duration_ns);

        call_result // Retornamos el resultado original
    };

    // Creamos la nueva función con la misma firma que la original
    Func::new(
        &mut *store, 
        func_ty.clone(), 
        wrapper
    )
}

/// Función mágica: instancia un módulo y reemplaza todas sus funciones exportadas
pub fn create_instrumented_funcs<T>(
    store: &mut Store<T>,
    module: &Module,
    observer: Arc<dyn WasmObserver>,
) -> wasmtime::Result<HashMap<String, Func>>
where
    T: Send + 'static,
{
    let mut instrumented_funcs = HashMap::new();

    // 1. Instanciamos *solamente* para obtener acceso a las funciones originales
    let instance = Instance::new(&mut *store, module, &[])?;
    
    // 2. Iteramos y creamos las versiones instrumentadas
    // Usamos iter().map() y collect() para evitar problemas de ownership en el bucle
    let export_names: Vec<String> = module.exports().map(|e| e.name().to_string()).collect();

    for name in export_names {
        // get_func(&mut *store, ...) ahora funciona porque estamos re-prestando 'store'
        if let Some(original_func) = instance.get_func(&mut *store, &name) {
            let instrumented = instrument_function(
                &mut *store,
                original_func,
                observer.clone(),
                name.clone(),
            );
            instrumented_funcs.insert(name.clone(), instrumented);
            //println!("✅ Función '{}' instrumentada automáticamente", name);
        }
    }

    Ok(instrumented_funcs)
}