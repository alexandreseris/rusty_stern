mod display;
mod error;
mod kubernetes;
mod settings;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::LogParams,
    config::{KubeConfigOptions, Kubeconfig},
    Api, Client, Config,
};
use termcolor::{ColorChoice, StandardStream};
use tokio;
use tokio::sync::Mutex;

use display::{build_color_cycle, eprint_color, pick_color, print_color};
use error::Errors;
use kubernetes::{get_pod_count, get_pod_name, get_pod_status, print_log, refresh_namespaces_pods};
use settings::Settings;

#[allow(deprecated)]
#[tokio::main]
async fn main() -> Result<(), Errors> {
    let stdout = StandardStream::stdout(ColorChoice::Always);
    let stderr = StandardStream::stderr(ColorChoice::Always);
    let stdout_lock = Arc::new(Mutex::new((stdout, stderr)));

    let settings = Settings::do_parse();

    let settings = match settings.to_validated() {
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

    let mut conf = match settings.kubeconfig {
        Some(val) => {
            let kconf = match Kubeconfig::read_from(val) {
                Ok(val) => val,
                Err(err) => return Err(Errors::Kubernetes("failled to read config file".to_string(), err.to_string())),
            };
            let kconfopt = &KubeConfigOptions::default();
            match Config::from_custom_kubeconfig(kconf, kconfopt).await {
                Ok(val) => val,
                Err(err) => return Err(Errors::Kubernetes("failled to parse config file".to_string(), err.to_string())),
            }
        }
        None => match Config::infer().await {
            Ok(val) => val,
            Err(err) => return Err(Errors::Kubernetes("failled to get default config".to_string(), err.to_string())),
        },
    };
    conf.read_timeout = None;
    conf.write_timeout = None;
    conf.connect_timeout = None;
    conf.timeout = None;

    let client = match Client::try_from(conf) {
        Ok(val) => val,
        Err(err) => return Err(Errors::Kubernetes("failled to use kubernetes configuration".to_string(), err.to_string())),
    };

    // namespace => [pod_name, ...]
    let mut running_pods: HashMap<String, HashSet<String>> = HashMap::new();

    // namespace => (api, [pod, ...])
    let mut namespaces: HashMap<String, (Api<Pod>, Vec<Pod>)> = HashMap::new();
    match settings.namespace {
        Some(val) => {
            for namespace in val {
                namespaces.insert(namespace.clone(), (Api::namespaced(client.clone(), &namespace.clone()), Vec::new()));
                running_pods.insert(namespace.clone(), HashSet::new());
            }
        }
        None => {
            let namespace = "default".to_string();
            namespaces.insert(namespace.clone(), (Api::default_namespaced(client), Vec::new()));
            running_pods.insert(namespace.clone(), HashSet::new());
        }
    };

    let running_pods_lock = Arc::new(Mutex::new(running_pods));

    refresh_namespaces_pods(&mut namespaces, settings.pod_search.clone()).await?;

    print_color(
        stdout_lock.clone(),
        settings.default_color,
        format!(
            "initial search found {} pods across {} namespaces",
            get_pod_count(&namespaces),
            namespaces.len()
        ),
    )
    .await?;

    if color_cycle_len == 0 {
        color_cycle_len = get_pod_count(&namespaces) as u8;
    }
    let mut color_cycle = build_color_cycle(
        color_cycle_len,
        settings.color_saturation,
        settings.color_lightness,
        settings.hue_intervals,
    )?;
    let mut no_pod_found = false;
    loop {
        if !settings.disable_pods_refresh {
            tokio::time::sleep(tokio::time::Duration::from_millis(settings.loop_pause * 1000)).await;
            refresh_namespaces_pods(&mut namespaces, settings.pod_search.clone()).await?;
        }
        let pods_cnt = get_pod_count(&namespaces);
        if pods_cnt == 0 && !no_pod_found {
            eprint_color(stdout_lock.clone(), settings.default_color, "no pod found :(".to_string()).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(settings.loop_pause * 1000)).await;
            no_pod_found = true;
        }
        if no_pod_found {
            continue;
        }
        no_pod_found = false;
        for (namespace, (pod_api, pods)) in namespaces.clone() {
            for pod in pods {
                let name = get_pod_name(pod.clone())?;
                let pod_is_already_running = {
                    let running_pods_locked = running_pods_lock.lock().await;
                    match running_pods_locked.get(&namespace) {
                        Some(val) => val.contains(name.as_str()),
                        None => return Err(Errors::Other("shared running pods have inconsistent state".to_string())),
                    }
                };
                let phase = get_pod_status(pod)?;
                if !pod_is_already_running && phase == "Running".to_string() {
                    let pods_api_cp = pod_api.clone();
                    let name_cp = name.clone();
                    let namespace_cp = namespace.clone();
                    let color = pick_color(&mut color_cycle).clone();
                    let stdout_lock = stdout_lock.clone();
                    let running_pods_lock = running_pods_lock.clone();
                    let params = params.clone();
                    tokio::spawn(async move {
                        let print_res = print_log(
                            stdout_lock.clone(),
                            pods_api_cp.clone(),
                            name_cp,
                            namespace_cp,
                            color,
                            running_pods_lock.clone(),
                            params.clone(),
                        )
                        .await;
                        match print_res {
                            Ok(_) => Ok(()),
                            Err(err) => {
                                let error = Errors::Other(err.to_string());
                                eprint_color(stdout_lock, settings.default_color, error.to_string()).await?;
                                return Err(error);
                            }
                        }
                    });
                }
            }
        }
    }
}
