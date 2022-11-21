use std::collections::HashSet;
use std::str;
use std::sync::Arc;

use bytes::Bytes;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{ListParams, LogParams};
use kube::Api;
use regex::Regex;
use termcolor::StandardStream;
use tokio::sync::Mutex;

use crate::display::{get_padding, print_color, ColorRGB};
use crate::error::LogError;

pub async fn get_pods(pods_api: Api<Pod>, pod_search: Regex) -> Vec<Pod> {
    let mut filt_pods: Vec<Pod> = Vec::new();
    let pods = pods_api.list(&ListParams::default()).await.unwrap();
    for pod in pods {
        let cloned_pod = pod.clone();
        if pod_search.is_match(pod.metadata.name.unwrap().as_str()) {
            filt_pods.push(cloned_pod);
        }
    }
    return filt_pods;
}

pub async fn print_log(
    stdout_lock: Arc<Mutex<(StandardStream, StandardStream)>>,
    pods_api: Api<Pod>,
    name: String,
    color_rgb: ColorRGB,
    running_pods: Arc<Mutex<HashSet<String>>>,
    params: LogParams,
) -> Result<(), LogError> {
    let pod_count = {
        let mut running_pods_locked = running_pods.lock().await;
        running_pods_locked.insert(name.clone());
        running_pods_locked.len()
    };
    print_color(
        stdout_lock.clone(),
        color_rgb,
        format!("+ pod {} starting, following {} pods", name, pod_count),
        true,
    )
    .await;
    let mut stream = match pods_api.log_stream(&name, &params).await {
        Ok(stream) => stream,
        Err(err) => return Err(LogError { message: err.to_string() }),
    };
    let mut line_bytes: Bytes;
    loop {
        line_bytes = stream.next().await.unwrap_or(Ok(Bytes::from(""))).unwrap_or(Bytes::from(""));
        if line_bytes == Bytes::from("") {
            break;
        }
        let content = match str::from_utf8(line_bytes.iter().as_slice()) {
            Ok(content) => content,
            Err(err) => return Err(LogError { message: err.to_string() }),
        };
        let padding = " ".repeat(get_padding(running_pods.clone()).await - name.len());
        print_color(stdout_lock.clone(), color_rgb, format!("{}:{} {}", name, padding, content), false).await;
    }
    let pod_count = {
        let mut running_pods_locked = running_pods.lock().await;
        running_pods_locked.remove(&name.clone());
        running_pods_locked.len()
    };
    print_color(
        stdout_lock.clone(),
        color_rgb,
        format!("- pod {} ended, following {} pods", name, pod_count),
        true,
    )
    .await;
    Ok(())
}
