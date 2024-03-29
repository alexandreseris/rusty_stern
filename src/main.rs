mod display;
mod error;
mod kubernetes;
mod settings;
mod types;

use crate::error::Errors;
use chrono::DateTime;
use chrono::FixedOffset;
use tokio;
use tokio::task::JoinHandle;

#[tokio::main]
async fn main() -> Result<(), Errors> {
    let streams: display::Streams = display::new_streams();
    let streams_lock = display::new_streams_mutex(streams);

    let settings = settings::Settings::do_parse();
    let settings = settings.to_validated()?;

    let log_params = kubernetes::new_log_param(&settings, false);
    let client = kubernetes::new_client(&settings).await?;

    let namespaces = kubernetes::Namespaces::new(&client, &settings.namespaces);
    let pod_cnt = namespaces.get_pods_cnt(&settings.pod_search).await?;
    let mut colors_params = display::ColorParams::new(&settings, pod_cnt);
    let colors = display::Colors::new(&mut colors_params);
    let pods = kubernetes::Pods::new(namespaces.clone(), &settings.pod_search, colors).await?;
    let pods_lock = pods.to_mutex();

    let running_pods = kubernetes::new_running_pods();

    {
        let mut streams = streams_lock.lock().await;
        display::print_color(
            &mut streams.out,
            None,
            format!("initial search found {} pods across {} namespaces", pod_cnt, namespaces.items.len()),
        )
        .await?;
    }
    if settings.is_previous_lines() {
        let previous_lines_settings = kubernetes::new_log_param(&settings, true);
        let mut log_lines = vec![];
        {
            let pods = pods_lock.lock().await;
            let mut tasks = vec![];
            for pod in pods.items.iter() {
                let pod = pod.clone();
                let previous_lines_settings = previous_lines_settings.clone();
                let settings = settings.clone();
                let task: JoinHandle<Result<Vec<(DateTime<FixedOffset>, String, kubernetes::Pod)>, Errors>> = tokio::spawn(async move {
                    return pod.get_previous_log_lines(&previous_lines_settings, &settings).await;
                });
                tasks.push(task);
            }
            for task in tasks {
                let mut task_res = task.await.map_err(|err| Errors::Other(err.to_string()))??;
                log_lines.append(&mut task_res);
            }
        }
        log_lines.sort_by(|current, next| current.0.cmp(&next.0));
        for line in log_lines {
            display::print_log_line(&line.1, &settings, &pods_lock, &streams_lock, &line.2).await?;
        }
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
        let pod_list = {
            let pods = pods_lock.lock().await;
            pods.items.clone()
        };
        let running_pods = running_pods.clone();
        for pod in pod_list {
            let pod_id = format!("{}/{}", pod.namespace.name, pod.name);
            if !pod.is_running() {
                {
                    let mut pods = pods_lock.lock().await;
                    pods.remove_pod(&pod).await;
                    pods.colors.set_color_to_unused(pod.color);
                }
                {
                    let mut running_pods = running_pods.lock().await;
                    running_pods.remove(&pod_id);
                }
                continue;
            }

            let already_running = {
                let running_pods = running_pods.lock().await;
                running_pods.get(&pod_id).is_some()
            };
            if already_running {
                continue;
            }
            {
                let mut running_pods = running_pods.lock().await;
                running_pods.insert(pod_id.clone());
            }
            let log_params = log_params.clone();
            let streams_lock = streams_lock.clone();
            let pods_lock = pods_lock.clone();
            let settings = settings.clone();
            let running_pods = running_pods.clone();

            tokio::spawn(async move {
                {
                    let mut streams = streams_lock.lock().await;
                    display::print_color(&mut streams.out, Some(pod.color), format!("+++ {} just started", pod_id)).await?;
                }

                let print_res = pod.print_logs(log_params, settings, pods_lock.clone(), streams_lock.clone()).await;
                {
                    let mut pods = pods_lock.lock().await;
                    pods.remove_pod(&pod).await;
                    pods.colors.set_color_to_unused(pod.color);
                }
                {
                    let mut running_pods = running_pods.lock().await;
                    running_pods.remove(&pod_id);
                }
                match print_res {
                    Ok(_) => Ok({
                        let mut streams = streams_lock.lock().await;
                        display::print_color(&mut streams.out, Some(pod.color), format!("--- {} gracefully stopped (maybe)", pod_id)).await?;
                    }),
                    Err(err) => {
                        let error = Errors::Other(err.to_string());
                        {
                            let mut streams = streams_lock.lock().await;
                            display::print_color(
                                &mut streams.err,
                                Some(pod.color),
                                format!("--- {} failled miserably ({})", pod_id, error.to_string()),
                            )
                            .await?;
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
