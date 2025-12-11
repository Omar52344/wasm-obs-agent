use opentelemetry::{
    global,
    trace::{SpanBuilder, SpanKind, Tracer},
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::runtime::Tokio;
use tokio::sync::mpsc;
use crate::WasmSpan;
use std::time::{Duration, UNIX_EPOCH}; // Importa Duration y UNIX_EPOCH

pub async fn run_otlp_exporter(
    mut rx: mpsc::UnboundedReceiver<WasmSpan>,
    endpoint: String,
) {
    // ... (la configuración del pipeline es correcta)
    println!("🛠️ Exporter task iniciada");
    
    println!("🛠️ Configurando exportador OTLP...");
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&endpoint);
    
    // ... (configuración del pipeline)
    // Usamos install_simple para evitar problemas de hilos de fondo/batching en apps cortas
    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .install_simple() 
        .expect("OTLP instalado");

    let tracer = global::tracer("wasm-obs-agent");

    while let Some(span) = rx.recv().await {
        // ... (código existente)
        // Usa UNIX_EPOCH para convertir nanosegundos a SystemTime
        let start_time = UNIX_EPOCH + Duration::from_nanos(span.start_time_ns);
        
        let end_time = span.end_time_ns
            .map(|end_ns| UNIX_EPOCH + Duration::from_nanos(end_ns))
            .unwrap_or(start_time); 

        let mut builder = SpanBuilder::from_name(format!("wasm::{}", span.function_name));
        builder.span_kind = Some(SpanKind::Internal);
        builder.start_time = Some(start_time);
        builder.end_time = Some(end_time);
        
        builder.attributes = Some(vec![
            KeyValue::new("wasm.runtime_id", span.runtime_id.to_string()),
        ]);
        
        tracer.build(builder);
    }

    // Ejecutamos shutdown en un thread dedicado para no bloquear el runtime de Tokio
    // y poder aplicar un timeout manual si el OTLP collector no responde.
    let shutdown_handle = std::thread::spawn(|| {
        opentelemetry::global::shutdown_tracer_provider();
    });

    // Esperamos máximo 3 segundos para el shutdown
    let timeout = Duration::from_secs(3);
    let start = std::time::Instant::now();
    
    loop {
        if shutdown_handle.is_finished() {
            break;
        }
        if start.elapsed() > timeout {
            eprintln!("⚠️ Warning: OTLP exporter shutdown timed out.");
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
