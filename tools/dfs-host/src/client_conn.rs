use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;

#[cfg(not(feature = "axstd"))]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use alloc::{
    string::{String, ToString},
    vec,
};

use axfs::distfs::request::Action;

use bincode::enc::write::Writer;
use dashmap::DashMap;

#[cfg(feature = "axstd")]
use crate::utils::*;

use crate::conn_utils::*;
use crate::host::NodeID;
use crate::queue_request::MessageQueue;
use crate::utils::PathBuf;

pub struct DfsClientConn {
    root_path: PathBuf,
    conn: TcpStream,
    peers: Arc<DashMap<NodeID, Arc<MessageQueue>>>,
    file_index: Arc<DashMap<String, NodeID>>,
}

pub(super) struct Tcpio<'a>(pub &'a mut TcpStream);

impl Writer for Tcpio<'_> {
    fn write(&mut self, bytes: &[u8]) -> core::result::Result<(), bincode::error::EncodeError> {
        self.0
            .write_all(bytes)
            .map_err(|e| bincode::error::EncodeError::OtherString(e.to_string()))
    }
}

impl DfsServer for DfsClientConn {
    fn get_tcp_stream(&mut self) -> &mut TcpStream {
        &mut self.conn
    }

    fn get_root_path(&self) -> PathBuf {
        self.root_path.clone()
    }
}

impl DfsClientConn {
    pub fn new(
        root_path: PathBuf,
        conn: TcpStream,
        peers: Arc<DashMap<NodeID, Arc<MessageQueue>>>,
        file_index: Arc<DashMap<String, NodeID>>,
    ) -> Self {
        DfsClientConn {
            root_path,
            conn,
            peers,
            file_index,
        }
    }

    pub fn handle_conn(&mut self) {
        // mount 127.0.0.1:8000 /dist distfs
        // Continuously read data from the TcpStream and store it in the buff.
        loop {
            let mut buff = vec![0u8; 1024];
            let bytes_read = read_data_from_conn(&mut buff, &mut self.conn);
            if bytes_read == 0 {
                // Todo: close connection here
                return;
            }
            let req = deserialize_client_request_from_buff(&buff, bytes_read);
            logger::debug!("Received client request: {:?}", req);
            match req.action {
                Action::Open => self.handle_open(req.relpath),
                Action::Release => self.handle_release(),
                Action::GetAttr => self.handle_getattr(req.relpath),
                Action::Read(read) => self.handle_read(req.relpath, read.offset, read.length),
                Action::Write(write) => self.handle_write(req.relpath, write.offset, write.content),
                Action::Fsync => self.handle_fsync(),
                Action::Trunc(trunc) => self.handle_trunc(req.relpath, trunc.size),
                Action::GetParent => self.handle_getparent(req.relpath),
                Action::Lookup(lookup) => self.handle_lookup(req.relpath, lookup.path),
                Action::Create(create) => self.handle_create(req.relpath, create.path),
                Action::Remove(remove) => self.handle_remove(req.relpath, remove.path),
                Action::ReadDir(readdir) => self.handle_readdir(req.relpath, readdir.start_idx),
                Action::Rename(rename) => {
                    self.handle_rename(req.relpath, rename.src_path, rename.dst_path)
                }
            }
        }
        // You can also write data back to the connection if required.
        // For example:
        // self.conn.write_all(b"Response data").expect("Failed to write data");

        // Return the total number of bytes read.
    }
}
