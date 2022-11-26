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
        let name = match pod.metadata.name {
            Some(val) => val,
            None => return Err(Errors::Kubernetes("pod has no name".to_string(), "(no details)".to_string())),
        };
        if pod_search.is_match(name.as_str()) {
            filt_pods.push(cloned_pod);
        }
    }
    return Ok(filt_pods);
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
    let error;
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
