use std::io::Write;
use std::iter::Cycle;
use std::str;
use std::str::FromStr;
use std::sync::Arc;
use std::{collections::HashSet, fmt};

use colors_transform::{Color as ColorTransform, Hsl};
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};
use tokio::sync::Mutex;

use crate::error::{ColorCycleError, ColorRGBError};

#[derive(Debug, Copy, Clone)]
pub struct ColorRGB(u8, u8, u8);

impl fmt::Display for ColorRGB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{}", self.0, self.1, self.2)
    }
}

impl FromStr for ColorRGB {
    type Err = ColorRGBError;
    fn from_str(s: &str) -> Result<Self, ColorRGBError> {
        let str_spl: Vec<&str> = s.split(",").collect();
        if str_spl.len() != 3 {
            return Err(ColorRGBError::new(
                "wrong color format, excpect 0-255,0-255,0-255".to_string(),
            ));
        }
        let r = match str_spl[0].parse::<u8>() {
            Ok(r) => r,
            Err(err) => {
                return Err(ColorRGBError::new(format!(
                    "failled to parse red color: {err}"
                )));
            }
        };
        let g = match str_spl[1].parse::<u8>() {
            Ok(g) => g,
            Err(err) => {
                return Err(ColorRGBError::new(format!(
                    "failled to parse green color: {err}"
                )));
            }
        };
        let b = match str_spl[2].parse::<u8>() {
            Ok(b) => b,
            Err(err) => {
                return Err(ColorRGBError::new(format!(
                    "failled to parse blue color: {err}"
                )));
            }
        };

        Ok(ColorRGB(r, g, b))
    }
}

pub async fn print_color(
    stdout: Arc<Mutex<StandardStream>>,
    color_rgb: ColorRGB,
    message: String,
    newline: bool,
) {
    let mut stdout_locked = stdout.lock().await;
    stdout_locked
        .set_color(ColorSpec::new().set_fg(Some(Color::Rgb(color_rgb.0, color_rgb.1, color_rgb.2))))
        .unwrap();
    if newline {
        stdout_locked
            .write_fmt(format_args!("{}\n", message))
            .unwrap();
    } else {
        stdout_locked
            .write_fmt(format_args!("{}", message))
            .unwrap();
    }
}

pub fn pick_color(color_cycle: &mut Cycle<std::vec::IntoIter<ColorRGB>>) -> ColorRGB {
    return color_cycle.next().unwrap();
}

pub fn build_color_cycle(
    cycle_len: u8,
    saturation: u8,
    lightness: u8,
) -> Result<Cycle<std::vec::IntoIter<ColorRGB>>, ColorCycleError> {
    if lightness > 100 {
        return Err(ColorCycleError::new(
            "lightness should be between 0 and 100".to_string(),
        ));
    }
    if saturation > 100 {
        return Err(ColorCycleError::new(
            "saturation should be between 0 and 100".to_string(),
        ));
    }
    let mut colors: Vec<ColorRGB> = Vec::new();
    let max_hue: u16 = 360;
    let hue_step = max_hue / cycle_len as u16;
    for step in 0..cycle_len {
        let current_hue = step as u16 * hue_step;
        let rgb = Hsl::from(current_hue as f32, saturation as f32, lightness as f32).to_rgb();
        colors.push(ColorRGB(
            rgb.get_red() as u8,
            rgb.get_green() as u8,
            rgb.get_blue() as u8,
        ));
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
