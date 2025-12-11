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

    println!("🔍 Paso 1: Resource creada");  // Nuevo log
    let exporter_builder = opentelemetry_otlp::new_exporter()
        .http()
        .with_endpoint(&endpoint);
    println!("🔍 Paso 2: Exporter builder creado");  

    let pipeline = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter_builder);

    println!("🔍 Paso 3: Pipeline básica creada");  // Nuevo log
    // Instala el exportador batch asíncrono para Tokio
    let configured_pipeline = pipeline
            .with_trace_config(sdktrace::config().with_resource(resource))
            .with_batch_config(
                sdktrace::BatchConfigBuilder::default()
                    .with_scheduled_delay(Duration::from_millis(500))
                    .with_max_export_batch_size(512)
                    .with_max_queue_size(2048)
                    .build()
            );

    println!("🔍 Paso 4: Config batch y trace agregada");  // Nuevo log
    let install_result = configured_pipeline.install_batch(Tokio);

    match install_result {
        Ok(_) => println!("🔍 Paso 5: OTLP batch instalado exitosamente"),
        Err(e) => {
            eprintln!("❌ Error instalando OTLP pipeline: {:?}", e);
            return;  // Salir si falla
        }
    }

    let tracer = global::tracer("wasm-obs-agent");
    println!("🔍 Paso 6: Tracer obtenido");  // Nuevo log
    if ready_tx.send(()).is_err() {
        eprintln!("⚠️ Error al enviar señal de listo al main.");
        return; // Salir si main ya cerró la espera
    }
    println!("🔍 Paso 7: Ready enviado");  // Nuevo log

    while let Some(span) = rx.recv().await {
        println!("📡 Procesando span para función: {}", span.function_name);

        if let Some(end_ns) = span.end_time_ns {
            if end_ns <= span.start_time_ns {
                println!("⚠️ Ignorando span con duración inválida (<=0) para {}", span.function_name);
                continue;
            }

            let start_time = UNIX_EPOCH + Duration::from_nanos(span.start_time_ns);
            let end_time = UNIX_EPOCH + Duration::from_nanos(end_ns);

            let mut builder = SpanBuilder::from_name(format!("wasm::{}", span.function_name));
            builder.span_kind = Some(SpanKind::Internal);
            builder.start_time = Some(start_time);
            builder.end_time = Some(end_time);
            
            builder.attributes = Some(vec![
                KeyValue::new("wasm.runtime_id", span.runtime_id.to_string()),
                // Opcional: agrega más attrs, e.g., KeyValue::new("status", format!("{:?}", span.status)),
            ]);

            println!("📤 Enviando span completo: {} (duración: {} ns)", 
                span.function_name, 
                end_ns - span.start_time_ns
            );

            tracer.build(builder).end();
        } else {
            println!("⚠️ Ignorando span incompleto (sin end_time_ns) para {}", span.function_name);
        }
    }
    
    global::shutdown_tracer_provider();
    println!("✅ Shutdown de OTLP completado dentro del exporter.");
}
