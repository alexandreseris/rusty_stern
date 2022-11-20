#![allow(unused_parens)]
mod display;
mod error;
mod kubernetes;

use std::collections::HashSet;
use std::sync::Arc;

use clap::Parser;
use kube::{
    api::LogParams,
    config::{KubeConfigOptions, Kubeconfig},
    Api, Client, Config,
};
use regex::Regex;
use termcolor::{ColorChoice, StandardStream};
use tokio;
use tokio::sync::Mutex;

use display::{build_color_cycle, pick_color, print_color, ColorRGB};
use error::GenericError;
use kubernetes::{get_pods, print_log};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// regex to match pod names
    #[arg(short, long, value_name="reg pattern", default_value_t = (".+".to_string()))]
    pub pod_search: String,
    /// path to the kubeconfig file. if the option is not passed, try to infer configuration
    #[arg(short, long, value_name="filepath", default_value_t = ("".to_string()))]
    pub kubeconfig: String,
    /// kubernetes namespace to use. if the option is not passed, use the default namespace
    #[arg(short, long, value_name="nmspc", default_value_t = ("".to_string()))]
    pub namespace: String,

    /// retrieve previous terminated container logs
    #[arg(long, default_value_t = false)]
    pub previous: bool,
    /// a relative time in seconds before the current time from which to show logs
    #[arg(long, value_name = "seconds", default_value_t = 0)]
    pub since_seconds: i64,
    /// number of lines from the end of the logs to show
    #[arg(long, value_name = "line_cnt", default_value_t = 0)]
    pub tail_lines: i64,
    /// show timestamp at the begining of each log line
    #[arg(long, default_value_t = false)]
    pub timestamps: bool,

    /// number of seconds between each pod list query (doesn't affect log line display)
    #[arg(long, value_name = "seconds", default_value_t = 2)]
    pub loop_pause: u64,

    /// verbose output
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,

    /// debug rgb color (format is 0-255,0-255,0-255)
    #[arg(long, value_name="rgb", default_value_t = ("255,255,255".to_string()))]
    pub debug_color: String,
    /// number of color to generate for the color cycle. if 0, it is later set for about the number of result retuned by the first pod search
    #[arg(long, value_name = "num", default_value_t = 0)]
    pub color_cycle_len: u8,
    /// the color saturation (0-100)
    #[arg(long, value_name = "sat", default_value_t = 100)]
    pub color_saturation: u8,
    /// the color lightness (0-100)
    #[arg(long, value_name = "light", default_value_t = 50)]
    pub color_lightness: u8,
}

#[tokio::main]
async fn main() -> Result<(), GenericError> {
    let args = Args::parse();

    let mut color_cycle_len = args.color_cycle_len;
    let namespace = args.namespace.as_str();
    let verbose = args.verbose;
    let debug_color = match args.debug_color.as_str().parse::<ColorRGB>() {
        Ok(debug_color) => debug_color,
        Err(err) => return Err(GenericError::new(err.to_string())),
    };
    let pod_search = args.pod_search.as_str();
    let color_saturation = args.color_saturation;
    let color_lightness = args.color_lightness;
    let loop_pause = args.loop_pause * 1000;

    let params = LogParams {
        container: None,
        follow: true,
        limit_bytes: None,
        pretty: false,
        previous: args.previous,
        since_seconds: {
            if args.since_seconds == 0 {
                None
            } else {
                Some(args.since_seconds)
            }
        },
        tail_lines: Some(args.tail_lines),
        timestamps: args.timestamps,
    };

    let stdout = StandardStream::stdout(ColorChoice::Always);
    let stdout_lock = Arc::new(Mutex::new(stdout));
    if verbose {
        print_color(
            stdout_lock.clone(),
            debug_color,
            "starting".to_string(),
            true,
        )
        .await;
    }

    let pod_reg = Regex::new(pod_search).unwrap();

    let client = if args.kubeconfig != "" {
        let kconf = Kubeconfig::read_from(args.kubeconfig).unwrap();
        let kconfopt = &KubeConfigOptions::default();
        let conf = Config::from_custom_kubeconfig(kconf, kconfopt)
            .await
            .unwrap();
        Client::try_from(conf).unwrap()
    } else {
        Client::try_default().await.unwrap()
    };

    let pods_api = if namespace == "" {
        Api::default_namespaced(client)
    } else {
        Api::namespaced(client, &namespace)
    };

    let running_pods: HashSet<String> = HashSet::new();
    let running_pods_lock = Arc::new(Mutex::new(running_pods));

    let mut pods = get_pods(pods_api.clone(), pod_reg.clone()).await;
    if color_cycle_len == 0 {
        color_cycle_len = pods.len() as u8 + (pods.len() as f32 * 0.5 as f32) as u8;
    }
    let mut color_cycle =
        build_color_cycle(color_cycle_len, color_saturation, color_lightness).unwrap();
    loop {
        if verbose && pods.len() == 0 {
            print_color(
                stdout_lock.clone(),
                debug_color,
                "no pod found :(".to_string(),
                true,
            )
            .await;
        }
        for pod in pods {
            let name = pod.metadata.name.unwrap();
            let pod_is_already_running = {
                let running_pods_locked = running_pods_lock.lock().await;
                running_pods_locked.contains(name.as_str())
            };
            if !pod_is_already_running
                && pod.status.unwrap().phase.unwrap() == "Running".to_string()
            {
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
        tokio::time::sleep(tokio::time::Duration::from_millis(loop_pause)).await;
        pods = get_pods(pods_api.clone(), pod_reg.clone()).await;
    }
}
