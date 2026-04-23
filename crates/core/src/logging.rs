use std::sync::Once;

use tracing_subscriber::EnvFilter;

static LOG_INIT: Once = Once::new();

pub fn init_logging(default_level: &str) {
 LOG_INIT.call_once(|| {
  let env_filter = EnvFilter::try_from_default_env()
   .or_else(|_| EnvFilter::try_new(default_level))
   .unwrap_or_else(|_| EnvFilter::new("info"));

  let _ = tracing_subscriber::fmt()
   .with_env_filter(env_filter)
   .with_target(true)
   .with_thread_ids(true)
   .compact()
   .try_init();
 });
}
