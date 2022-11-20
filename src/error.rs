use std::fmt;

#[derive(Debug, Clone)]
pub struct ColorRGBError {
    message: String,
}

impl ColorRGBError {
    pub fn new(message: String) -> ColorRGBError {
        ColorRGBError { message }
    }
}

impl fmt::Display for ColorRGBError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.message)
    }
}

#[derive(Debug, Clone)]
pub struct ColorCycleError {
    message: String,
}

impl ColorCycleError {
    pub fn new(message: String) -> ColorCycleError {
        ColorCycleError { message }
    }
}

impl fmt::Display for ColorCycleError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.message)
    }
}

#[derive(Debug, Clone)]
pub struct LogError {
    message: String,
}

impl LogError {
    pub fn new(message: String) -> LogError {
        LogError { message }
    }
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.message)
    }
}

#[derive(Debug, Clone)]
pub struct GenericError {
    message: String,
}

impl GenericError {
    pub fn new(message: String) -> GenericError {
        GenericError { message }
    }
}

impl fmt::Display for GenericError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.message)
    }
}
