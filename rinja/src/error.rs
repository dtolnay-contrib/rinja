use std::convert::Infallible;
use std::error::Error as StdError;
use std::{fmt, io};

/// The [`Result`](std::result::Result) type with [`Error`] as default error type
pub type Result<I, E = Error> = std::result::Result<I, E>;

/// rinja's error type
///
/// Used as error value for e.g. [`Template::render()`][crate::Template::render()]
/// and custom filters.
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// Generic, unspecified formatting error
    Fmt,
    /// An error raised by using `?` in a template
    Custom(Box<dyn StdError + Send + Sync>),
    /// JSON conversion error
    #[cfg(feature = "serde_json")]
    Json(serde_json::Error),
}

impl Error {
    /// Capture an [`StdError`]
    #[inline]
    pub fn custom(err: impl Into<Box<dyn StdError + Send + Sync>>) -> Self {
        Self::Custom(err.into())
    }

    /// Convert this [`Error`] into a
    /// <code>[Box]&lt;dyn [StdError] + [Send] + [Sync]&gt;</code>
    pub fn into_box(self) -> Box<dyn StdError + Send + Sync> {
        match self {
            Error::Fmt => fmt::Error.into(),
            Error::Custom(err) => err,
            #[cfg(feature = "serde_json")]
            Error::Json(err) => err.into(),
        }
    }

    /// Convert this [`Error`] into an [`io::Error`]
    ///
    /// Not this error itself, but the contained [`source`][StdError::source] is returned.
    pub fn into_io_error(self) -> io::Error {
        io::Error::other(match self {
            Error::Custom(err) => match err.downcast() {
                Ok(err) => return *err,
                Err(err) => err,
            },
            err => err.into_box(),
        })
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Fmt => Some(&fmt::Error),
            Error::Custom(err) => Some(err.as_ref()),
            #[cfg(feature = "serde_json")]
            Error::Json(err) => Some(err),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Fmt => fmt::Error.fmt(f),
            Error::Custom(err) => err.fmt(f),
            #[cfg(feature = "serde_json")]
            Error::Json(err) => err.fmt(f),
        }
    }
}

impl From<Error> for fmt::Error {
    #[inline]
    fn from(_: Error) -> Self {
        Self
    }
}

impl From<Error> for io::Error {
    #[inline]
    fn from(err: Error) -> Self {
        err.into_io_error()
    }
}

impl From<fmt::Error> for Error {
    #[inline]
    fn from(_: fmt::Error) -> Self {
        Error::Fmt
    }
}

/// This conversion inspects the argument and chooses the best fitting [`Error`] variant
impl From<Box<dyn StdError + Send + Sync>> for Error {
    #[inline]
    fn from(err: Box<dyn StdError + Send + Sync>) -> Self {
        error_from_stderror(err, MAX_ERROR_UNWRAP_COUNT)
    }
}

/// This conversion inspects the argument and chooses the best fitting [`Error`] variant
impl From<io::Error> for Error {
    #[inline]
    fn from(err: io::Error) -> Self {
        from_from_io_error(err, MAX_ERROR_UNWRAP_COUNT)
    }
}

const MAX_ERROR_UNWRAP_COUNT: usize = 5;

fn error_from_stderror(err: Box<dyn StdError + Send + Sync>, unwraps: usize) -> Error {
    let Some(unwraps) = unwraps.checked_sub(1) else {
        return Error::Custom(err);
    };
    match ErrorKind::inspect(err.as_ref()) {
        ErrorKind::Fmt => Error::Fmt,
        ErrorKind::Custom => Error::Custom(err),
        #[cfg(feature = "serde_json")]
        ErrorKind::Json => match err.downcast() {
            Ok(err) => Error::Json(*err),
            Err(_) => Error::Fmt, // unreachable
        },
        ErrorKind::Io => match err.downcast() {
            Ok(err) => from_from_io_error(*err, unwraps),
            Err(_) => Error::Fmt, // unreachable
        },
        ErrorKind::Rinja => match err.downcast() {
            Ok(err) => *err,
            Err(_) => Error::Fmt, // unreachable
        },
    }
}

fn from_from_io_error(err: io::Error, unwraps: usize) -> Error {
    let Some(inner) = err.get_ref() else {
        return Error::custom(err);
    };
    let Some(unwraps) = unwraps.checked_sub(1) else {
        return match err.into_inner() {
            Some(err) => Error::Custom(err),
            None => Error::Fmt, // unreachable
        };
    };
    match ErrorKind::inspect(inner) {
        ErrorKind::Fmt => Error::Fmt,
        ErrorKind::Rinja => match err.downcast() {
            Ok(err) => err,
            Err(_) => Error::Fmt, // unreachable
        },
        #[cfg(feature = "serde_json")]
        ErrorKind::Json => match err.downcast() {
            Ok(err) => Error::Json(err),
            Err(_) => Error::Fmt, // unreachable
        },
        ErrorKind::Custom => match err.into_inner() {
            Some(err) => Error::Custom(err),
            None => Error::Fmt, // unreachable
        },
        ErrorKind::Io => match err.downcast() {
            Ok(inner) => from_from_io_error(inner, unwraps),
            Err(_) => Error::Fmt, // unreachable
        },
    }
}

enum ErrorKind {
    Fmt,
    Custom,
    #[cfg(feature = "serde_json")]
    Json,
    Io,
    Rinja,
}

impl ErrorKind {
    fn inspect(err: &(dyn StdError + 'static)) -> ErrorKind {
        if err.is::<fmt::Error>() {
            ErrorKind::Fmt
        } else if err.is::<io::Error>() {
            ErrorKind::Io
        } else if err.is::<Error>() {
            ErrorKind::Rinja
        } else {
            #[cfg(feature = "serde_json")]
            if err.is::<serde_json::Error>() {
                return ErrorKind::Json;
            }
            ErrorKind::Custom
        }
    }
}

#[cfg(feature = "serde_json")]
impl From<serde_json::Error> for Error {
    #[inline]
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err)
    }
}

impl From<Infallible> for Error {
    #[inline]
    fn from(value: Infallible) -> Self {
        match value {}
    }
}

#[cfg(test)]
const _: () = {
    trait AssertSendSyncStatic: Send + Sync + 'static {}
    impl AssertSendSyncStatic for Error {}
};
