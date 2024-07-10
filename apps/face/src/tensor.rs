use std::collections::HashMap;

use wonnx::utils::{InputTensor, OutputTensor};

pub type Inputs<'a> = HashMap<String, InputTensor<'a>>;
pub type Outputs = HashMap<String, OutputTensor>;

pub trait AsInputs {
    fn as_inputs<'a>(&'a self) -> Inputs<'a>;
}
