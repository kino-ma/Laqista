use std::{error::Error, marker::PhantomData};

use crate::{
    session::Session,
    tensor::{AsInputs, Outputs},
};

pub struct AbtsractServer<I: AsInputs, O: TryFrom<Outputs>> {
    session: Session,
    /// Phantom field for combine I/O types for this struct
    phantom: PhantomData<(I, O)>,
}

impl<I: AsInputs, O: TryFrom<Outputs>> AbtsractServer<I, O> {
    pub fn new(session: Session) -> Self {
        Self {
            session,
            phantom: PhantomData,
        }
    }

    pub async fn infer(&mut self, input: I) -> Result<O, Box<dyn Error>>
    where
        <O as TryFrom<Outputs>>::Error: std::error::Error + 'static,
    {
        let input = input.as_inputs();
        let output = self.session.detect(&input).await?;
        let reply = O::try_from(output)?;

        Ok(reply)
    }
}
