use std::net::TcpStream;
use std::sync::Arc;

use alloc::string::String;
use alloc::vec::Vec;
use dashmap::DashMap;

use crate::conn_utils::{deserialize_node_request_from_buff, read_data_from_conn, deserialize_client_request_from_buff};
use crate::host::NodeID;
use crate::queue_request::{MessageQueue, PeerAction};
use crate::utils::*;
use axfs::distfs::request::Action;

pub struct DfsNodeOutConn {
    conn: TcpStream,
    message_queue: Arc<MessageQueue>,
}

pub struct DfsNodeInConn {
    root_path: PathBuf,
    conn: TcpStream,
    file_index: Arc<DashMap<String, NodeID>>,
}

impl DfsNodeOutConn {
    pub fn new(conn: TcpStream, mq: Arc<MessageQueue>) -> Self {
        DfsNodeOutConn {
            conn,
            message_queue: mq,
        }
    }

    pub fn handle_conn(&mut self) {}
}

impl DfsNodeInConn {
    pub fn new(root_path: PathBuf, conn: TcpStream, file_index: Arc<DashMap<String, NodeID>>) -> Self {
        DfsNodeInConn { root_path, conn, file_index }
    }

    pub fn handle_conn(&mut self) {
        loop {
            let mut buff = vec![0u8; 1024];
            let bytes_read = read_data_from_conn(&mut buff, &mut self.conn);
            if bytes_read == 0 {
                // Todo: close connection here
                return;
            }
            let req = deserialize_node_request_from_buff(&buff, bytes_read);
            logger::debug!("Recieved peer request: {:?}", req);
            match req {
                PeerAction::SerializedAction(action) => {
                    let req = deserialize_client_request_from_buff(&action, action.len());
                    match req.action {
                        Action::Open => self.handle_(),
                        Action::Release => self.handle_(),
                        Action::GetAttr => self.handle_(),
                        Action::Read(read) => self.handle_(),
                        Action::Write(write) => self.handle_(),
                        Action::Fsync => self.handle_(),
                        Action::Trunc(trunc) => self.handle_(),
                        Action::GetParent => self.handle_(),
                        Action::Lookup(lookup) => self.handle_(),
                        Action::Create(create) => self.handle_(),
                        Action::Remove(remove) => self.handle_(),
                        Action::ReadDir(readdir) => self.handle_(),
                        Action::Rename(rename) => self.handle_(),
                    }
                },
                PeerAction::UpdateIndex(update_index) => {
                    self.handle_update_index(update_index.index)
                }
                PeerAction::RemoveIndex(remove_index) => {
                    self.handle_remove_index(remove_index)
                }
            }
        }
    }

    fn handle_(&self) {}

    fn handle_update_index(&self, index_map: DashMap<String, NodeID>) {
        let file_index = &self.file_index;
        for entry in index_map.iter() {
            if file_index.contains_key(entry.key()) {
                logger::error!("Unachieable circumstance: key already exist in file index when updating!")
            }
            file_index.insert(entry.key().clone(), *entry.value());
        }
    }

    fn handle_remove_index(&self, index_map: Vec<String>) {
        let file_index = &self.file_index;
        for entry in index_map.iter() {
            let _ = file_index.remove(entry).ok_or_else(|| {
                logger::error!("Unachieable circumstance: key doesn't exist in file index when removing!")
            });
        }
    }
}
