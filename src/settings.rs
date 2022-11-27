use std::path::PathBuf;
use std::str::FromStr;
use std::string::ToString;

use clap::Parser;
use colors_transform::{Color, Hsl, Rgb};
use regex::Regex;

use validator::Validate;

use crate::{
    display::{Hsl as CustomHsl, HueInterval, Lightness, Saturation},
    error::Errors,
};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Settings {
    /// regex to match pod names
    #[arg(short, long, value_name="reg pattern", default_value_t = Settings::default().pod_search)]
    pub pod_search: String,
    /// path to the kubeconfig file. if the option is not passed, try to infer configuration
    #[arg(short, long, value_name="filepath", default_value_t = Settings::default().kubeconfig)]
    pub kubeconfig: String,
    /// kubernetes namespace to use. if the option is not passed, use the default namespace
    #[arg(short, long, value_name = "nmspc")]
    pub namespace: Option<Vec<String>>,

    /// retrieve previous terminated container logs
    #[arg(long, default_value_t = Settings::default().previous)]
    pub previous: bool,
    /// a relative time in seconds before the current time from which to show logs
    #[arg(long, value_name = "seconds", default_value_t = Settings::default().since_seconds)]
    pub since_seconds: i64,
    /// number of lines from the end of the logs to show
    #[arg(long, value_name = "line_cnt", default_value_t = Settings::default().tail_lines)]
    pub tail_lines: i64,
    /// show timestamp at the begining of each log line
    #[arg(long, default_value_t = Settings::default().timestamps)]
    pub timestamps: bool,

    /// disable automatic pod list refresh
    #[arg(long, default_value_t = Settings::default().disable_pods_refresh)]
    pub disable_pods_refresh: bool,
    /// number of seconds between each pod list query (doesn't affect log line display)
    #[arg(long, value_name = "seconds", default_value_t = Settings::default().loop_pause)]
    pub loop_pause: u64,

    /// default hsl color (format is hue,saturation,lightness), used for general and error messages
    /// default hsl color (format is hue,saturation,lightness)
    #[arg(long, value_name="hsl", default_value_t = Settings::default().default_color)]
    pub default_color: String,
    /// number of color to generate for the color cycle. if 0, it is later set for about the number of result retuned by the first pod search
    #[arg(long, value_name = "num", default_value_t = Settings::default().color_cycle_len)]
    pub color_cycle_len: u8,
    /// hue (hsl) intervals to pick for color cycle generation
    /// format is $start-$end(,$start-$end)* where $start>=0 and $end<=359
    /// eg for powershell: 0-180,280-359
    #[arg(long, value_name = "intervals", default_value_t = Settings::default().hue_intervals)]
    pub hue_intervals: String,
    /// the color saturation (0-100)
    #[arg(long, value_name = "sat", default_value_t = Settings::default().color_saturation)]
    pub color_saturation: u8,
    /// the color lightness (0-100)
    #[arg(long, value_name = "light", default_value_t = Settings::default().color_lightness)]
    pub color_lightness: u8,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            pod_search: ".+".to_string(),
            kubeconfig: "".to_string(),
            namespace: None,
            previous: false,
            since_seconds: 0,
            tail_lines: 0,
            timestamps: false,
            disable_pods_refresh: false,
            loop_pause: 2,
            default_color: "0,0,100".to_string(),
            color_cycle_len: 0,
            hue_intervals: "0-359".to_string(),
            color_saturation: 100,
            color_lightness: 50,
        }
    }
}

#[derive(Clone)]
pub struct SettingsValidated {
    pub pod_search: Regex,
    pub kubeconfig: Option<PathBuf>,
    pub namespace: Option<Vec<String>>,
    pub previous: bool,
    pub since_seconds: i64,
    pub tail_lines: i64,
    pub timestamps: bool,
    pub disable_pods_refresh: bool,
    pub loop_pause: u64,
    pub default_color: Rgb,
    pub color_cycle_len: u8,
    pub hue_intervals: Vec<HueInterval>,
    pub color_saturation: Saturation,
    pub color_lightness: Lightness,
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
        let default_color = self.get_default_color()?;
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

        return Ok(SettingsValidated {
            pod_search,
            kubeconfig,
            namespace: self.namespace,
            previous: self.previous,
            since_seconds: self.since_seconds,
            tail_lines: self.tail_lines,
            timestamps: self.timestamps,
            disable_pods_refresh: self.disable_pods_refresh,
            loop_pause: self.loop_pause,
            default_color,
            color_cycle_len: self.color_cycle_len,
            hue_intervals,
            color_saturation,
            color_lightness,
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

    pub fn get_default_color(&self) -> Result<Rgb, Errors> {
        let hsl = CustomHsl::from_str(self.default_color.as_str())?;
        match hsl.clone().validate() {
            Ok(val) => val,
            Err(err) => return Err(Errors::Validation(err.to_string())),
        };
        let lib_hsl = Hsl::from(hsl.h.value as f32, hsl.s.value as f32, hsl.l.value as f32);
        let rgb = lib_hsl.to_rgb();
        return Ok(rgb);
    }
}
