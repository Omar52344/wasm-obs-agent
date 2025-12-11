use opentelemetry::{
    global,
    trace::{SpanBuilder, SpanKind, Tracer},
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::runtime::Tokio;
use tokio::sync::mpsc;
use crate::WasmSpan;

pub async fn run_otlp_exporter(
    mut rx: mpsc::UnboundedReceiver<WasmSpan>,
    endpoint: String,
) {
    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(&endpoint),
        )
        .install_batch(Tokio)
        .expect("OTLP instalado");

    let tracer = global::tracer("wasm-obs-agent");

    while let Some(span) = rx.recv().await {
        let mut builder = SpanBuilder::from_name(format!("wasm::{}", span.function_name));
        builder.span_kind = Some(SpanKind::Internal);
        builder.attributes = Some(vec![
            KeyValue::new("wasm.runtime_id", span.runtime_id.to_string()),
            KeyValue::new("wasm.duration_ns", (span.end_time_ns.unwrap_or(0) - span.start_time_ns) as i64),
        ]);
        tracer.build(builder);
    }
}