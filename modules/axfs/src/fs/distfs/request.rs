use axerrno::{ax_err_type, AxResult};
use axnet::TcpSocket;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use super::tcpio::{TcpIO, BINCODE_CONFIG};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct NodeAttr {
    /// note that it shall mirror the repr of [VfsNodePerm](axfs_vfs::structs::VfsNodePerm)
    pub mode: u16,
    /// note that it shall mirror the repr of [VfsNodeType](axfs_vfs::structs::VfsNodeType)
    pub ty: u8,
    pub size: u64,
    pub blocks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Encode)]
pub struct Read {
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Encode)]
pub struct Write<'write> {
    pub offset: u64,
    pub content: &'write [u8],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Encode)]
pub struct Trunc {
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Encode)]
pub struct Lookup<'s> {
    pub path: &'s str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Encode)]
pub struct Create<'s> {
    pub path: &'s str,
    /// note that it must mirror the repr of [VfsNodeType](axfs_vfs::structs::VfsNodeType)
    pub ty: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Encode)]
pub struct Remove<'s> {
    pub path: &'s str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Encode)]
pub struct ReadDir {
    pub start_idx: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Encode)]
pub struct Rename<'s> {
    pub src_path: &'s str,
    pub dst_path: &'s str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Encode)]
#[repr(u8)]
pub enum Action<'a> {
    Open,
    Release,
    GetAttr,

    Read(Read),
    Write(Write<'a>),
    Fsync,
    Trunc(Trunc),
    GetParent,

    Lookup(Lookup<'a>),
    Create(Create<'a>),
    Remove(Remove<'a>),
    ReadDir(ReadDir),
    Rename(Rename<'a>),
}

macro_rules! impl_into_action_impl {
    () => {};
    ( $ty:ident ) => {
        impl From<$ty> for Action<'_> {
            fn from(other: $ty) -> Self {
                Action::$ty(other)
            }
        }
    };
    ( $ty:ident < $lt:lifetime > ) => {
        impl<$lt> From<$ty<$lt>> for Action<$lt> {
            fn from(other: $ty<$lt>) -> Self {
                Action::$ty(other)
            }
        }
    };
}

macro_rules! impl_into_action {
    ( $( $ty:ident $(< $lt:lifetime >)? ),* $(,)? ) => {
        $(
            impl_into_action_impl!( $ty $(<$lt>)? );
        )*
    };
}

impl_into_action!(
    Read,
    Write<'a>,
    Trunc,
    Lookup<'a>,
    Create<'a>,
    Remove<'a>,
    ReadDir,
    Rename<'a>,
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Encode)]
pub struct Request<'s, 'a> {
    pub relpath: &'s str,
    pub action: Action<'a>,
}

impl<'s, 'a> Request<'s, 'a> {
    pub fn new(path: &'s str, action: Action<'a>) -> Self {
        Self {
            relpath: path,
            action,
        }
    }
}

pub fn send_fsop(req: Request, conn: &TcpSocket) -> AxResult {
    bincode::encode_into_writer(req, TcpIO(conn), BINCODE_CONFIG)
        .map_err(|e| ax_err_type!(Io, e))?;
    Ok(())
}

pub fn recv_fsop<T: Decode>(conn: &TcpSocket) -> AxResult<T> {
    let ret = bincode::decode_from_reader(TcpIO(conn), BINCODE_CONFIG)
        .map_err(|e| ax_err_type!(Io, e))?;
    Ok(ret)
}

pub fn recv_fsop_serde<T>(conn: &TcpSocket) -> AxResult<T>
where
    for<'de> T: Deserialize<'de>,
{
    let ret = bincode::serde::decode_from_reader(TcpIO(conn), BINCODE_CONFIG)
        .map_err(|e| ax_err_type!(Io, e))?;
    Ok(ret)
}

#[cfg(test)]
mod test {
    use super::*;

    use std::error::Error;

    type Result<T> = std::result::Result<T, Box<dyn Error>>;

    #[test]
    fn test_serde() -> Result<()> {
        Ok(())
    }
}
