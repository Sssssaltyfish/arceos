use std::{
    format,
    fs::{self, FileType},
    io,
    net::{TcpListener, TcpStream},
    sync::MutexGuard,
};

use alloc::{borrow::Cow, string::String};

use axerrno::AxError;

pub(crate) type PathBuf = String;
pub(crate) type Path = str;

pub(crate) fn io_err_to_axerr(e: io::Error) -> AxError {
    e
}

pub(crate) fn unix_ty_to_axty(t: FileType) -> u8 {
    t as u8
}

pub(crate) trait AsPath: AsRef<Path> {
    fn new<S>(s: &S) -> &Path
    where
        S: AsRef<Path> + ?Sized,
    {
        s.as_ref()
    }

    fn join<S>(&self, s: S) -> PathBuf
    where
        S: AsRef<Path>,
    {
        format!("{}/{}", self.as_ref().trim_end_matches('/'), s.as_ref())
    }

    fn to_string_lossy(&self) -> Cow<'_, Path> {
        Cow::Borrowed(self.as_ref())
    }

    fn is_dir(&self) -> bool {
        let Ok(meta) = fs::metadata(self.as_ref()) else {
            return false;
        };
        meta.is_dir()
    }

    fn is_file(&self) -> bool {
        let Ok(meta) = fs::metadata(self.as_ref()) else {
            return false;
        };
        meta.is_file()
    }

    fn parent(&self) -> Option<&Path> {
        let path = self.as_ref().trim_end_matches('/');
        path.rfind('/').map(|last_idx| &path[..last_idx])
    }
}

impl AsPath for Path {}

pub(crate) trait UnwrapSelf: Sized {
    fn unwrap(self) -> Self {
        self
    }
}

impl<T> UnwrapSelf for MutexGuard<'_, T> {}
impl UnwrapSelf for FileType {}

pub(crate) struct Incoming<'a> {
    listener: &'a TcpListener,
}

impl Iterator for Incoming<'_> {
    type Item = Result<TcpStream, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.listener.accept().map(|(stream, _)| stream);
        Some(ret)
    }
}

pub(crate) trait IncomingImpl {
    fn incoming(&self) -> Incoming<'_>;
}

impl IncomingImpl for TcpListener {
    fn incoming(&self) -> Incoming<'_> {
        Incoming { listener: self }
    }
}
