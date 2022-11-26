use std::collections::HashSet;
use std::io::Write;
use std::iter::Cycle;
use std::str::FromStr;
use std::sync::Arc;

use colors_transform::{Color as ColorTransform, Hsl, Rgb};
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};
use tokio::sync::Mutex;
use validator::Validate;

use crate::error::Errors;

#[derive(Debug, Validate, Clone)]
pub struct Saturation {
    #[validate(range(min = 0, max = 100))]
    pub value: u8,
}

#[derive(Debug, Validate, Clone)]
pub struct Lightness {
    #[validate(range(min = 0, max = 100))]
    pub value: u8,
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
    pub fn validate(self) -> Result<HueInterval, Errors> {
        if self.start.value >= self.end.value {
            return Err(Errors::Validation("start value must be greater than end value".to_string()));
        } else {
            return Ok(self);
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

async fn _print_color(std: &mut StandardStream, color_rgb: Rgb, message: String, newline: bool) -> Result<(), Errors> {
    match std.set_color(ColorSpec::new().set_fg(Some(Color::Rgb(
        color_rgb.get_red() as u8,
        color_rgb.get_green() as u8,
        color_rgb.get_blue() as u8,
    )))) {
        Ok(val) => val,
        Err(err) => return Err(Errors::StdErr(err.to_string())),
    };
    let mut newlinechar = "";
    if newline {
        newlinechar = "\n";
    }
    match std.write_fmt(format_args!("{message}{newlinechar}")) {
        Ok(val) => val,
        Err(err) => return Err(Errors::StdErr(err.to_string())),
    };
    Ok(())
}

pub async fn print_color(stdout: Arc<Mutex<(StandardStream, StandardStream)>>, color_rgb: Rgb, message: String, newline: bool) -> Result<(), Errors> {
    let mut stdout_locked = stdout.lock().await;
    let std = &mut stdout_locked.0;
    _print_color(std, color_rgb, message, newline).await
}

pub async fn eprint_color(
    stdout: Arc<Mutex<(StandardStream, StandardStream)>>,
    color_rgb: Rgb,
    message: String,
    newline: bool,
) -> Result<(), Errors> {
    let mut stdout_locked = stdout.lock().await;
    let std = &mut stdout_locked.1;
    _print_color(std, color_rgb, message, newline).await
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
    let hue_step = hue_count as u16 / cycle_len as u16;
    for step in 0..cycle_len {
        let current_hue_index = hue_step * step as u16;
        let current_hue = hue_values[current_hue_index as usize];
        let rgb = Hsl::from(current_hue as f32, saturation.value as f32, lightness.value as f32).to_rgb();
        colors.push(rgb);
    }
    return Ok(colors.into_iter().cycle());
}

pub async fn get_padding(running_pods: Arc<Mutex<HashSet<String>>>) -> usize {
    let pods = running_pods.lock().await;
    let mut max_len = 0;
    for pod in pods.iter() {
        let len = pod.len();
        if len > max_len {
            max_len = len;
        }
    }
    return max_len;
}
