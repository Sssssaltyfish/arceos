mod bufreader;
mod bufwriter;

const DEFAULT_BUF_SIZE: usize = 1024;

pub use self::bufreader::BufReader;

#[cfg(feature = "alloc")]
pub use self::bufwriter::{BufWriter, WriterPanicked};

#[cfg(feature = "alloc")]
use alloc::fmt;

use super::Error;

/// An error returned by [`BufWriter::into_inner`] which combines an error that
/// happened while writing out the buffer, and the buffered writer object
/// which may be used to recover from the condition.
///
/// # Examples
///
/// ```no_run
/// use std::io::BufWriter;
/// use std::net::TcpStream;
///
/// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
///
/// // do stuff with the stream
///
/// // we want to get our `TcpStream` back, so let's try:
///
/// let stream = match stream.into_inner() {
///     Ok(s) => s,
///     Err(e) => {
///         // Here, e is an IntoInnerError
///         panic!("An error occurred");
///     }
/// };
/// ```
#[derive(Debug)]
pub struct IntoInnerError<W>(W, Error);

impl<W> IntoInnerError<W> {
    /// Construct a new IntoInnerError
    fn new(writer: W, error: Error) -> Self {
        Self(writer, error)
    }

    /// Returns the error which caused the call to [`BufWriter::into_inner()`]
    /// to fail.
    ///
    /// This error was returned when attempting to write the internal buffer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // do stuff with the stream
    ///
    /// // we want to get our `TcpStream` back, so let's try:
    ///
    /// let stream = match stream.into_inner() {
    ///     Ok(s) => s,
    ///     Err(e) => {
    ///         // Here, e is an IntoInnerError, let's log the inner error.
    ///         //
    ///         // We'll just 'log' to stdout for this example.
    ///         println!("{}", e.error());
    ///
    ///         panic!("An unexpected error occurred.");
    ///     }
    /// };
    /// ```

    pub fn error(&self) -> &Error {
        &self.1
    }

    /// Returns the buffered writer instance which generated the error.
    ///
    /// The returned object can be used for error recovery, such as
    /// re-inspecting the buffer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // do stuff with the stream
    ///
    /// // we want to get our `TcpStream` back, so let's try:
    ///
    /// let stream = match stream.into_inner() {
    ///     Ok(s) => s,
    ///     Err(e) => {
    ///         // Here, e is an IntoInnerError, let's re-examine the buffer:
    ///         let buffer = e.into_inner();
    ///
    ///         // do stuff to try to recover
    ///
    ///         // afterwards, let's just return the stream
    ///         buffer.into_inner().unwrap()
    ///     }
    /// };
    /// ```

    pub fn into_inner(self) -> W {
        self.0
    }

    /// Consumes the [`IntoInnerError`] and returns the error which caused the call to
    /// [`BufWriter::into_inner()`] to fail.  Unlike `error`, this can be used to
    /// obtain ownership of the underlying error.
    ///
    /// # Example
    /// ```no_run
    /// use std::io::{BufWriter, ErrorKind, Write};
    ///
    /// let mut not_enough_space = [0u8; 10];
    /// let mut stream = BufWriter::new(not_enough_space.as_mut());
    /// write!(stream, "this cannot be actually written").unwrap();
    /// let into_inner_err = stream.into_inner().expect_err("now we discover it's too small");
    /// let err = into_inner_err.into_error();
    /// assert_eq!(err.kind(), ErrorKind::WriteZero);
    /// ```

    pub fn into_error(self) -> Error {
        self.1
    }

    /// Consumes the [`IntoInnerError`] and returns the error which caused the call to
    /// [`BufWriter::into_inner()`] to fail, and the underlying writer.
    ///
    /// This can be used to simply obtain ownership of the underlying error; it can also be used for
    /// advanced error recovery.
    ///
    /// # Example
    /// ```no_run
    /// use std::io::{BufWriter, ErrorKind, Write};
    ///
    /// let mut not_enough_space = [0u8; 10];
    /// let mut stream = BufWriter::new(not_enough_space.as_mut());
    /// write!(stream, "this cannot be actually written").unwrap();
    /// let into_inner_err = stream.into_inner().expect_err("now we discover it's too small");
    /// let (err, recovered_writer) = into_inner_err.into_parts();
    /// assert_eq!(err.kind(), ErrorKind::WriteZero);
    /// assert_eq!(recovered_writer.buffer(), b"t be actually written");
    /// ```

    pub fn into_parts(self) -> (Error, W) {
        (self.1, self.0)
    }
}

#[cfg(feature = "alloc")]
impl<W> fmt::Display for IntoInnerError<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error().fmt(f)
    }
}
