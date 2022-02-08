use figment::providers::{Env, Format, Yaml};
use figment::Error;
use figment::Figment;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{Layer, Registry};

#[macro_export]
macro_rules! init {
    ( $name:expr ) => {
        $crate::init(env!("CARGO_PKG_NAME"), include_str!($name))
    };
}

fn extract<C: Serialize + DeserializeOwned>(default_yaml: &str) -> Result<C, Error> {
    let config = Figment::new()
        .merge(Yaml::string(default_yaml))
        .merge(Env::prefixed("APP_").split("__"))
        .extract()?;

    Ok(config)
}

fn extract_with_default<C: Default + Serialize + DeserializeOwned>(default_yaml: &str) -> Result<C, Error> {
    // serialize default config to use as a start
    let defaults = figment::providers::Serialized::defaults(C::default());

    let config = Figment::from(defaults)
        .merge(Yaml::string(default_yaml))
        .merge(Env::prefixed("APP_").split("__"))
        .extract()?;

    Ok(config)
}

#[derive(Serialize, Deserialize)]
struct BaseConfig {
    #[serde(default)]
    verbose: bool,

    // TODO probably move into a monitoring package
    zipkin: String,
    // TODO add metrics
    // statsd: HostPort,
}

pub fn init<C: Default + Serialize + DeserializeOwned>(
    service_name: impl Into<String>,
    config: &str,
) -> Result<C, Error> {
    // install error handler
    color_eyre::install().unwrap();

    // parse base config
    let base_config: BaseConfig = extract(config)?;

    let loglevel = if base_config.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };

    opentelemetry::global::set_text_map_propagator(opentelemetry_zipkin::Propagator::new());

    let tracer = opentelemetry_zipkin::new_pipeline()
        .with_service_name(service_name)
        .with_collector_endpoint(base_config.zipkin.clone())
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();

    let registry = Registry::default()
        .with(tracing_subscriber::fmt::layer().with_filter(loglevel))
        .with(tracing_opentelemetry::layer().with_tracer(tracer));

    // TODO maybe move this out here, or put it into some kind of guard / let the app handle this.
    tracing::info!("Initializing zipkin tracing to {:?}", base_config.zipkin);
    tracing::subscriber::set_global_default(registry).expect("set global tracer");

    // extract and return app config
    let config = extract_with_default(config)?;

    Ok(config)
}
