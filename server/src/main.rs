use clap::{App, Arg, SubCommand};
use drogue_cloud_api_key_service::{
    endpoints as keys,
    service::{KeycloakApiKeyService, KeycloakApiKeyServiceConfig},
};
use drogue_cloud_endpoint_common::command::KafkaCommandSourceConfig;
use drogue_cloud_event_common::config::{
    KafkaClientConfig as EventKafkaClientConfig, KafkaConfig as EventKafkaConfig,
};
use drogue_cloud_registry_events::sender::KafkaSenderConfig;
use drogue_cloud_service_api::{
    endpoints::*,
    kafka::{KafkaClientConfig, KafkaConfig},
};
use drogue_cloud_service_common::{client::UserAuthClientConfig, config::ConfigFromEnv};
use std::collections::HashMap;
use url::Url;

fn main() {
    env_logger::init();
    let matches = App::new("Drogue Cloud Server")
        .about("Server for all Drogue Cloud components")
        .subcommand(
            SubCommand::with_name("run")
                .about("run server")
                .arg(
                    Arg::with_name("debug")
                        .short("d")
                        .help("print debug information verbosely"),
                )
                .arg(
                    Arg::with_name("console-backend")
                        .long("--console-backend")
                        .help("run the console backend"),
                )
                .arg(
                    Arg::with_name("registry")
                        .long("--registry")
                        .help("run the registry backend"),
                )
                .arg(
                    Arg::with_name("mqtt-endpoint")
                        .long("--mqtt-endpoint")
                        .help("run the mqtt endpoint"),
                ),
        )
        .get_matches();

    // You can handle information about subcommands by requesting their matches by name
    // (as below), requesting just the name used, or both at the same time
    if let Some(matches) = matches.subcommand_matches("run") {
        if matches.is_present("debug") {
            println!("Printing debug info...");
        } else {
            println!("Printing normally...");
        }

        const KAFKA_BOOTSTRAP: &'static str = "localhost:9092";
        let endpoints = Endpoints {
            api: Some("http://localhost:10001".into()),
            console: Some("http://localhost:10000".into()),
            coap: Some(CoapEndpoint {
                url: "coap://localhost:5683".into(),
            }),
            http: Some(HttpEndpoint {
                url: "http://localhost:10002".into(),
            }),
            mqtt: Some(MqttEndpoint {
                host: "localhost".into(),
                port: 1883,
            }),
            mqtt_integration: Some(MqttEndpoint {
                host: "localhost".into(),
                port: 11883,
            }),
            websocket_integration: Some(HttpEndpoint {
                url: "http://localhost:10003".into(),
            }),
            sso: Some("http://localhost:8080".into()),
            issuer_url: Some("http://localhost:8080/auth".into()),
            redirect_url: Some("http://localhost:10000".into()),
            registry: Some(RegistryEndpoint {
                url: "http://localhost:10004".into(),
            }),
            command_url: Some("http://localhost:10005".into()),
            local_certs: false,
            kafka_bootstrap_servers: Some(KAFKA_BOOTSTRAP.into()),
        };
        let kafka_config = |topic: &str| KafkaConfig {
            client: KafkaClientConfig {
                bootstrap_servers: KAFKA_BOOTSTRAP.into(),
                properties: HashMap::new(),
            },
            topic: topic.to_string(),
        };

        let kafka_event_config = |topic: &str| EventKafkaConfig {
            client: EventKafkaClientConfig {
                bootstrap_servers: KAFKA_BOOTSTRAP.into(),
                properties: HashMap::new(),
            },
            topic: topic.to_string(),
        };

        let mut threads = Vec::new();
        //if matches.is_present("registry") {
        println!("Starting REGISTRY");
        let e = endpoints.clone();
        let t = std::thread::spawn(move || {
            let config = drogue_cloud_device_management_service::Config {
                enable_auth: false,
                health: None,
                bind_addr: e.registry.as_ref().unwrap().url.clone(),
                kafka_sender: KafkaSenderConfig {
                    client: kafka_event_config("registry"),
                    queue_timeout: None,
                },
            };
            actix_rt::System::with_tokio_rt(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .worker_threads(1)
                    .thread_name("registry")
                    .build()
                    .unwrap()
            })
            .block_on(drogue_cloud_device_management_service::run(config))
            .unwrap();
        });
        threads.push(t);
        //}

        //if matches.is_present("console-backend") {
        let e = endpoints.clone();
        let t = std::thread::spawn(move || {
            let config = drogue_cloud_console_backend::Config {
                health: None,
                enable_auth: false,
                disable_account_url: false,
                scopes: "user".into(),
                user_auth: UserAuthClientConfig {
                    url: Url::parse(&e.sso.as_ref().unwrap()).unwrap(),
                },
                keycloak: KeycloakApiKeyServiceConfig {
                    url: Url::parse(&e.sso.as_ref().unwrap()).unwrap(),
                    realm: "drogue".into(),
                    admin_username: "admin".into(),
                    admin_password: "admin123456".into(),
                    tls_noverify: true,
                },
                bind_addr: e.console.as_ref().unwrap().clone(),
                kafka: KafkaClientConfig {
                    bootstrap_servers: KAFKA_BOOTSTRAP.into(),
                    properties: HashMap::new(),
                },
            };
            actix_rt::System::with_tokio_rt(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .worker_threads(1)
                    .thread_name("console-server")
                    .build()
                    .unwrap()
            })
            .block_on(drogue_cloud_console_backend::run(config, e))
            .unwrap();
        });
        threads.push(t);
        //}

        //if matches.is_present("mqtt-endpoint") {
        let e = endpoints.clone();
        let t = std::thread::spawn(move || {
            let mqtt_endpoint = e.mqtt.as_ref().unwrap();
            let config = drogue_cloud_mqtt_endpoint::Config {
                health: None,
                disable_tls: true,
                cert_bundle_file: None,
                key_file: None,
                bind_addr_mqtt: Some(format!("{}:{}", mqtt_endpoint.host, mqtt_endpoint.port)),
                command_source_kafka: KafkaCommandSourceConfig {
                    kafka: kafka_config("iot-commands"),
                    consumer_group: "mqtt-endpoint".to_string(),
                },
            };

            ntex::rt::System::new("mqtt-endpoint")
                .block_on(drogue_cloud_mqtt_endpoint::run(config))
                .unwrap();
        });
        threads.push(t);
        //}
        for t in threads.drain(..) {
            println!("Joining {:?}", t);
            t.join().unwrap();
            println!("joined!");
        }
        println!("All threads finished");
    }
    /*
    let local = tokio::task::LocalSet::new();
    let sys = actix_web::rt::System::run_in_tokio("server", &local);
    let server_res = HttpServer::new(|| App::new().route("/", web::get().to(hello_world)))
        .bind("0.0.0.0:8000")?
        .run()
        .await?;
    sys.await?;
    Ok(server_res)
    */
}
