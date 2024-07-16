use std::{collections::HashMap, error::Error, fmt::Display};

use wonnx::utils::{InputTensor, OutputTensor};

pub type Inputs<'a> = HashMap<String, InputTensor<'a>>;
pub type Outputs = HashMap<String, OutputTensor>;

pub trait AsInputs {
    fn as_inputs<'a>(&'a self) -> Inputs<'a>;
}

#[derive(Debug)]
pub enum OutputsParseError {
    KeyNotFound(&'static str),
    InvalidDataType,
}

pub trait TryFromOutputs: TryFrom<Outputs, Error = OutputsParseError> {}
impl<T: TryFrom<Outputs, Error = OutputsParseError>> TryFromOutputs for T {}

impl Display for OutputsParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::KeyNotFound(key) => write!(f, "Key not found: {key}"),
            Self::InvalidDataType => write!(f, "Invalid data type of output"),
        }
    }
}

impl Error for OutputsParseError {}
