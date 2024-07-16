use std::{convert::Infallible, error::Error as GenericError, io};

use mac_address::MacAddressError;
use tokio::sync::mpsc;
use tonic::{transport::Error as TransportError, Status};

#[derive(Debug)]
pub enum Error {
    AppInstantiation(String), // FIXME: This error should contain actual error type returned from the application
    Io(io::Error),
    Mac(MacAddressError),
    NoneError,
    RequestError(Status),
    SendStateError(mpsc::error::SendError<DaemonState>),
    TransportError(TransportError),
    Text(String),
    Url(ParseError),
    Uuid(uuid::Error),
    Other(Box<dyn GenericError + Send>),
}

pub type Result<T> = std::result::Result<T, Error>;

use url::ParseError;
use Error::*;

use crate::server::DaemonState;

impl Into<Status> for Error {
    fn into(self) -> Status {
        Status::aborted(self)
    }
}

impl Into<String> for Error {
    fn into(self) -> String {
        match self {
            AppInstantiation(err) => format!("error instantiating an application: {err}"),
            Io(err) => format!("io error: {err}"),
            Mac(err) => format!("error parsing a mac address: {err}"),
            NoneError => "an empty value occured somewhere".to_owned(),
            RequestError(err) => format!("request to another node failed: {err}"),
            SendStateError(err) => format!("failed to send state via tx: {err}"),
            TransportError(err) => format!("tonic transport error: {err}"),
            Text(err) => err,
            Url(err) => format!("error parsing an url: {err}"),
            Uuid(err) => format!("error parsing uuid: {err}"),
            Other(err) => err.to_string(),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Io(err)
    }
}

impl From<MacAddressError> for Error {
    fn from(err: MacAddressError) -> Self {
        Mac(err)
    }
}

impl From<TransportError> for Error {
    fn from(err: TransportError) -> Self {
        TransportError(err)
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Text(err.to_string())
    }
}

impl From<ParseError> for Error {
    fn from(err: ParseError) -> Self {
        Url(err)
    }
}

impl From<mpsc::error::SendError<DaemonState>> for Error {
    fn from(err: mpsc::error::SendError<DaemonState>) -> Self {
        SendStateError(err)
    }
}

impl From<Status> for Error {
    fn from(err: Status) -> Self {
        RequestError(err)
    }
}

impl From<uuid::Error> for Error {
    fn from(err: uuid::Error) -> Self {
        Uuid(err)
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Text(err.to_owned())
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        NoneError
    }
}
