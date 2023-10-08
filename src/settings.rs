use std::path::PathBuf;
use std::str::FromStr;
use std::string::ToString;

use clap::Parser;
use regex::Regex;

use validator::Validate;

use crate::{
    display::{HueInterval, Lightness, Saturation},
    error::Errors,
};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Settings {
    /// regex to match pod names
    #[arg(short, long, value_name = "reg pattern", default_value = ".+")]
    pub pod_search: String,

    /// path to the kubeconfig file. if the option is not passed, try to infer configuration
    #[arg(short, long, value_name = "filepath", default_value = "")]
    pub kubeconfig: String,

    /// kubernetes namespaces to use. if the option is not passed, use the default namespace
    #[arg(short, long, value_name = "nmspc")]
    pub namespaces: Option<Vec<String>>,

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

    /// disable automatic pod list refresh
    #[arg(long, default_value_t = false)]
    pub disable_pods_refresh: bool,

    /// number of seconds between each pod list query (doesn't affect log line display)
    #[arg(long, value_name = "seconds", default_value_t = 2)]
    pub loop_pause: u64,

    /// number of color to generate for the color cycle. if 0, it is later set for the number of result retuned by the first pod search
    #[arg(long, value_name = "num", default_value_t = 0)]
    pub color_cycle_len: u8,

    /// hue (hsl) intervals to pick for color cycle generation
    /// format is $start-$end(,$start-$end)* where $start>=0 and $end<=359
    /// eg for powershell: 0-180,280-359
    #[arg(long, value_name = "intervals", default_value = "0-359")]
    pub hue_intervals: String,

    /// the color saturation (0-100)
    #[arg(long, value_name = "sat", default_value_t = 100)]
    pub color_saturation: u8,

    /// the color lightness (0-100)
    #[arg(long, value_name = "light", default_value_t = 50)]
    pub color_lightness: u8,

    /// regex string to filter output that match
    #[arg(long, value_name = "filter", default_value = "")]
    pub filter: String,

    /// regex string to filter output that does not match
    #[arg(long, value_name = "inv_filter", default_value = "")]
    pub inv_filter: String,
}

#[derive(Clone)]
pub struct SettingsValidated {
    pub pod_search: Regex,
    pub kubeconfig: Option<PathBuf>,
    pub namespaces: Vec<String>,
    pub previous: bool,
    pub since_seconds: i64,
    pub tail_lines: i64,
    pub timestamps: bool,
    pub disable_pods_refresh: bool,
    pub loop_pause: u64,
    pub color_cycle_len: u8,
    pub hue_intervals: Vec<HueInterval>,
    pub color_saturation: Saturation,
    pub color_lightness: Lightness,
    pub filter: Option<Regex>,
    pub inv_filter: Option<Regex>,
}

impl Settings {
    pub fn to_validated(self) -> Result<SettingsValidated, Errors> {
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
        let namespaces = match self.namespaces.clone() {
            Some(namespaces) => namespaces,
            None => vec!["default".to_string()],
        };
        let hue_intervals = self.get_hue_intervals()?;
        let color_saturation = Saturation {
            value: self.color_saturation,
        };

        match color_saturation.validate() {
            Ok(val) => val,
            Err(err) => return Err(Errors::Validation(err.to_string())),
        };
        let color_lightness = Lightness { value: self.color_lightness };
        match color_lightness.validate() {
            Ok(val) => val,
            Err(err) => return Err(Errors::Validation(err.to_string())),
        };

        let filter = if self.filter == "".to_string() {
            None
        } else {
            match Regex::new(self.filter.as_str()) {
                Ok(val) => Some(val),
                Err(err) => return Err(Errors::Validation(err.to_string())),
            }
        };
        let inv_filter = if self.inv_filter == "".to_string() {
            None
        } else {
            match Regex::new(self.inv_filter.as_str()) {
                Ok(val) => Some(val),
                Err(err) => return Err(Errors::Validation(err.to_string())),
            }
        };

        return Ok(SettingsValidated {
            pod_search,
            kubeconfig,
            namespaces: namespaces,
            previous: self.previous,
            since_seconds: self.since_seconds,
            tail_lines: self.tail_lines,
            timestamps: self.timestamps,
            disable_pods_refresh: self.disable_pods_refresh,
            loop_pause: self.loop_pause,
            color_cycle_len: self.color_cycle_len,
            hue_intervals,
            color_saturation,
            color_lightness,
            filter,
            inv_filter,
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
}
