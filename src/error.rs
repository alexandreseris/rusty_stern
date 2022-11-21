use std::fmt;

#[derive(Debug, Clone)]
pub struct ColorRGBError {
    pub message: String,
}

impl fmt::Display for ColorRGBError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.message)
    }
}

#[derive(Debug, Clone)]
pub struct ColorCycleError {
    pub message: String,
}

impl fmt::Display for ColorCycleError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.message)
    }
}

#[derive(Debug, Clone)]
pub struct LogError {
    pub message: String,
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.message)
    }
}

#[derive(Debug, Clone)]
pub struct GenericError {
    pub message: String,
}

impl fmt::Display for GenericError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.message)
    }
}
