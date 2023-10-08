use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::iter::Cycle;
use std::str::FromStr;
use std::sync::Arc;

use colors_transform::{Color as ColorTransform, Hsl as HslColorTransform, Rgb};
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};
use tokio::sync::Mutex;
use validator::Validate;

use crate::error::Errors;

#[derive(Debug, Validate, Clone)]
pub struct Saturation {
    #[validate(range(min = 0, max = 100))]
    pub value: u8,
}

impl FromStr for Saturation {
    type Err = Errors;
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let value = match string.parse::<u8>() {
            Ok(val) => val,
            Err(err) => return Err(Errors::Validation(err.to_string())),
        };
        return Ok(Saturation { value });
    }
}

#[derive(Debug, Validate, Clone)]
pub struct Lightness {
    #[validate(range(min = 0, max = 100))]
    pub value: u8,
}

impl FromStr for Lightness {
    type Err = Errors;
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let value = match string.parse::<u8>() {
            Ok(val) => val,
            Err(err) => return Err(Errors::Validation(err.to_string())),
        };
        return Ok(Lightness { value });
    }
}

#[derive(Debug, Validate, Clone)]
pub struct Hue {
    #[validate(range(min = 0, max = 359))]
    pub value: u16,
}

impl FromStr for Hue {
    type Err = Errors;
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let int_value = match string.parse::<u16>() {
            Ok(val) => val,
            Err(err) => return Err(Errors::Validation(format!("failled to parse {string}: {err}"))),
        };
        let hue = Hue { value: int_value };
        return match hue.validate() {
            Ok(_) => Ok(hue),
            Err(err) => return Err(Errors::Validation(format!("failled to parse {string}: {err}"))),
        };
    }
}

#[derive(Clone)]
pub struct HueInterval {
    pub start: Hue,
    pub end: Hue,
}

impl HueInterval {
    pub fn validate(self) -> Result<(), Errors> {
        if self.start.value >= self.end.value {
            return Err(Errors::Validation("start value must be greater than end value".to_string()));
        } else {
            return Ok(());
        }
    }
}

impl FromStr for HueInterval {
    type Err = Errors;
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let interval: Vec<&str> = string.split("-").collect();
        if interval.len() != 2 {
            return Err(Errors::Validation(format!("excpected 2 elements, found {} for {string}", interval.len())));
        }
        let start = Hue::from_str(interval[0])?;
        let end = Hue::from_str(interval[1])?;

        if start.value > 359 || end.value > 359 || start.value >= end.value {
            return Err(Errors::Validation(format!(
                "failled to parse {string}: format excpected => 0 <= value <= 359 && start < end"
            )));
        }
        return Ok(HueInterval { start, end });
    }
}

#[derive(Debug, Clone)]
pub struct Hsl {
    pub h: Hue,
    pub s: Saturation,
    pub l: Lightness,
}

impl Hsl {
    pub fn validate(self) -> Result<(), validator::ValidationErrors> {
        self.h.validate()?;
        self.s.validate()?;
        self.l.validate()?;
        Ok(())
    }
}

impl FromStr for Hsl {
    type Err = Errors;
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let params: Vec<&str> = string.split(",").collect();
        if params.len() != 3 {
            return Err(Errors::Validation(format!("excpected 3 elements, found {} for {string}", params.len())));
        }
        let h = Hue::from_str(params[0])?;
        let s = Saturation::from_str(params[1])?;
        let l = Lightness::from_str(params[2])?;
        return Ok(Hsl { h, s, l });
    }
}

async fn _print_color(std: &mut StandardStream, color_rgb: Option<Rgb>, message: String) -> Result<(), Errors> {
    let mut message = message;
    if message.len() > 0 && message.chars().last().unwrap().to_string() != "\n" {
        message = format!("{message}\n");
    }
    match color_rgb {
        Some(color_rgb) => {
            match std.set_color(ColorSpec::new().set_fg(Some(Color::Rgb(
                color_rgb.get_red() as u8,
                color_rgb.get_green() as u8,
                color_rgb.get_blue() as u8,
            )))) {
                Ok(val) => val,
                Err(err) => return Err(Errors::StdErr(err.to_string())),
            };
        }

        None => match std.set_color(&ColorSpec::default()) {
            Ok(val) => val,
            Err(err) => return Err(Errors::StdErr(err.to_string())),
        },
    }
    match std.write_fmt(format_args!("{message}")) {
        Ok(val) => val,
        Err(err) => return Err(Errors::StdErr(err.to_string())),
    };
    Ok(())
}

pub async fn print_color(stdout: Arc<Mutex<(StandardStream, StandardStream)>>, color_rgb: Option<Rgb>, message: String) -> Result<(), Errors> {
    let mut stdout_locked = stdout.lock().await;
    let std = &mut stdout_locked.0;
    _print_color(std, color_rgb, message).await
}

pub async fn eprint_color(stdout: Arc<Mutex<(StandardStream, StandardStream)>>, message: String) -> Result<(), Errors> {
    let mut stdout_locked = stdout.lock().await;
    let std = &mut stdout_locked.1;
    _print_color(std, None, message).await
}

pub fn pick_color(color_cycle: &mut Cycle<std::vec::IntoIter<Rgb>>) -> Rgb {
    return color_cycle.next().unwrap(); // cycle should never return Err
}

pub fn build_color_cycle(
    cycle_len: u8,
    saturation: Saturation,
    lightness: Lightness,
    hue_intervals: Vec<HueInterval>,
) -> Result<Cycle<std::vec::IntoIter<Rgb>>, Errors> {
    let mut colors: Vec<Rgb> = Vec::new();
    let mut hue_values: Vec<u16> = Vec::new();
    for interval in hue_intervals {
        for val in interval.start.value..interval.end.value + 1 {
            hue_values.push(val);
        }
    }

    let hue_count = hue_values.len();
    let mut cycle_len = cycle_len;
    if cycle_len == 0 {
        cycle_len = 1;
    }
    let hue_step = hue_count as u16 / cycle_len as u16;
    for step in 0..cycle_len {
        let current_hue_index = hue_step * step as u16;
        let current_hue = hue_values[current_hue_index as usize];
        let hsl = HslColorTransform::from(current_hue as f32, saturation.value as f32, lightness.value as f32);
        let rgb = hsl.to_rgb();
        colors.push(rgb);
    }
    return Ok(colors.into_iter().cycle());
}

pub async fn get_padding(running_pods: Arc<Mutex<HashMap<String, HashSet<String>>>>) -> (usize, bool) {
    let mut print_namespace = true;
    let running_pods_lock = running_pods.lock().await;
    let namespace_cnt = running_pods_lock.len();
    if namespace_cnt <= 1 {
        print_namespace = false;
    }
    let mut max_len = 0;
    for (namespace, pods) in running_pods_lock.iter() {
        for pod in pods {
            let mut len = pod.len();
            if print_namespace {
                len += namespace.len();
            }
            if len > max_len {
                max_len = len;
            }
        }
    }
    return (max_len, print_namespace);
}
