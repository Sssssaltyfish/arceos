use std::net::TcpStream;
use std::sync::Arc;

use alloc::string::String;
use alloc::vec::Vec;
use axfs::distfs::BINCODE_CONFIG;
use dashmap::DashMap;

use crate::conn_utils::{
    deserialize_client_request_from_buff, deserialize_node_request_from_buff,
    deserialize_response_from_buff, read_data_from_conn, DfsServer,
    Tcpio,
};
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

    pub fn handle_conn(&mut self) {
        loop {
            self.message_queue.pop_to_work(|action| {
                bincode::serde::encode_into_writer(
                    action.clone(),
                    Tcpio(&mut self.conn),
                    BINCODE_CONFIG,
                )
                .expect(&format!(
                    "Error happen when writing back success response from connection: {:?}",
                    self.conn
                ));
                let mut buff = vec![0u8; 1024];
                let bytes_read = read_data_from_conn(&mut buff, &mut self.conn);
                let res = deserialize_response_from_buff(&buff, bytes_read);
                res
            })
        }
    }
}

impl DfsServer for DfsNodeInConn {
    fn get_tcp_stream(&mut self) -> &mut TcpStream {
        &mut self.conn
    }

    fn get_root_path(&self) -> PathBuf {
        self.root_path.clone()
    }
}

impl DfsNodeInConn {
    pub fn new(
        root_path: PathBuf,
        conn: TcpStream,
        file_index: Arc<DashMap<String, NodeID>>,
    ) -> Self {
        DfsNodeInConn {
            root_path,
            conn,
            file_index,
        }
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
                        Action::Open => self.handle_open(req.relpath),
                        Action::Release => self.handle_release(),
                        Action::GetAttr => self.handle_getattr(req.relpath),
                        Action::Read(read) => {
                            self.handle_read(req.relpath, read.offset, read.length)
                        }
                        Action::Write(write) => {
                            self.handle_write(req.relpath, write.offset, write.content)
                        }
                        Action::Fsync => self.handle_fsync(),
                        Action::Trunc(trunc) => self.handle_trunc(req.relpath, trunc.size),
                        Action::GetParent => self.handle_getparent(req.relpath),
                        Action::Lookup(lookup) => self.handle_lookup(req.relpath, lookup.path),
                        Action::Create(create) => self.handle_create(req.relpath, create.path),
                        Action::Remove(remove) => self.handle_remove(req.relpath, remove.path),
                        Action::ReadDir(readdir) => {
                            self.handle_readdir(req.relpath, readdir.start_idx)
                        }
                        Action::Rename(rename) => {
                            self.handle_rename(req.relpath, rename.src_path, rename.dst_path)
                        }
                    }
                }
                PeerAction::InsertIndex(insert_index) => self.handle_insert_index(insert_index),
                PeerAction::RemoveIndex(remove_index) => self.handle_remove_index(remove_index),
                PeerAction::UpdateIndex(update_index) => self.handle_update_index(update_index),
            }
        }
    }

    fn handle_update_index(&self, index_map: DashMap<String, String>) {
        let file_index = &self.file_index;
        for entry in index_map.iter() {
            let node_id = file_index
                .remove(entry.key())
                .ok_or_else(|| {
                    logger::error!(
                        "Unachieable circumstance: key doesn't exist in file index when updating!"
                    );
                })
                .unwrap()
                .1;
            file_index.insert(entry.value().clone(), node_id);
        }
    }

    fn handle_insert_index(&self, index_map: DashMap<String, NodeID>) {
        let file_index = &self.file_index;
        for entry in index_map.iter() {
            if file_index.contains_key(entry.key()) {
                logger::error!(
                    "Unachieable circumstance: key already exist in file index when inserting!"
                )
            }
            file_index.insert(entry.key().clone(), *entry.value());
        }
    }

    fn handle_remove_index(&self, index_map: Vec<String>) {
        let file_index = &self.file_index;
        for entry in index_map.iter() {
            let _ = file_index.remove(entry).ok_or_else(|| {
                logger::error!(
                    "Unachieable circumstance: key doesn't exist in file index when removing!"
                )
            });
        }
    }
}
