// src/wrapper.rs (Actualizado para el nuevo flujo lógico)

use wasmtime::{Store, Module, Instance, Func};
use std::sync::Arc;
use std::collections::HashMap; // Importamos HashMap
// Importamos la función con el nombre correcto
use crate::{WasmObserver, instrument::create_instrumented_funcs}; 

/// Wrapper que oculta toda la instrumentación del cliente
pub struct ObservedInstance {
    // Almacenamos un mapa de funciones instrumentadas en lugar de la instancia completa
    funcs: HashMap<String, Func>, 
    // Opcional: si necesitas la instancia por alguna otra razón
    // inner_instance: Instance, 
}

impl ObservedInstance {
    /// ÚNICO CAMBIO QUE EL CLIENTE HACE: Llama a este método en lugar de Instance::new
    pub fn new<T>(
        store: &mut Store<T>,
        module: &Module,
        observer: Arc<dyn WasmObserver>,
    ) -> wasmtime::Result<Self> 
    where
        T: Send + 'static,
    {
        // Llamamos a la función con el nombre correcto
        let funcs_map = create_instrumented_funcs(store, module, observer)?;
        
        Ok(Self { 
            funcs: funcs_map,
            // inner_instance: ... 
        })
    }

    /// El resto de la API es idéntica a Instance (pero usa nuestro mapa)
    pub fn get_func(&self, _store: &mut Store<()>, name: &str) -> Option<wasmtime::Func> {
        // Obtenemos la función de nuestro mapa, no de una instancia interna
        self.funcs.get(name).cloned()
    }
}
