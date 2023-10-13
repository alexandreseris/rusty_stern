use std::collections::HashSet;

use chrono::{DateTime, FixedOffset};
use colors_transform::Rgb;
use futures::{AsyncBufReadExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod as ApiPod;
use kube::api::ListParams;
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, Config};
use regex::Regex;

use crate::error::Errors;
use crate::{display_v2 as display, settings_v2 as settings, types};

fn get_pod_name(pod: &ApiPod) -> String {
    return pod.metadata.name.clone().unwrap_or("NO_NAME".to_string());
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
            let pod_list = namespace
                .api
                .list(&ListParams::default())
                .await
                .map_err(|err| Errors::Kubernetes(format!("get pods list on namespace {}", namespace.name), err.to_string()))?;

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

pub fn get_pod_status(pod: &ApiPod) -> Option<&String> {
    if let Some(status) = &pod.status {
        if let Some(phase) = &status.phase {
            return Some(phase);
        }
    }
    return None;
}

pub fn is_pod_running(pod: &ApiPod) -> bool {
    if let Some(phase) = get_pod_status(pod) {
        return phase == "Running";
    }
    return false;
}

impl Pod {
    pub fn is_running(&self) -> bool {
        return is_pod_running(&self.pod_api);
    }

    pub async fn print_logs(
        &self,
        log_params: kube::api::LogParams,
        settings: settings::SettingsValidated,
        pods: types::ArcMutex<Pods>,
        streams: types::ArcMutex<display::Streams>,
    ) -> Result<(), Errors> {
        let mut stream = self
            .namespace
            .api
            .log_stream(&self.name, &log_params)
            .await
            .map_err(|err| Errors::LogError(err.to_string()))?
            .lines();
        while let Some(line) = stream.try_next().await.map_err(|err| Errors::LogError(err.to_string()))? {
            display::print_log_line(&line, &settings, &pods, &streams, self).await?;
        }
        return Ok(());
    }

    pub async fn get_previous_log_lines(
        &self,
        log_param: &kube::api::LogParams,
        settings: &settings::SettingsValidated,
    ) -> Result<Vec<(DateTime<FixedOffset>, String, Pod)>, Errors> {
        let mut lines = vec![];
        for raw_line in self
            .namespace
            .api
            .logs(&self.name, &log_param)
            .await
            .map_err(|err| Errors::Kubernetes("getting log sync".to_string(), err.to_string()))?
            .split("\n")
            .filter(|line| line.len() != 0)
        {
            let date_str = raw_line.split(" ").next().ok_or(Errors::LogError("failled to split line".to_string()))?;
            let mut line = raw_line;
            if !settings.timestamps {
                line = &line[date_str.len() + 1..];
            }
            let date = chrono::DateTime::parse_from_rfc3339(date_str).map_err(|err| Errors::LogError(err.to_string()))?;
            lines.push((date, line.to_string(), self.clone()));
        }
        return Ok(lines);
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
            let pod_list = namespace
                .api
                .list(&ListParams::default())
                .await
                .map_err(|err| Errors::Kubernetes(format!("get pods list on namespace {}", namespace.name), err.to_string()))?;

            for pod in pod_list {
                let name = get_pod_name(&pod);
                if pod_search.is_match(name.as_str()) && is_pod_running(&pod) {
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
            let pod_list = namespace
                .api
                .list(&ListParams::default())
                .await
                .map_err(|err| Errors::Kubernetes(format!("get pods list on namespace {}", namespace.name), err.to_string()))?;

            for pod in pod_list {
                let name = get_pod_name(&pod);
                if self.pod_search.is_match(name.as_str()) && is_pod_running(&pod) && !self.pod_already_exists(&name, namespace) {
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

pub fn new_log_param(settings: &settings::SettingsValidated, previous_line_set: bool) -> kube::api::LogParams {
    return if previous_line_set {
        kube::api::LogParams {
            container: None,
            limit_bytes: None,
            pretty: false,
            previous: settings.previous,
            follow: false,
            timestamps: true,
            since_seconds: settings.since_seconds,
            tail_lines: settings.tail_lines,
        }
    } else {
        kube::api::LogParams {
            container: None,
            limit_bytes: None,
            pretty: false,
            previous: settings.previous,
            follow: true,
            timestamps: settings.timestamps,
            since_seconds: None,
            tail_lines: Some(0),
        }
    };
}

pub async fn new_client(settings: &crate::settings_v2::SettingsValidated) -> Result<Client, Errors> {
    let mut conf = match &settings.kubeconfig {
        Some(val) => {
            let kconf = Kubeconfig::read_from(val).map_err(|err| Errors::Kubernetes("reading config file".to_string(), err.to_string()))?;
            let kconfopt = &KubeConfigOptions::default();
            Config::from_custom_kubeconfig(kconf, kconfopt)
                .await
                .map_err(|err| Errors::Kubernetes("parsing config file".to_string(), err.to_string()))?
        }
        None => Config::infer()
            .await
            .map_err(|err| Errors::Kubernetes("getting default config".to_string(), err.to_string()))?,
    };
    conf.read_timeout = None;
    conf.write_timeout = None;
    conf.connect_timeout = None;

    let client = Client::try_from(conf).map_err(|err| Errors::Kubernetes("using kubernetes configuration".to_string(), err.to_string()))?;
    return Ok(client);
}

pub fn new_running_pods() -> types::ArcMutex<HashSet<String>> {
    let hash = HashSet::new();
    return std::sync::Arc::new(tokio::sync::Mutex::new(hash));
}
