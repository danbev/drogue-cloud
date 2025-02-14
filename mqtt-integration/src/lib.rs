mod service;

use crate::service::ServiceConfig;
use anyhow::Context;
use drogue_cloud_endpoint_common::{sender::UpstreamSender, sink::KafkaSink};
use drogue_cloud_mqtt_common::server::{build, TlsConfig};
use drogue_cloud_service_common::{
    client::{RegistryConfig, UserAuthClient, UserAuthClientConfig},
    health::{HealthServer, HealthServerConfig},
    openid::Authenticator,
};
use futures::TryFutureExt;
use serde::Deserialize;
use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};

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
    pub registry: Option<RegistryConfig>,

    pub max_size: Option<u32>,

    #[serde(default)]
    pub service: ServiceConfig,

    #[serde(default)]
    pub user_auth: Option<UserAuthClientConfig>,

    #[serde(default)]
    pub health: Option<HealthServerConfig>,
}

impl TlsConfig for Config {
    fn is_disabled(&self) -> bool {
        self.disable_tls
    }

    fn key_file(&self) -> Option<&str> {
        self.key_file.as_deref()
    }

    fn cert_bundle_file(&self) -> Option<&str> {
        self.cert_bundle_file.as_deref()
    }
}

#[derive(Clone)]
pub struct OpenIdClient {
    pub client: openid::Client,
}

impl Debug for OpenIdClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OpenIdClient")
            .field("client", &"...")
            .finish()
    }
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    let app_config = config.clone();

    log::info!(
        "User/password enabled: {}",
        config.service.enable_username_password_auth
    );
    log::info!("Kafka servers: {}", config.service.kafka.bootstrap_servers);

    // set up security

    let (authenticator, user_auth) = if let Some(user_auth) = config.user_auth {
        let client = reqwest::Client::new();
        let authenticator = Authenticator::new().await?;
        let user_auth = Arc::new(UserAuthClient::from_config(client, user_auth).await?);
        (Some(authenticator), Some(user_auth))
    } else {
        (None, None)
    };

    let client = reqwest::Client::new();
    let registry = config
        .registry
        .as_ref()
        .cloned()
        .context("no registry configured")?
        .into_client(client.clone())
        .await?;

    let sender = UpstreamSender::new(KafkaSink::new("COMMAND_KAFKA_SINK")?)?;

    // creating the application

    let app = service::App {
        authenticator,
        user_auth,
        config: config.service.clone(),
        sender,
        client,
        registry,
    };

    // create server

    let addr = config.bind_addr_mqtt.as_deref();
    let srv = build(addr, app, &app_config)?.run();

    log::info!("Starting server");

    // run

    if let Some(health) = config.health {
        // health server
        let health = HealthServer::new(health, vec![]);
        futures::try_join!(health.run_ntex(), srv.err_into(),)?;
    } else {
        futures::try_join!(srv)?;
    }

    // exiting

    Ok(())
}
