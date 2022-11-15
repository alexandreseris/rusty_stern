use std::collections::HashMap;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Write;
use std::str;
use std::sync::Arc;
use std::sync::Mutex;

use bytes::Bytes;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::api::ListParams;
use kube::api::LogParams;
use kube::core::ObjectList;
use kube::Api;
use kube::Client;
use termcolor::WriteColor;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream};
use tokio;

#[derive(Debug, Copy, Clone)]
struct ColorRGB(u8, u8, u8);

async fn print_color(
    stdout: Arc<Mutex<StandardStream>>,
    color_rgb: ColorRGB,
    message: String,
    newline: bool,
) {
    let mut stdout_locked = stdout.lock().unwrap();
    stdout_locked
        .set_color(ColorSpec::new().set_fg(Some(Color::Rgb(color_rgb.0, color_rgb.1, color_rgb.2))))
        .unwrap();
    if newline {
        stdout_locked
            .write_fmt(format_args!("{}\n", message))
            .unwrap();
    } else {
        stdout_locked
            .write_fmt(format_args!("{}", message))
            .unwrap();
    }
}

fn pick_color() -> ColorRGB {
    ColorRGB(250, 205, 0) // dumdum function :|
}

async fn get_pods(pods_api: Api<Pod>) -> Result<ObjectList<Pod>, kube::Error> {
    let pods = pods_api.list(&ListParams::default()).await;
    return pods;
}

async fn print_log(
    stdout_lock: Arc<Mutex<StandardStream>>,
    pods_api: Api<Pod>,
    name: String,
    color_rgb: ColorRGB,
) -> Result<(), Error> {
    let params = LogParams {
        container: None,
        follow: true,
        limit_bytes: None,
        pretty: false,
        previous: false,
        since_seconds: None,
        tail_lines: Some(0),
        timestamps: true,
    };
    print_color(
        stdout_lock.clone(),
        color_rgb,
        format!("pod {} starting", name),
        true,
    )
    .await;
    let mut stream = match pods_api.log_stream(&name, &params).await {
        Ok(stream) => stream,
        Err(err) => return Err(Error::new(ErrorKind::Other, err)),
    };
    let mut line_bytes: Bytes;
    loop {
        line_bytes = stream
            .next()
            .await
            .unwrap_or(Ok(Bytes::from("")))
            .unwrap_or(Bytes::from(""));
        if line_bytes == Bytes::from("") {
            break;
        }
        let content = match str::from_utf8(line_bytes.iter().as_slice()) {
            Ok(content) => content,
            Err(err) => return Err(Error::new(ErrorKind::Other, err)),
        };
        print_color(
            stdout_lock.clone(),
            color_rgb,
            format!("{}: {}", name, content),
            false,
        )
        .await;
    }
    print_color(
        stdout_lock.clone(),
        color_rgb,
        format!("pod {} ended", name),
        false,
    )
    .await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let namespace = "";

    let stdout = StandardStream::stdout(ColorChoice::Always);
    let stdout_lock = Arc::new(Mutex::new(stdout));

    let client = Client::try_default().await.unwrap();
    let pods_api: Api<Pod>;
    if namespace == "" {
        pods_api = Api::default_namespaced(client);
    } else {
        pods_api = Api::namespaced(client, &namespace);
    }
    let mut processes: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();
    loop {
        let pods = get_pods(pods_api.clone()).await.unwrap();
        let mut to_remove: Vec<String> = vec![]; // ugly af
        for (name, handle) in processes.iter() {
            if handle.is_finished() {
                to_remove.push(name.clone());
            }
        }
        for process in to_remove {
            processes.remove(&process);
        }
        for pod in pods {
            let name = pod.metadata.name.unwrap();
            if !processes.contains_key(&name)
                && pod.status.unwrap().phase.unwrap() == "Running".to_string()
            {
                let pods_api_cp = pods_api.clone();
                let name_cp = name.clone();
                let stdout_lock = stdout_lock.clone();
                let handle = tokio::spawn(async move {
                    print_log(stdout_lock, pods_api_cp.clone(), name_cp, pick_color())
                        .await
                        .unwrap();
                });
                processes.insert(name.clone(), handle);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(2 * 1000)).await;
    }
}
