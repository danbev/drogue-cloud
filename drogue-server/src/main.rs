use clap::{App, Arg, SubCommand};
use drogue_cloud_service_common::config::ConfigFromEnv;

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

        let mut threads = Vec::new();
        if matches.is_present("registry") {
            println!("Starting REGISTRY");
            let t = std::thread::spawn(move || {
                let mut config =
                    drogue_cloud_device_management_service::Config::from_env().unwrap();
                config.bind_addr = "127.0.0.1:10001".into();
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
        }

        if matches.is_present("console-backend") {
            let t = std::thread::spawn(move || {
                let mut config = drogue_cloud_console_backend::Config::from_env().unwrap();
                config.bind_addr = "127.0.0.1:10000".into();
                actix_rt::System::with_tokio_rt(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .worker_threads(1)
                        .thread_name("console-server")
                        .build()
                        .unwrap()
                })
                .block_on(drogue_cloud_console_backend::run(config))
                .unwrap();
            });
            threads.push(t);
        }

        if matches.is_present("mqtt-endpoint") {
            let t = std::thread::spawn(move || {
                let mut config = drogue_cloud_mqtt_endpoint::Config::from_env().unwrap();
                config.bind_addr_mqtt = Some("127.0.0.1:10002".into());
                config.bind_addr_http = "127.0.0.1:10003".into();
                ntex::rt::System::new("mqtt-endpoint")
                    .block_on(drogue_cloud_mqtt_endpoint::run(config))
                    .unwrap();
            });
            threads.push(t);
        }
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
