use std::collections::HashSet;
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
use tokio::sync::Mutex;

use crate::display::{get_padding, print_color};
use crate::error::Errors;

pub async fn get_pods(pods_api: Api<Pod>, pod_search: Regex) -> Result<Vec<Pod>, Errors> {
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
    color_rgb: Rgb,
    running_pods: Arc<Mutex<HashSet<String>>>,
    params: LogParams,
) -> Result<(), Errors> {
    let pod_count = {
        let mut running_pods_locked = running_pods.lock().await;
        running_pods_locked.insert(name.clone());
        running_pods_locked.len()
    };
    print_color(
        stdout_lock.clone(),
        color_rgb,
        format!("+ pod {name} starting, following {pod_count} pods"),
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
        let padding = " ".repeat(get_padding(running_pods.clone()).await - name.len());
        print_color(stdout_lock.clone(), color_rgb, format!("{name}:{padding} {content}"), false).await?;
    }
    let pod_count = {
        let mut running_pods_locked = running_pods.lock().await;
        running_pods_locked.remove(&name.clone());
        running_pods_locked.len()
    };
    let error_reason = match error {
        Some(value) => format!(", reason: {}", value.to_string()),
        None => "".to_string(),
    };
    print_color(
        stdout_lock.clone(),
        color_rgb,
        format!("- pod {name} ended{error_reason}, following {pod_count} pods"),
        true,
    )
    .await?;
    Ok(())
}
