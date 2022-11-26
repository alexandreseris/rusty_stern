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
use termcolor::{ColorChoice, StandardStream};
use tokio;
use tokio::sync::Mutex;

use rusty_stern_traits::Update;

use display::{build_color_cycle, eprint_color, pick_color, print_color};
use error::Errors;
use kubernetes::{get_pods, print_log};
use settings::{create_default_config_file, Settings};

#[tokio::main]
async fn main() -> Result<(), Errors> {
    let stdout = StandardStream::stdout(ColorChoice::Always);
    let stderr = StandardStream::stderr(ColorChoice::Always);
    let stdout_lock = Arc::new(Mutex::new((stdout, stderr)));

    let mut settings = match Settings::from_config_file() {
        Ok(val) => val,
        Err(err) => {
            eprintln!("{err}");
            Settings { ..Default::default() }
        }
    };

    let args = Settings::do_parse();
    if args.generate_config_file {
        create_default_config_file()?;
        return Ok(());
    }

    settings.update_from(args);
    let settings = match settings.validate() {
        Ok(val) => val,
        Err(err) => return Err(Errors::Validation(err.to_string())),
    };

    let mut color_cycle_len = settings.color_cycle_len;

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
        print_color(stdout_lock.clone(), settings.debug_color, "starting".to_string(), true).await?;
    }

    let client = match settings.kubeconfig {
        Some(val) => {
            let kconf = match Kubeconfig::read_from(val) {
                Ok(val) => val,
                Err(err) => return Err(Errors::Kubernetes("failled to read config file".to_string(), err.to_string())),
            };
            let kconfopt = &KubeConfigOptions::default();
            let conf = match Config::from_custom_kubeconfig(kconf, kconfopt).await {
                Ok(val) => val,
                Err(err) => return Err(Errors::Kubernetes("failled to parse config file".to_string(), err.to_string())),
            };
            match Client::try_from(conf) {
                Ok(val) => val,
                Err(err) => return Err(Errors::Kubernetes("failled to use kubernetes configuration".to_string(), err.to_string())),
            }
        }
        None => match Client::try_default().await {
            Ok(val) => val,
            Err(err) => {
                return Err(Errors::Kubernetes(
                    "failled to use default kubernetes configuration".to_string(),
                    err.to_string(),
                ))
            }
        },
    };

    let pods_api = match settings.namespace {
        Some(val) => Api::namespaced(client, &val),
        None => Api::default_namespaced(client),
    };

    let running_pods: HashSet<String> = HashSet::new();
    let running_pods_lock = Arc::new(Mutex::new(running_pods));

    let mut pods = get_pods(pods_api.clone(), settings.pod_search.clone()).await?;
    if color_cycle_len == 0 {
        color_cycle_len = pods.len() as u8 + (pods.len() as f32 * 0.5 as f32) as u8;
    }
    let mut color_cycle = build_color_cycle(
        color_cycle_len,
        settings.color_saturation,
        settings.color_lightness,
        settings.hue_intervals,
    )?;
    loop {
        if settings.verbose && pods.len() == 0 {
            eprint_color(stdout_lock.clone(), settings.debug_color, "no pod found :(".to_string(), true).await?;
        }
        for pod in pods.clone() {
            let name = match pod.metadata.name {
                Some(val) => val,
                None => return Err(Errors::Kubernetes("pod has no name".to_string(), "(no details)".to_string())),
            };
            let pod_is_already_running = {
                let running_pods_locked = running_pods_lock.lock().await;
                running_pods_locked.contains(name.as_str())
            };
            let phase = match pod.status {
                Some(status) => match status.phase {
                    Some(phase) => phase,
                    None => return Err(Errors::Kubernetes(format!("pod {name} has no phase"), "(no details)".to_string())),
                },
                None => return Err(Errors::Kubernetes(format!("pod {name} has no status"), "(no details)".to_string())),
            };
            if !pod_is_already_running && phase == "Running".to_string() {
                let pods_api_cp = pods_api.clone();
                let name_cp = name.clone();
                let color = pick_color(&mut color_cycle).clone();
                let stdout_lock = stdout_lock.clone();
                let running_pods_lock = running_pods_lock.clone();
                let params = params.clone();
                tokio::spawn(async move {
                    let print_res = print_log(
                        stdout_lock,
                        pods_api_cp.clone(),
                        name_cp,
                        color,
                        running_pods_lock.clone(),
                        params.clone(),
                    )
                    .await;
                    match print_res {
                        Ok(_) => Ok(()),
                        Err(err) => return Err(Errors::Other(err.to_string())),
                    }
                });
            }
        }
        if !settings.disable_pods_refresh {
            tokio::time::sleep(tokio::time::Duration::from_millis(settings.loop_pause * 1000)).await;
            pods = get_pods(pods_api.clone(), settings.pod_search.clone()).await?;
        }
    }
}
