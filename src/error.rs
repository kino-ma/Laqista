use std::{convert::Infallible, error::Error as GenericError, fmt::Display, io};

use mac_address::MacAddressError;
use tokio::sync::mpsc;
use tonic::{transport::Error as TransportError, Status};

#[derive(Debug)]
pub enum Error {
    AppInstantiation(String), // FIXME: This error should contain actual error type returned from the application
    Io(io::Error),
    Ip(local_ip_address::Error),
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

impl GenericError for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppInstantiation(err) => write!(f, "error instantiating an application: {err}"),
            Io(err) => write!(f, "io error: {err}"),
            Ip(err) => write!(f, "local ip address error: {err}"),
            Mac(err) => write!(f, "error parsing a mac address: {err}"),
            NoneError => write!(f, "an empty value occured somewhere"),
            RequestError(err) => write!(f, "request to another node failed: {err}"),
            SendStateError(err) => write!(f, "failed to send state via tx: {err}"),
            TransportError(err) => write!(f, "tonic transport error: {err}"),
            Text(err) => write!(f, "{}", err),
            Url(err) => write!(f, "error parsing an url: {err}"),
            Uuid(err) => write!(f, "error parsing uuid: {err}"),
            Other(err) => write!(f, "{}", err),
        }
    }
}

use url::ParseError;
use Error::*;

use crate::server::DaemonState;

impl Into<Status> for Error {
    fn into(self) -> Status {
        Status::aborted(self.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Io(err)
    }
}

impl From<local_ip_address::Error> for Error {
    fn from(err: local_ip_address::Error) -> Self {
        Ip(err)
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
