use std::collections::{HashMap, HashSet};
use std::str;
use std::sync::Arc;

use bytes::Bytes;
use colors_transform::Rgb;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{ListParams, LogParams};
use kube::Api;
use regex::Regex;
use termcolor::StandardStream;
use tokio::sync::{Mutex, MutexGuard};

use crate::display::{get_padding, print_color};
use crate::error::Errors;

fn get_pod_count_from_mutex(namespaces: MutexGuard<HashMap<String, HashSet<String>>>) -> usize {
    let mut cnt = 0;
    for (_, pods) in namespaces.iter() {
        cnt += pods.len();
    }
    return cnt;
}

pub fn get_pod_count(namespaces: &HashMap<String, (Api<Pod>, Vec<Pod>)>) -> usize {
    let mut cnt = 0;
    for (_, (_, pods)) in namespaces {
        cnt += pods.len();
    }
    return cnt;
}

pub async fn refresh_namespaces_pods(namespaces: &mut HashMap<String, (Api<Pod>, Vec<Pod>)>, pod_search: Regex) -> Result<(), Errors> {
    for (namespace, (pod_api, _)) in namespaces.clone() {
        let refreshed_pods = get_namespace_pods(pod_api.clone(), pod_search.clone()).await?;
        namespaces.insert(namespace, (pod_api, refreshed_pods));
    }
    Ok(())
}

pub async fn get_namespace_pods(pods_api: Api<Pod>, pod_search: Regex) -> Result<Vec<Pod>, Errors> {
    let mut filt_pods: Vec<Pod> = Vec::new();
    let pods = match pods_api.list(&ListParams::default()).await {
        Ok(val) => val,
        Err(err) => return Err(Errors::Kubernetes("failled to retrieve pods".to_string(), err.to_string())),
    };
    for pod in pods {
        let cloned_pod = pod.clone();
        let name = get_pod_name(pod)?;
        if pod_search.is_match(name.as_str()) {
            filt_pods.push(cloned_pod);
        }
    }
    return Ok(filt_pods);
}

pub fn get_pod_name(pod: Pod) -> Result<String, Errors> {
    match pod.metadata.name {
        Some(val) => Ok(val),
        None => return Err(Errors::Kubernetes("pod has no name".to_string(), "(no detail)".to_string())),
    }
}

pub fn get_pod_status(pod: Pod) -> Result<String, Errors> {
    match pod.status {
        Some(val) => match val.phase {
            Some(val) => Ok(val),
            None => return Err(Errors::Kubernetes("pod has no phase".to_string(), "(no detail)".to_string())),
        },
        None => return Err(Errors::Kubernetes("pod has no status".to_string(), "(no detail)".to_string())),
    }
}

async fn is_pod_running(pods_api: Api<Pod>, pod_name: String) -> bool {
    match pods_api.get_status(pod_name.as_str()).await {
        Ok(val) => match get_pod_status(val) {
            Ok(val) => val == "Running",
            Err(_) => false,
        },
        Err(_) => false,
    }
}

pub async fn print_log(
    stdout_lock: Arc<Mutex<(StandardStream, StandardStream)>>,
    pods_api: Api<Pod>,
    name: String,
    namespace: String,
    color_rgb: Rgb,
    running_pods: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    params: LogParams,
) -> Result<(), Errors> {
    let pod_count = {
        let mut running_pods_locked = running_pods.lock().await;
        match running_pods_locked.get_mut(&namespace) {
            Some(val) => val.insert(name.clone()),
            None => return Err(Errors::Other("shared running pods have inconsistent state".to_string())),
        };
        get_pod_count_from_mutex(running_pods_locked)
    };
    print_color(
        stdout_lock.clone(),
        color_rgb,
        format!("+++ pod {namespace}/{name} starting, following {pod_count} pods"),
        true,
    )
    .await?;
    let mut stream = match pods_api.log_stream(&name, &params).await {
        Ok(stream) => stream,
        Err(err) => return Err(Errors::LogError(err.to_string())),
    };
    let mut line_bytes: Bytes;
    let mut error = None;
    loop {
        let next = match stream.next().await {
            Some(val) => val,
            None => Ok(Bytes::from("")),
        };
        line_bytes = match next {
            Ok(val) => val,
            Err(err) => {
                error = Some(Errors::Kubernetes("failled to retrieve logs".to_string(), err.to_string()));
                break;
            }
        };
        if line_bytes == Bytes::from("") {
            if is_pod_running(pods_api.clone(), name.clone()).await {
                continue;
            }
            break;
        }
        let content = match str::from_utf8(line_bytes.iter().as_slice()) {
            Ok(content) => content,
            Err(err) => return Err(Errors::LogError(err.to_string())),
        };

        let (padding, print_namespace) = get_padding(running_pods.clone()).await;
        let message: String;
        if print_namespace {
            let padding_str = " ".repeat(padding - name.len() - namespace.len() + 1);
            message = format!("{namespace}/{name}:{padding_str} {content}");
        } else {
            let padding_str = " ".repeat(padding - name.len());
            message = format!("{name}:{padding_str} {content}");
        }

        print_color(stdout_lock.clone(), color_rgb, message, false).await?;
    }
    let pod_count = {
        let mut running_pods_locked = running_pods.lock().await;
        match running_pods_locked.get_mut(&namespace) {
            Some(val) => val.remove(&name.clone()),
            None => return Err(Errors::Other("shared running pods have inconsistent state".to_string())),
        };
        get_pod_count_from_mutex(running_pods_locked)
    };
    let error_reason = match error {
        Some(value) => format!(", reason: {}", value.to_string()),
        None => "".to_string(),
    };
    print_color(
        stdout_lock.clone(),
        color_rgb,
        format!("--- pod {namespace}/{name} ended{error_reason}, following {pod_count} pods"),
        true,
    )
    .await?;
    Ok(())
}
