mod auth;
mod service;

use crate::{
    auth::{AcceptAllClientCertVerifier, DeviceAuthenticator},
    service::App,
};
use drogue_cloud_endpoint_common::{
    command::{Commands, KafkaCommandSource, KafkaCommandSourceConfig},
    sender::DownstreamSender,
    sink::KafkaSink,
};
use drogue_cloud_mqtt_common::server::{build, TlsConfig};
use drogue_cloud_service_common::health::{HealthServer, HealthServerConfig};
use futures::TryFutureExt;
use rust_tls::ClientCertVerifier;
use serde::Deserialize;
use std::{fmt::Debug, sync::Arc};

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub disable_tls: bool,
    #[serde(default)]
    pub cert_bundle_file: Option<String>,
    #[serde(default)]
    pub key_file: Option<String>,
    #[serde(default)]
    pub bind_addr_mqtt: Option<String>,

    #[serde(default)]
    pub health: Option<HealthServerConfig>,

    pub command_source_kafka: KafkaCommandSourceConfig,
}

impl TlsConfig for Config {
    fn is_disabled(&self) -> bool {
        self.disable_tls
    }

    fn verifier(&self) -> Arc<dyn ClientCertVerifier> {
        // This seems dangerous, as we simply accept all client certificates. However,
        // we validate them later during the "connect" packet validation.
        Arc::new(AcceptAllClientCertVerifier)
    }

    fn key_file(&self) -> Option<&str> {
        self.key_file.as_deref()
    }

    fn cert_bundle_file(&self) -> Option<&str> {
        self.cert_bundle_file.as_deref()
    }
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    let commands = Commands::new();

    let app = App {
        downstream: DownstreamSender::new(KafkaSink::new("DOWNSTREAM_KAFKA_SINK")?)?,
        authenticator: DeviceAuthenticator(
            drogue_cloud_endpoint_common::auth::DeviceAuthenticator::new().await?,
        ),
        commands: commands.clone(),
    };

    let addr = config.bind_addr_mqtt.as_deref();
    let srv = build(addr, app, &config)?.run();

    log::info!("Starting web server");

    // command source

    let command_source = KafkaCommandSource::new(commands, config.command_source_kafka)?;

    // run
    if let Some(health) = config.health {
        // health server
        let health = HealthServer::new(health, vec![Box::new(command_source)]);
        futures::try_join!(health.run_ntex(), srv.err_into(),)?;
    } else {
        futures::try_join!(srv)?;
    }

    // exiting

    Ok(())
}
