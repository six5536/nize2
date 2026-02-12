use thiserror::Error;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{}", .0)]
    Custom(String),

    #[error("IO::{:?}: {}", .0, .0)]
    Io(#[from] std::io::Error),

    #[error("Fmt::{:?}: {}", .0, .0)]
    Fmt(#[from] std::fmt::Error),

    #[error("FlexiLogger::{:?}: {}", .0, .0)]
    FlexiLogger(#[from] flexi_logger::FlexiLoggerError),
}
