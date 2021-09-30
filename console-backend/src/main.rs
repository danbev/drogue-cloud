use drogue_cloud_console_backend::{run, Config};
use drogue_cloud_service_common::config::ConfigFromEnv;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let config = Config::from_env().unwrap();
    run(config).await
}
