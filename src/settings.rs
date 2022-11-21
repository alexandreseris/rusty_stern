use std::fs;
use std::path::PathBuf;

use clap::Parser;
use home;
use serde::{Deserialize, Serialize};
use serde_json::{to_string, Error};

use crate::display::ColorRGB;
use crate::error::GenericError;

fn get_config_file_path() -> Result<PathBuf, GenericError> {
    let home_dir = home::home_dir();
    if home_dir.is_some() {
        let conf_file = home_dir.unwrap().join(".rusty_stern").join("config");
        return Ok(conf_file);
    } else {
        return Err(GenericError {
            message: "home directory is not available".to_string(),
        });
    }
}

pub fn create_default_config_file() -> Result<(), GenericError> {
    let conf_file = get_config_file_path()?;
    let def_settings = Settings { ..Default::default() };
    fs::write(conf_file, to_string(&def_settings).unwrap().as_bytes()).unwrap();
    Ok(())
}

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Settings {
    /// regex to match pod names
    #[arg(short, long, value_name="reg pattern", default_value_t = (".+".to_string()))]
    #[serde(default)]
    pub pod_search: String,
    /// path to the kubeconfig file. if the option is not passed, try to infer configuration
    #[arg(short, long, value_name="filepath", default_value_t = ("".to_string()))]
    #[serde(default)]
    pub kubeconfig: String,
    /// kubernetes namespace to use. if the option is not passed, use the default namespace
    #[arg(short, long, value_name="nmspc", default_value_t = ("".to_string()))]
    #[serde(default)]
    pub namespace: String,

    /// retrieve previous terminated container logs
    #[arg(long, default_value_t = false)]
    #[serde(default)]
    pub previous: bool,
    /// a relative time in seconds before the current time from which to show logs
    #[arg(long, value_name = "seconds", default_value_t = 0)]
    #[serde(default)]
    pub since_seconds: i64,
    /// number of lines from the end of the logs to show
    #[arg(long, value_name = "line_cnt", default_value_t = 0)]
    #[serde(default)]
    pub tail_lines: i64,
    /// show timestamp at the begining of each log line
    #[arg(long, default_value_t = false)]
    #[serde(default)]
    pub timestamps: bool,

    /// number of seconds between each pod list query (doesn't affect log line display)
    #[arg(long, value_name = "seconds", default_value_t = 2)]
    #[serde(default)]
    pub loop_pause: u64,

    /// verbose output
    #[arg(short, long, default_value_t = false)]
    #[serde(default)]
    pub verbose: bool,

    /// debug rgb color (format is 0-255,0-255,0-255)
    #[arg(long, value_name="rgb", default_value_t = ("255,255,255".to_string()))]
    #[serde(default)]
    pub debug_color: String,
    /// number of color to generate for the color cycle. if 0, it is later set for about the number of result retuned by the first pod search
    #[arg(long, value_name = "num", default_value_t = 0)]
    #[serde(default)]
    pub color_cycle_len: u8,
    /// the color saturation (0-100)
    #[arg(long, value_name = "sat", default_value_t = 100)]
    #[serde(default)]
    pub color_saturation: u8,
    /// the color lightness (0-100)
    #[arg(long, value_name = "light", default_value_t = 50)]
    #[serde(default)]
    pub color_lightness: u8,

    /// generate a default config file and exit
    #[arg(long, default_value_t = false)]
    #[serde(default)]
    pub generate_config_file: bool,
}

impl Default for Settings {
    // default is redundant with clap default_value_t, kinda sucks
    fn default() -> Settings {
        Settings {
            pod_search: ".+".to_string(),
            kubeconfig: "".to_string(),
            namespace: "".to_string(),
            previous: false,
            since_seconds: 0,
            tail_lines: 0,
            timestamps: false,
            loop_pause: 2,
            verbose: false,
            debug_color: "255,255,255".to_string(),
            color_cycle_len: 0,
            color_saturation: 100,
            color_lightness: 50,
            generate_config_file: false,
        }
    }
}

impl Settings {
    pub fn do_parse() -> Settings {
        Settings::parse()
    }

    pub fn get_debug_color(self) -> Result<ColorRGB, GenericError> {
        let res: Result<ColorRGB, GenericError> = match self.debug_color.as_str().parse::<ColorRGB>() {
            Ok(debug_color) => Ok(debug_color),
            Err(err) => Err(GenericError { message: err.to_string() }),
        };
        res
    }

    pub fn from_config_file() -> Result<Settings, GenericError> {
        let conf_file = get_config_file_path()?;
        if conf_file.exists() {
            let file_content = fs::read_to_string(conf_file.clone());
            if file_content.is_err() {
                let err = file_content.unwrap_err();
                return Err(GenericError {
                    message: format!("failled to read config file {}: {}", conf_file.display(), err),
                });
            }
            let settings: Result<Settings, Error> = serde_json::from_str(file_content.unwrap().as_str());
            if settings.is_err() {
                let err = settings.unwrap_err();
                return Err(GenericError {
                    message: format!("failled to parse config file {}: {}", conf_file.display(), err),
                });
            }
            return Ok(settings.unwrap());
        } else {
            return Err(GenericError {
                message: format!("config file {} does not exists", conf_file.display()),
            });
        }
    }

    pub fn update(&mut self, other_setting: Self) {
        self.pod_search = other_setting.pod_search;
        self.kubeconfig = other_setting.kubeconfig;
        self.namespace = other_setting.namespace;
        self.previous = other_setting.previous;
        self.since_seconds = other_setting.since_seconds;
        self.tail_lines = other_setting.tail_lines;
        self.timestamps = other_setting.timestamps;
        self.loop_pause = other_setting.loop_pause;
        self.verbose = other_setting.verbose;
        self.debug_color = other_setting.debug_color;
        self.color_cycle_len = other_setting.color_cycle_len;
        self.color_saturation = other_setting.color_saturation;
        self.color_lightness = other_setting.color_lightness;
    }
}
