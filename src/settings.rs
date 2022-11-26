use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::string::ToString;

use clap::Parser;
use colors_transform::{Color, Hsl, Rgb};
use home;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

use rusty_stern_traits::Update;
use validator::Validate;

use crate::{
    display::{HueInterval, Lightness, Saturation},
    error::Errors,
};

fn get_config_file_path() -> Result<PathBuf, Errors> {
    let home_dir = match home::home_dir() {
        Some(val) => val,
        None => return Err(Errors::FileNotFound("home directory is not available".to_string())),
    };
    return Ok(home_dir.join(".rusty_stern").join("config"));
}

pub fn create_default_config_file() -> Result<(), Errors> {
    let conf_file = get_config_file_path()?;
    let def_settings = Settings { ..Default::default() };
    let contents_str = match to_string_pretty(&def_settings) {
        Ok(val) => val,
        Err(err) => return Err(Errors::Other(err.to_string())),
    };
    let contents = contents_str.as_bytes();
    match fs::write(conf_file.clone(), contents) {
        Ok(val) => val,
        Err(err) => return Err(Errors::FileError(conf_file.display().to_string(), err.to_string())),
    };
    println!("wrote default configuration to {} file", conf_file.display());
    Ok(())
}

#[derive(Parser, Debug, Serialize, Deserialize, Clone, Update)]
#[command(author, version, about, long_about = None)]
pub struct Settings {
    /// regex to match pod names
    #[arg(short, long, value_name="reg pattern", default_value_t = Settings::default().pod_search)]
    #[serde(default)]
    pub pod_search: String,
    /// path to the kubeconfig file. if the option is not passed, try to infer configuration
    #[arg(short, long, value_name="filepath", default_value_t = Settings::default().kubeconfig)]
    #[serde(default)]
    pub kubeconfig: String,
    /// kubernetes namespace to use. if the option is not passed, use the default namespace
    #[arg(short, long, value_name="nmspc", default_value_t = Settings::default().namespace)]
    #[serde(default)]
    pub namespace: String,

    /// retrieve previous terminated container logs
    #[arg(long, default_value_t = Settings::default().previous)]
    #[serde(default)]
    pub previous: bool,
    /// a relative time in seconds before the current time from which to show logs
    #[arg(long, value_name = "seconds", default_value_t = Settings::default().since_seconds)]
    #[serde(default)]
    pub since_seconds: i64,
    /// number of lines from the end of the logs to show
    #[arg(long, value_name = "line_cnt", default_value_t = Settings::default().tail_lines)]
    #[serde(default)]
    pub tail_lines: i64,
    /// show timestamp at the begining of each log line
    #[arg(long, default_value_t = Settings::default().timestamps)]
    #[serde(default)]
    pub timestamps: bool,

    /// disable automatic pod list refresh
    #[arg(long, default_value_t = Settings::default().disable_pods_refresh)]
    #[serde(default)]
    pub disable_pods_refresh: bool,
    /// number of seconds between each pod list query (doesn't affect log line display)
    #[arg(long, value_name = "seconds", default_value_t = Settings::default().loop_pause)]
    #[serde(default)]
    pub loop_pause: u64,

    /// verbose output
    #[arg(short, long, default_value_t = Settings::default().verbose)]
    #[serde(default)]
    pub verbose: bool,

    /// debug hsl color (format is hue,saturation,lightness)
    #[arg(long, value_name="hsl", default_value_t = Settings::default().debug_color)]
    #[serde(default)]
    pub debug_color: String,
    /// number of color to generate for the color cycle. if 0, it is later set for about the number of result retuned by the first pod search
    #[arg(long, value_name = "num", default_value_t = Settings::default().color_cycle_len)]
    #[serde(default)]
    pub color_cycle_len: u8,
    /// hue (hsl) intervals to pick for color cycle generation
    /// format is $start-$end(,$start-$end)* where $start>=0 and $end<=359
    /// eg for powershell: 0-180,280-359
    #[arg(long, value_name = "intervals", default_value_t = Settings::default().hue_intervals)]
    #[serde(default)]
    pub hue_intervals: String,
    /// the color saturation (0-100)
    #[arg(long, value_name = "sat", default_value_t = Settings::default().color_saturation)]
    #[serde(default)]
    pub color_saturation: u8,
    /// the color lightness (0-100)
    #[arg(long, value_name = "light", default_value_t = Settings::default().color_lightness)]
    #[serde(default)]
    pub color_lightness: u8,

    /// generate a default config file and exit
    #[arg(long, default_value_t = Settings::default().generate_config_file)]
    #[serde(default)]
    pub generate_config_file: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            pod_search: ".+".to_string(),
            kubeconfig: "".to_string(),
            namespace: "".to_string(),
            previous: false,
            since_seconds: 0,
            tail_lines: 0,
            timestamps: false,
            disable_pods_refresh: false,
            loop_pause: 2,
            verbose: false,
            debug_color: "0,0,100".to_string(),
            color_cycle_len: 0,
            hue_intervals: "0-359".to_string(),
            color_saturation: 100,
            color_lightness: 50,
            generate_config_file: false,
        }
    }
}

#[derive(Clone)]
pub struct SettingsParsed {
    pub pod_search: Regex,
    pub kubeconfig: Option<PathBuf>,
    pub namespace: Option<String>,
    pub previous: bool,
    pub since_seconds: i64,
    pub tail_lines: i64,
    pub timestamps: bool,
    pub disable_pods_refresh: bool,
    pub loop_pause: u64,
    pub verbose: bool,
    pub debug_color: Rgb,
    pub color_cycle_len: u8,
    pub hue_intervals: Vec<HueInterval>,
    pub color_saturation: Saturation,
    pub color_lightness: Lightness,
    pub generate_config_file: bool,
}

impl Settings {
    pub fn validate(self) -> Result<SettingsParsed, Errors> {
        let pod_search = match Regex::new(self.pod_search.as_str()) {
            Ok(val) => val,
            Err(err) => return Err(Errors::Validation(err.to_string())),
        };
        let kubeconfig = if self.kubeconfig == "".to_string() {
            None
        } else {
            Some(match PathBuf::from_str(self.kubeconfig.as_str()) {
                Ok(val) => val,
                Err(err) => return Err(Errors::Other(err.to_string())),
            })
        };
        let namespace = if self.namespace == "".to_string() {
            None
        } else {
            Some(self.clone().namespace)
        };
        let debug_color = self.get_debug_color()?;
        let hue_intervals = self.get_hue_intervals()?;
        let color_saturation = Saturation {
            value: self.color_saturation,
        };
        let color_lightness = Lightness { value: self.color_lightness };

        match color_saturation.validate() {
            Ok(val) => val,
            Err(err) => return Err(Errors::Validation(err.to_string())),
        };
        match color_lightness.validate() {
            Ok(val) => val,
            Err(err) => return Err(Errors::Validation(err.to_string())),
        };

        return Ok(SettingsParsed {
            pod_search,
            kubeconfig,
            namespace,
            previous: self.previous,
            since_seconds: self.since_seconds,
            tail_lines: self.tail_lines,
            timestamps: self.timestamps,
            disable_pods_refresh: self.disable_pods_refresh,
            loop_pause: self.loop_pause,
            verbose: self.verbose,
            debug_color,
            color_cycle_len: self.color_cycle_len,
            hue_intervals,
            color_saturation,
            color_lightness,
            generate_config_file: self.generate_config_file,
        });
    }

    pub fn do_parse() -> Settings {
        Settings::parse()
    }

    pub fn get_hue_intervals(&self) -> Result<Vec<HueInterval>, Errors> {
        let mut intervals: Vec<HueInterval> = Vec::new();
        for str_intervals in self.hue_intervals.split(",") {
            let interval = HueInterval::from_str(str_intervals)?;
            match interval.clone().validate() {
                Ok(val) => val,
                Err(err) => return Err(Errors::Validation(err.to_string())),
            };
            intervals.push(interval);
        }
        return Ok(intervals);
    }

    pub fn get_debug_color(&self) -> Result<Rgb, Errors> {
        return match Hsl::from_str(self.debug_color.as_str()) {
            Ok(debug_color) => Ok(debug_color.to_rgb()),
            Err(err) => Err(Errors::Validation(err.message)),
        };
    }

    pub fn from_config_file() -> Result<Settings, Errors> {
        let conf_file = get_config_file_path()?;
        let conf_file_display = conf_file.display();
        if conf_file.exists() {
            let file_content = match fs::read_to_string(conf_file.clone()) {
                Ok(val) => val,
                Err(err) => return Err(Errors::Other(format!("failled to read config file {conf_file_display}: {err}"))),
            };
            let settings = match serde_json::from_str::<Settings>(file_content.as_str()) {
                Ok(val) => val,
                Err(err) => return Err(Errors::Validation(format!("failled to parse config file {conf_file_display}: {err}"))),
            };
            return Ok(settings);
        } else {
            return Err(Errors::FileNotFound(conf_file_display.to_string()));
        }
    }
}
