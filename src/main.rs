#![allow(unused_parens)]
mod display;
mod error;
mod kubernetes;
mod settings;

use std::collections::HashSet;
use std::sync::Arc;

use kube::{
    api::LogParams,
    config::{KubeConfigOptions, Kubeconfig},
    Api, Client, Config,
};
use regex::Regex;
use termcolor::{ColorChoice, StandardStream};
use tokio;
use tokio::sync::Mutex;

use display::{build_color_cycle, eprint_color, pick_color, print_color};
use error::GenericError;
use kubernetes::{get_pods, print_log};
use settings::{create_default_config_file, Settings};

#[tokio::main]
async fn main() -> Result<(), GenericError> {
    let stdout = StandardStream::stdout(ColorChoice::Always);
    let stderr = StandardStream::stderr(ColorChoice::Always);
    let stdout_lock = Arc::new(Mutex::new((stdout, stderr)));

    let mut settings = {
        let file_settings = Settings::from_config_file();
        if file_settings.is_ok() {
            file_settings.unwrap()
        } else {
            eprintln!("{}", file_settings.unwrap_err().to_string());
            Settings { ..Default::default() }
        }
    };

    let args = Settings::do_parse();
    if args.generate_config_file {
        create_default_config_file().unwrap();
        return Ok(());
    }

    settings.update(args);

    let mut color_cycle_len = settings.color_cycle_len;
    let debug_color = match settings.clone().get_debug_color() {
        Ok(debug_color) => debug_color,
        Err(err) => return Err(GenericError { message: err.to_string() }),
    };

    let params = LogParams {
        container: None,
        follow: true,
        limit_bytes: None,
        pretty: false,
        previous: settings.previous,
        since_seconds: {
            if settings.since_seconds == 0 {
                None
            } else {
                Some(settings.since_seconds)
            }
        },
        tail_lines: Some(settings.tail_lines),
        timestamps: settings.timestamps,
    };

    if settings.verbose {
        print_color(stdout_lock.clone(), debug_color, "starting".to_string(), true).await;
    }

    let pod_reg = Regex::new(settings.pod_search.as_str()).unwrap();

    let client = if settings.kubeconfig != "" {
        let kconf = Kubeconfig::read_from(settings.kubeconfig).unwrap();
        let kconfopt = &KubeConfigOptions::default();
        let conf = Config::from_custom_kubeconfig(kconf, kconfopt).await.unwrap();
        Client::try_from(conf).unwrap()
    } else {
        Client::try_default().await.unwrap()
    };

    let pods_api = if settings.namespace == "" {
        Api::default_namespaced(client)
    } else {
        Api::namespaced(client, &settings.namespace)
    };

    let running_pods: HashSet<String> = HashSet::new();
    let running_pods_lock = Arc::new(Mutex::new(running_pods));

    let mut pods = get_pods(pods_api.clone(), pod_reg.clone()).await;
    if color_cycle_len == 0 {
        color_cycle_len = pods.len() as u8 + (pods.len() as f32 * 0.5 as f32) as u8;
    }
    let mut color_cycle = build_color_cycle(color_cycle_len, settings.color_saturation, settings.color_lightness).unwrap();
    loop {
        if settings.verbose && pods.len() == 0 {
            eprint_color(stdout_lock.clone(), debug_color, "no pod found :(".to_string(), true).await;
        }
        for pod in pods {
            let name = pod.metadata.name.unwrap();
            let pod_is_already_running = {
                let running_pods_locked = running_pods_lock.lock().await;
                running_pods_locked.contains(name.as_str())
            };
            if !pod_is_already_running && pod.status.unwrap().phase.unwrap() == "Running".to_string() {
                let pods_api_cp = pods_api.clone();
                let name_cp = name.clone();
                let color = pick_color(&mut color_cycle).clone();
                let stdout_lock = stdout_lock.clone();
                let running_pods_lock = running_pods_lock.clone();
                let params = params.clone();
                tokio::spawn(async move {
                    print_log(
                        stdout_lock,
                        pods_api_cp.clone(),
                        name_cp,
                        color,
                        running_pods_lock.clone(),
                        params.clone(),
                    )
                    .await
                    .unwrap();
                });
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(settings.loop_pause * 1000)).await;
        pods = get_pods(pods_api.clone(), pod_reg.clone()).await;
    }
}
