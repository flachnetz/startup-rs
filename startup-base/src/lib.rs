use atty::Stream;
use figment::providers::{Env, Format, Yaml};
use figment::Error;
use figment::Figment;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing_subscriber::reload::Handle;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{Layer, Registry};
use parking_lot::RwLock;
use tracing_subscriber::util::SubscriberInitExt;

lazy_static::lazy_static! {
    static ref TRACING_LAYER: RwLock<Option<Handle<Option<Box<dyn Layer<Registry>+Send+Sync>>, Registry>>> = RwLock::new(None);
}

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
}

pub fn init<C: Default + Serialize + DeserializeOwned>(service_name: &str, config: &str) -> Result<C, Error> {
    // install error handler
    color_eyre::install().unwrap();

    // parse base config
    let base_config: BaseConfig = extract(config)?;

    let loglevel = if base_config.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };

    // build a dynamic handler that can be set later
    let (dynamic_layer, reload_handle) = tracing_subscriber::reload::Layer::new(None);

    // set the handle so we can set the filter later on.
    *TRACING_LAYER.write() = Some(reload_handle);

    // a layer for logging based on the requested log level.
    let log_layer = tracing_subscriber::fmt::layer()
        .with_ansi(atty::is(Stream::Stderr))
        .with_filter(loglevel);

    Registry::default()
        .with(dynamic_layer)
        .with(log_layer)
        .init();

    // extract and return app config
    let config = extract_with_default(config)?;

    tracing::info!("Starting application {:?} now", service_name);

    Ok(config)
}

pub fn replace_tracing_layer(layer: Option<Box<dyn Layer<Registry> + Send + Sync>>) -> color_eyre::Result<()> {
    let handler = TRACING_LAYER.read();

    if let Some(handler) = handler.as_ref() {
        handler.reload(layer)?;
        return Ok(());
    }

    Err(color_eyre::eyre::eyre!("tracing handler not yet initialized"))
}
