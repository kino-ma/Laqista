use std::error::Error as GenericError;

use tonic::transport::Error as TransportError;

#[derive(Debug)]
pub enum Error {
    TransportError(TransportError),
    Other(Box<dyn GenericError>),
}

pub type Result<T> = std::result::Result<T, Error>;

use Error::*;

impl<E> From<E> for Error
where
    E: GenericError,
{
    fn from(err: E) -> Self {
        Other(Box::new(err))
    }
}
