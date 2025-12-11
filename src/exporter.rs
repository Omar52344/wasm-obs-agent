use opentelemetry::{
    global,
    trace::{SpanBuilder, SpanKind, Tracer,Span},
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    runtime::Tokio,
    Resource, // <-- Importa esto
    trace as sdktrace,
};
use tokio::sync::mpsc;
use crate::WasmSpan;
use std::time::{Duration, UNIX_EPOCH};
use tokio::sync::oneshot; 
pub async fn run_otlp_exporter(
    mut rx: mpsc::UnboundedReceiver<WasmSpan>,
    endpoint: String,
    ready_tx: oneshot::Sender<()>, 
) {
    println!("🛠️ Exporter task iniciada");
    println!("🛠️ Configurando exportador OTLP a: {}", endpoint);
    let resource = Resource::new(vec![
        KeyValue::new("service.name", "wasm-obs-agent"), // 
        KeyValue::new("environment", "development"),
    ]);
    let exporter_builder = opentelemetry_otlp::new_exporter()
        .tonic() 
        .with_endpoint(&endpoint);
       
    // Instala el exportador batch asíncrono para Tokio
    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter_builder)
        .with_trace_config(
            sdktrace::Config::default()
                .with_resource(resource)
        )
        .install_batch(Tokio)
        .expect("OTLP instalado");
    
    let tracer = global::tracer("wasm-obs-agent");
    if ready_tx.send(()).is_err() {
        eprintln!("⚠️ Error al enviar señal de listo al main.");
        return; // Salir si main ya cerró la espera
    }

    while let Some(span) = rx.recv().await {
        println!("📡 Procesando span para función: {}", span.function_name);

        let start_time = UNIX_EPOCH + Duration::from_nanos(span.start_time_ns);
        let end_time = span.end_time_ns
            .map(|end_ns| UNIX_EPOCH + Duration::from_nanos(end_ns))
            .unwrap_or(start_time); 
        if span.end_time_ns.unwrap_or(0) == 0 {
            println!("⚠️ Ignorando span incompleto para {}", span.function_name);
            continue; // Saltar al siguiente elemento del bucle
        }    

        let mut builder = SpanBuilder::from_name(format!("wasm::{}", span.function_name));
        builder.span_kind = Some(SpanKind::Internal);
        builder.start_time = Some(start_time);
        builder.end_time = Some(end_time);
        
        builder.attributes = Some(vec![
            KeyValue::new("wasm.runtime_id", span.runtime_id.to_string()),
        ]);

        println!("📤 Enviando span: {} ({} -> {})", 
            span.function_name, 
            span.start_time_ns, 
            span.end_time_ns.unwrap_or(0)
        );

        // Finaliza el span para que sea enviado por el batch exporter
        tracer.build(builder).end();
    }
    
    // ELIMINA TODA LA LÓGICA DE APAGADO MANUAL DE HILOS

    // Llama al apagado global aquí, después de que todos los spans han sido procesados.
    // El 'await' en main esperará a que esta función termine.
    opentelemetry::global::shutdown_tracer_provider();
    println!("✅ Shutdown de OTLP completado dentro del exporter.");
}
