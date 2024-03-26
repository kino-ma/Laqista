use std::error::Error;
use std::io::Error as IOError;

// use tonic::{transport::Server, Request, Response, Status};

use crate::proto;

pub struct Server {}

impl Server {
    pub fn new() -> Self {
        return Self {};
    }

    pub fn start(self) -> Result<(), impl Error> {
        println!("starting the server!");
        return Ok::<(), IOError>(());
    }
}
