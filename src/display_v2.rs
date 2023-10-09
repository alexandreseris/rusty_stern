use std::collections::HashSet;
use std::io::Write;
use std::str::FromStr;

use colors_transform::Color as ColorTransform;
use termcolor::WriteColor;
use validator::Validate;

use crate::error::Errors;
use crate::types;

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

#[derive(Clone)]
pub struct ColorParams {
    pod_cnt: u8,
    saturation: Saturation,
    lightness: Lightness,
    hue_intervals: Vec<Hue>,
    state: ColorGeneratorState,
}

#[derive(Clone)]
struct ColorGeneratorState {
    step: u16,
    offset: u16,
    pod_cnt: u16,
    hue_count: u16,
    hue_generated: HashSet<u16>,
}

impl ColorParams {
    pub fn new(settings: &crate::settings_v2::SettingsValidated, pod_cnt: usize) -> ColorParams {
        let mut hue_values = vec![];
        for interval in settings.hue_intervals.iter() {
            for val in interval.start.value..interval.end.value + 1 {
                hue_values.push(Hue { value: val });
            }
        }
        return ColorParams {
            pod_cnt: pod_cnt as u8,
            saturation: settings.color_saturation.clone(),
            lightness: settings.color_lightness.clone(),
            hue_intervals: hue_values.clone(),
            state: ColorGeneratorState {
                step: 0,
                offset: 0,
                pod_cnt: pod_cnt as u16,
                hue_count: hue_values.len() as u16,
                hue_generated: HashSet::new(),
            },
        };
    }
    pub fn init_colors(&mut self) -> Vec<colors_transform::Rgb> {
        let mut colors: Vec<colors_transform::Rgb> = Vec::new();
        for _ in 0..self.pod_cnt {
            colors.push(self.next_color());
        }
        return colors;
    }
    fn next_color(&mut self) -> colors_transform::Rgb {
        if self.state.step >= self.state.pod_cnt {
            self.state.offset = (self.state.hue_count / self.state.pod_cnt) / 2;
            self.state.step = 1;
            let new_cycle_len = self.state.pod_cnt * 2;
            if new_cycle_len < self.state.hue_count {
                self.state.pod_cnt = new_cycle_len;
            }
        }
        let hue_step = self.state.hue_count / self.state.pod_cnt;

        let current_hue_index = std::cmp::min(hue_step * self.state.step, self.state.hue_count - 1);
        let current_hue = &self.hue_intervals[current_hue_index as usize];
        if self.state.hue_generated.contains(&current_hue.value) {
            self.state.step += 1;
            return self.next_color();
        }
        self.state.hue_generated.insert(current_hue.value);
        let hsl = colors_transform::Hsl::from(current_hue.value as f32, self.saturation.value as f32, self.lightness.value as f32);
        self.state.step += 1;
        return hsl.to_rgb();
    }
}

#[derive(Clone)]
pub struct Colors {
    available: Vec<colors_transform::Rgb>,
    used: Vec<colors_transform::Rgb>,
    colors_param: ColorParams,
}

impl Colors {
    pub fn new(colors_param: &mut ColorParams) -> Colors {
        let colors = colors_param.init_colors();
        return Colors {
            available: colors,
            used: vec![],
            colors_param: colors_param.clone(),
        };
    }

    pub fn get_new_color(&mut self) -> colors_transform::Rgb {
        let color = self.available.pop().unwrap_or(self.colors_param.next_color());
        self.used.push(color);
        return color;
    }

    pub fn set_color_to_unused(&mut self, color: colors_transform::Rgb) {
        self.used
            .remove(self.used.iter().position(|item| item.as_tuple() == color.as_tuple()).unwrap()); // item cannot missing from used
        self.available.push(color);
    }
}

pub struct Streams {
    pub out: termcolor::StandardStream,
    pub err: termcolor::StandardStream,
}

pub fn new_streams() -> Streams {
    return Streams {
        out: termcolor::StandardStream::stdout(termcolor::ColorChoice::Always),
        err: termcolor::StandardStream::stderr(termcolor::ColorChoice::Always),
    };
}

pub fn new_streams_mutex(streams: Streams) -> types::ArcMutex<Streams> {
    return std::sync::Arc::new(tokio::sync::Mutex::new(streams));
}

pub async fn print_color(std: &mut termcolor::StandardStream, color_rgb: Option<colors_transform::Rgb>, message: String) -> Result<(), Errors> {
    let mut message = message;
    if let Some(last_char) = message.chars().last() {
        if last_char.to_string() != "\n" {
            message = format!("{message}\n");
        }
    }
    let color_spec = match color_rgb {
        Some(color_rgb) => {
            let mut spec = termcolor::ColorSpec::new();
            spec.set_fg(Some(termcolor::Color::Rgb(
                color_rgb.get_red() as u8,
                color_rgb.get_green() as u8,
                color_rgb.get_blue() as u8,
            )));
            spec
        }
        None => termcolor::ColorSpec::default(),
    };

    match std.set_color(&color_spec) {
        Ok(_) => (),
        Err(err) => return Err(Errors::StdErr(err.to_string())),
    };

    match std.write_fmt(format_args!("{message}")) {
        Ok(val) => val,
        Err(err) => return Err(Errors::StdErr(err.to_string())),
    };
    Ok(())
}

#[allow(dead_code)]
pub async fn reset_terminal_colors(stdout: &mut termcolor::StandardStream, stderr: &mut termcolor::StandardStream) -> Result<(), Errors> {
    match stdout.set_color(&termcolor::ColorSpec::default()) {
        Ok(_) => (),
        Err(err) => return Err(Errors::StdErr(err.to_string())),
    };
    match stderr.set_color(&termcolor::ColorSpec::default()) {
        Ok(_) => (),
        Err(err) => return Err(Errors::StdErr(err.to_string())),
    };

    match stdout.write_fmt(format_args!("bye")) {
        Ok(val) => val,
        Err(err) => return Err(Errors::StdErr(err.to_string())),
    };
    match stderr.write_fmt(format_args!("bye")) {
        Ok(val) => val,
        Err(err) => return Err(Errors::StdErr(err.to_string())),
    };
    Ok(())
}
