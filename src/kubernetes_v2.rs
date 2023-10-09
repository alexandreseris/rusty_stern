use std::collections::HashSet;

use colors_transform::Rgb;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod as ApiPod;
use kube::api::ListParams;
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, Config};
use regex::Regex;

use crate::error::Errors;
use crate::{display_v2 as display, settings_v2 as settings, types};

fn get_pod_name(pod: &ApiPod) -> String {
    return match &pod.metadata.name {
        Some(name) => name.clone(),
        None => "NO_NAME".to_string(),
    };
}

#[derive(Clone)]
pub struct Namespace {
    pub name: String,
    pub api: Api<ApiPod>,
}

#[derive(Clone)]
pub struct Namespaces {
    pub items: Vec<Namespace>,
}

impl Namespaces {
    pub fn new(client: &kube::Client, namespaces_names: &Vec<String>) -> Namespaces {
        let mut namespaces: Vec<Namespace> = vec![];
        let namespaces_mut: &mut Vec<Namespace> = namespaces.as_mut();
        for namespace in namespaces_names {
            namespaces_mut.push(Namespace {
                name: namespace.clone(),
                api: Api::namespaced(client.clone(), &namespace.clone()),
            });
        }
        return Namespaces { items: namespaces };
    }

    pub async fn get_pods_cnt(&self, search: &Regex) -> Result<usize, Errors> {
        let mut cnt: usize = 0;
        for namespace in self.items.iter() {
            let pod_list = match namespace.api.list(&ListParams::default()).await {
                Ok(pod_list) => pod_list,
                Err(err) => {
                    return Err(Errors::Kubernetes(
                        format!("get pods list on namespace {}", namespace.name),
                        err.to_string(),
                    ))
                }
            };
            for pod in pod_list {
                let name = get_pod_name(&pod);
                if search.is_match(name.as_str()) {
                    cnt += 1;
                }
            }
        }
        return Ok(cnt);
    }
}

#[derive(Clone)]
pub struct Pod {
    pub name: String,
    pub namespace: Namespace,
    pub pod_api: ApiPod,
    pub color: Rgb,
}

impl PartialEq for Pod {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.namespace.name == other.namespace.name
    }
}
impl Eq for Pod {}

impl Pod {
    pub fn is_running(&self) -> Option<bool> {
        if let Some(status) = &self.pod_api.status {
            if let Some(phase) = &status.phase {
                return Some(phase == "Running");
            }
        }
        return None;
    }

    pub async fn print_logs(
        &self,
        log_params: kube::api::LogParams,
        settings: settings::SettingsValidated,
        pods: types::ArcMutex<Pods>,
        streams: types::ArcMutex<display::Streams>,
    ) -> Result<(), Errors> {
        let mut stream = match self.namespace.api.log_stream(&self.name, &log_params).await {
            Ok(stream) => stream,
            Err(err) => return Err(Errors::LogError(err.to_string())),
        };
        let mut line_bytes: bytes::Bytes;
        let empty_bytes = &bytes::Bytes::from("");
        let mut error = None;
        loop {
            let next = match stream.next().await {
                Some(val) => val,
                None => Ok(empty_bytes.clone()),
            };
            line_bytes = match next {
                Ok(val) => val,
                Err(err) => {
                    error = Some(Errors::Kubernetes("failled to retrieve logs".to_string(), err.to_string()));
                    break;
                }
            };
            if line_bytes == empty_bytes {
                if self.is_running().unwrap_or(false) {
                    continue;
                }
                break;
            }
            let content = match std::str::from_utf8(line_bytes.iter().as_slice()) {
                Ok(content) => content,
                Err(err) => return Err(Errors::LogError(err.to_string())),
            };

            if let Some(reg) = &settings.filter {
                if !reg.is_match(content) {
                    continue;
                }
            }
            if let Some(reg) = &settings.inv_filter {
                if reg.is_match(content) {
                    continue;
                }
            }

            let padding_cnt;
            let namespace: String;
            {
                let pods = pods.lock().await;
                namespace = match pods.print_namespace {
                    true => {
                        padding_cnt = pods.padding - self.name.len() - self.namespace.name.len() + 1;
                        format!("{}/", self.namespace.name)
                    }
                    false => {
                        padding_cnt = pods.padding - self.name.len();
                        "".to_string()
                    }
                };
            }
            let padding_str = " ".repeat(padding_cnt);
            let message = format!("{namespace}{}:{padding_str} {content}", &self.name);

            {
                let mut streams = streams.lock().await;
                let stdout = &mut streams.out;
                display::print_color(stdout, Some(self.color), message).await?;
            }
        }

        let error_reason = match error {
            Some(value) => value.to_string(),
            None => "unknown".to_string(),
        };
        {
            let mut streams = streams.lock().await;
            let stderr = &mut streams.err;
            display::print_color(
                stderr,
                Some(self.color),
                format!("--- pod {}/{} ended (reason: {error_reason})", self.name, self.namespace.name),
            )
            .await?;
        }
        {
            let mut pods = pods.lock().await;
            pods.remove_pod(self).await;
            pods.colors.set_color_to_unused(self.color);
        }
        return Ok(());
    }
}

#[derive(Clone)]
pub struct Pods {
    pub items: Vec<Pod>,
    pub padding: usize,
    pub print_namespace: bool,
    pub namespaces: Namespaces,
    pub pod_search: Regex,
    pub colors: display::Colors,
}

impl Pods {
    pub fn to_mutex(&self) -> types::ArcMutex<Self> {
        return std::sync::Arc::new(tokio::sync::Mutex::new(self.clone()));
    }

    fn set_global_fields(&mut self) {
        let print_namespace = self.namespaces.items.len() > 1;
        let mut max_len = 0;
        let mut namespaces = HashSet::new();
        for pod in self.items.iter() {
            namespaces.insert(pod.namespace.name.clone());
            let mut len = pod.name.len();
            if print_namespace {
                len += pod.namespace.name.len();
            }
            if len > max_len {
                max_len = len;
            }
        }
        self.padding = max_len;
        self.print_namespace = self.print_namespace;
    }

    pub async fn new(namespaces: Namespaces, pod_search: &Regex, mut colors: display::Colors) -> Result<Pods, Errors> {
        let mut pod_list = vec![];
        let pods_mut: &mut Vec<Pod> = pod_list.as_mut();
        for namespace in namespaces.clone().items {
            let pod_list = match namespace.api.list(&ListParams::default()).await {
                Ok(pod_list) => pod_list,
                Err(err) => {
                    return Err(Errors::Kubernetes(
                        format!("get pods list on namespace {}", namespace.name),
                        err.to_string(),
                    ))
                }
            };
            for pod in pod_list {
                let name = get_pod_name(&pod);
                if pod_search.is_match(name.as_str()) {
                    pods_mut.push(Pod {
                        name: name.clone(),
                        pod_api: pod,
                        namespace: namespace.clone(),
                        color: colors.get_new_color(),
                    });
                }
            }
        }
        let mut pods = Pods {
            items: pod_list,
            padding: 0,
            print_namespace: false,
            namespaces: namespaces.clone(),
            pod_search: pod_search.clone(),
            colors: colors,
        };
        pods.set_global_fields();
        return Ok(pods);
    }

    pub async fn remove_pod(&mut self, pod: &Pod) {
        if let Some(pod_idx) = self
            .items
            .iter()
            .position(|item| item.name == pod.name && item.namespace.name == pod.namespace.name)
        {
            self.items.remove(pod_idx);
            self.set_global_fields();
        }
    }

    fn pod_already_exists(&self, pod_name: &String, namespace: &Namespace) -> bool {
        let pod_name = pod_name.clone();
        return self
            .items
            .iter()
            .filter(|pod| pod.name == pod_name && pod.namespace.name == namespace.name)
            .next()
            .is_some();
    }

    pub async fn refresh(&mut self) -> Result<(), Errors> {
        let mut found_one = false;
        for namespace in self.namespaces.items.iter() {
            let pod_list = match namespace.api.list(&ListParams::default()).await {
                Ok(pod_list) => pod_list,
                Err(err) => {
                    return Err(Errors::Kubernetes(
                        format!("get pods list on namespace {}", namespace.name),
                        err.to_string(),
                    ))
                }
            };
            for pod in pod_list {
                let name = get_pod_name(&pod);
                if self.pod_search.is_match(name.as_str()) && !self.pod_already_exists(&name, namespace) {
                    found_one = true;
                    self.items.push(Pod {
                        name: name.clone(),
                        pod_api: pod,
                        namespace: namespace.clone(),
                        color: self.colors.get_new_color(),
                    });
                }
            }
        }
        if found_one {
            self.set_global_fields();
        }
        return Ok(());
    }
}

pub fn new_log_param(settings: &crate::settings_v2::SettingsValidated) -> kube::api::LogParams {
    return kube::api::LogParams {
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
}

pub async fn new_client(settings: &crate::settings_v2::SettingsValidated) -> Result<Client, Errors> {
    let mut conf = match &settings.kubeconfig {
        Some(val) => {
            let kconf = match Kubeconfig::read_from(val) {
                Ok(val) => val,
                Err(err) => return Err(Errors::Kubernetes("reading config file".to_string(), err.to_string())),
            };
            let kconfopt = &KubeConfigOptions::default();
            match Config::from_custom_kubeconfig(kconf, kconfopt).await {
                Ok(val) => val,
                Err(err) => return Err(Errors::Kubernetes("parsing config file".to_string(), err.to_string())),
            }
        }
        None => match Config::infer().await {
            Ok(val) => val,
            Err(err) => return Err(Errors::Kubernetes("getting default config".to_string(), err.to_string())),
        },
    };
    conf.read_timeout = None;
    conf.write_timeout = None;
    conf.connect_timeout = None;

    let client = match Client::try_from(conf) {
        Ok(val) => val,
        Err(err) => return Err(Errors::Kubernetes("using kubernetes configuration".to_string(), err.to_string())),
    };
    return Ok(client);
}
