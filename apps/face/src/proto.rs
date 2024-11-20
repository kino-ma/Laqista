use laqista_core::tensor::{AsInputs, Inputs, Outputs, OutputsParseError};
use wonnx::utils::{InputTensor, OutputTensor};

tonic::include_proto!("face");

impl AsInputs for InferRequest {
    fn as_inputs<'a>(&'a self) -> Inputs<'a> {
        let data = (&self.data).into();
        let input = InputTensor::F32(data);
        Inputs::from([("data".to_owned(), input)])
    }
}

const SQUEEZENET_OUTPUT: &'static str = "squeezenet0_flatten0_reshape0";

impl TryFrom<Outputs> for InferReply {
    type Error = OutputsParseError;

    fn try_from(mut outputs: Outputs) -> Result<Self, Self::Error> {
        use OutputsParseError::*;

        let data = outputs
            .remove(SQUEEZENET_OUTPUT)
            .ok_or(KeyNotFound(SQUEEZENET_OUTPUT))?;

        if let OutputTensor::F32(coerced) = data {
            Ok(Self {
                squeezenet0_flatten0_reshape0: coerced,
            })
        } else {
            Err(InvalidDataType)
        }
    }
}
