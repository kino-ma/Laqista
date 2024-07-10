use wonnx::utils::{InputTensor, OutputTensor};

use crate::tensor::{AsInputs, Inputs, Outputs};

tonic::include_proto!("face");

impl AsInputs for DetectRequest {
    fn as_inputs<'a>(&'a self) -> crate::tensor::Inputs<'a> {
        let data = (&self.data).into();
        let input = InputTensor::F32(data);
        Inputs::from([("data".to_owned(), input)])
    }
}

impl TryFrom<Outputs> for DetectReply {
    type Error = String;

    fn try_from(mut outputs: Outputs) -> Result<Self, Self::Error> {
        let data = outputs
            .remove("squeezenet0_flatten0_reshape0")
            .ok_or("Value not found for key".to_owned())?;

        if let OutputTensor::F32(coerced) = data {
            Ok(Self {
                squeezenet0_flatten0_reshape0: coerced,
            })
        } else {
            Err("Invalid data type for output".to_owned())
        }
    }
}
