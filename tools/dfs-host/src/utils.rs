#[cfg(feature = "axstd")]
#[cfg_attr(feature = "axstd", path = "utils_axstd.rs")]
mod utils_impl;

pub(crate) use utils_impl::*;

#[cfg(not(feature = "axstd"))]
mod utils_impl {
    use std::{fs::FileType, io};

    use axerrno::AxError;

    use std::os::unix::prelude::FileTypeExt;

    pub use std::path::{Path, PathBuf};

    pub(crate) fn io_err_to_axerr(e: io::Error) -> AxError {
        match e.kind() {
            io::ErrorKind::NotFound => AxError::NotFound,
            io::ErrorKind::PermissionDenied => AxError::PermissionDenied,
            io::ErrorKind::ConnectionRefused => AxError::ConnectionRefused,
            io::ErrorKind::ConnectionReset => AxError::ConnectionReset,
            io::ErrorKind::NotConnected => AxError::NotConnected,
            io::ErrorKind::AddrInUse => AxError::AddrInUse,
            io::ErrorKind::AlreadyExists => AxError::AlreadyExists,
            io::ErrorKind::WouldBlock => AxError::WouldBlock,
            io::ErrorKind::InvalidInput => AxError::InvalidInput,
            io::ErrorKind::InvalidData => AxError::InvalidData,
            io::ErrorKind::WriteZero => AxError::WriteZero,
            io::ErrorKind::Unsupported => AxError::Unsupported,
            io::ErrorKind::UnexpectedEof => AxError::UnexpectedEof,
            _ => AxError::Unsupported,
        }
    }

    pub(crate) fn unix_ty_to_axty(t: FileType) -> u8 {
        match t {
            _ if t.is_fifo() => 0o1,
            _ if t.is_char_device() => 0o2,
            _ if t.is_dir() => 0o4,
            _ if t.is_block_device() => 0o6,
            _ if t.is_file() => 0o10,
            _ if t.is_symlink() => 0o12,
            _ if t.is_socket() => 0o14,
            _ => 0o0,
        }
    }
}
