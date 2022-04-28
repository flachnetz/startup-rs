use eyre::Result;
use opentelemetry::sdk::trace;
use serde::{Deserialize, Serialize};

mod idgenerator;

#[derive(Default, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub zipkin: Option<String>,
    // statsd: HostPort,
}

impl MonitoringConfig {
    pub fn setup(&self, service_name: &str) -> Result<()> {
        if let Some(zipkin) = self.zipkin.as_ref() {
            tracing::info!("Setup zipkin tracing to {}", zipkin);

            opentelemetry::global::set_text_map_propagator(opentelemetry_zipkin::Propagator::new());

            let trace_config = trace::Config::default()
                .with_id_generator(idgenerator::IdGenerator64);

            let tracer = opentelemetry_zipkin::new_pipeline()
                .with_service_name(service_name)
                .with_collector_endpoint(zipkin)
                .with_trace_config(trace_config)
                .install_batch(opentelemetry::runtime::Tokio)
                .unwrap();

            // inject layer into registry
            let layer = tracing_opentelemetry::layer().with_tracer(tracer);
            startup_base::replace_tracing_layer(Some(Box::new(layer)))?;
        }

        Ok(())
    }
}


