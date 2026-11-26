//! Errors of the file infobase layer.

use std::fmt;
use std::io;

/// A failure while reading or writing a 1Cv8.1CD file.
#[derive(Debug)]
pub enum Error {
    /// The operating system refused a read, write or lock.
    Io(io::Error),
    /// The file is not a 1Cv8.1CD file of a supported format version.
    Unsupported(String),
    /// The file contradicts the format: a bad signature, a page or block
    /// number out of range, a broken chain. Nothing is written on this error.
    Corrupt(String),
    /// The infobase is open by the platform or another ibcmd-rs process.
    Busy(String),
    /// The caller asked for something the format layer does not do: an unknown
    /// table, a key outside the verified collation range.
    Refused(String),
    /// The database publication succeeded, but updating its backup failed.
    /// The pre-write database is retained at recovery; the previous backup is
    /// untouched. Callers must never report this outcome as cancellation.
    Published {
        recovery: std::path::PathBuf,
        error: io::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "{err}"),
            Error::Published { recovery, error } => write!(
                f,
                "database was published, but backup publication failed ({error}); previous backup is retained; pre-write recovery database remains at {}; inspect before retrying",
                recovery.display()
            ),
            Error::Unsupported(text)
            | Error::Corrupt(text)
            | Error::Busy(text)
            | Error::Refused(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) | Error::Published { error: err, .. } => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn corrupt(text: impl Into<String>) -> Error {
    Error::Corrupt(text.into())
}
