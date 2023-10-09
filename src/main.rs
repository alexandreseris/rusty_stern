mod display_v2;
mod error;
mod kubernetes_v2;
mod settings_v2;
mod types;

use crate::display_v2 as display;
use crate::error::Errors;
use crate::kubernetes_v2 as kubernetes;
use crate::settings_v2 as settings;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Errors> {
    let streams: display_v2::Streams = display::new_streams();
    let streams_lock = display::new_streams_mutex(streams);

    let settings = settings::Settings::do_parse();
    let settings = settings.to_validated()?;

    let log_params = kubernetes::new_log_param(&settings);
    let client = kubernetes::new_client(&settings).await?;

    let namespaces = kubernetes::Namespaces::new(&client, &settings.namespaces);
    let pod_cnt = namespaces.get_pods_cnt(&settings.pod_search).await?;
    let mut colors_params = display::ColorParams::new(&settings, pod_cnt);
    let colors = display::Colors::new(&mut colors_params);
    let pods = kubernetes::Pods::new(namespaces.clone(), &settings.pod_search, colors).await?;
    let pods_lock = pods.to_mutex();

    {
        let mut streams = streams_lock.lock().await;
        display::print_color(
            &mut streams.out,
            None,
            format!("initial search found {} pods across {} namespaces", pod_cnt, namespaces.items.len()),
        )
        .await?;
    }
    let loop_pause = settings.loop_pause;
    let mut no_pod_found = pod_cnt == 0;
    loop {
        if no_pod_found {
            {
                let mut streams = streams_lock.lock().await;
                display::print_color(&mut streams.err, None, "no pod found :(".to_string()).await?;
            }
            continue;
        }
        let pods = pods.clone();
        for pod in pods.items {
            if !pod.is_running().unwrap_or(false) {
                continue;
            }
            let log_params = log_params.clone();
            let streams_lock = streams_lock.clone();
            let pods_lock = pods_lock.clone();
            let settings = settings.clone();
            tokio::spawn(async move {
                let print_res = pod.print_logs(log_params, settings, pods_lock, streams_lock.clone()).await;
                match print_res {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        let error = Errors::Other(err.to_string());
                        {
                            let mut streams = streams_lock.lock().await;
                            display::print_color(&mut streams.err, None, error.to_string()).await?;
                        }
                        return Err(error);
                    }
                }
            });
        }
        no_pod_found = false;
        tokio::time::sleep(tokio::time::Duration::from_millis(loop_pause * 1000)).await;
        {
            let mut pods = pods_lock.lock().await;
            pods.refresh().await?;
        }
    }
}
