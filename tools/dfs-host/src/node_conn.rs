use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;

use alloc::string::String;
use alloc::vec::Vec;
use axfs::distfs::BINCODE_CONFIG;
use dashmap::DashMap;

use crate::conn_utils::{
    deserialize_client_request_from_buff, deserialize_node_request_from_buff, read_data_from_conn,
    send_ok_to_conn_serialized, send_vec_to_conn, DfsServer, Tcpio, END_SERIAL,
};
use crate::host::NodeID;
use crate::queue_request::{MessageQueue, PeerAction, ResponseFromPeer};
use crate::utils::*;
use axfs::distfs::request::{Action, Response};

pub fn contains_end_sequence(slice: &[u8]) -> Option<usize> {
    for i in 0..slice.len() {
        if i + 3 < slice.len() && slice[i..i + 4] == END_SERIAL {
            return Some(i);
        }
    }
    None
}

pub struct DfsNodeOutConn {
    conn: TcpStream,
    message_queue: Arc<MessageQueue>,
}

pub struct DfsNodeInConn {
    node_id: NodeID,
    root_path: PathBuf,
    conn: TcpStream,
    peers: Arc<DashMap<NodeID, Arc<MessageQueue>>>,
    file_index: Arc<DashMap<String, NodeID>>,
}

impl DfsNodeOutConn {
    pub fn new(conn: TcpStream, mq: Arc<MessageQueue>) -> Self {
        DfsNodeOutConn {
            conn,
            message_queue: mq,
        }
    }

    pub fn get_tcp_stream(&mut self) -> &mut TcpStream {
        &mut self.conn
    }

    pub fn handle_conn(&mut self) {
        loop {
            self.message_queue.pop_to_work(|action| {
                let action_vec = bincode::serde::encode_to_vec(action, BINCODE_CONFIG)
                    .expect("Error happen when serializing peer action.");
                self.conn
                    .write_all(&action_vec)
                    .expect("Error happen when sending serialized peer action.");
                let mut res_buff: Vec<Vec<u8>> = Vec::new();
                let mut buff = vec![0u8; 1024];
                loop {
                    let bytes_read = read_data_from_conn(&mut buff, &mut self.conn);
                    logger::debug!("Recieved {} bytes of data", bytes_read);
                    match contains_end_sequence(&buff[..bytes_read]) {
                        Some(i) => {
                            if i != 0 {
                                res_buff.push(buff[..i].to_vec());
                            }
                            break;
                        }
                        None => res_buff.push(buff[..bytes_read].to_vec()),
                    }
                }
                res_buff
            })
        }
    }

    pub fn send_init_index(&self, file_index: Arc<DashMap<String, NodeID>>) {
        let init_index = DashMap::new();
        for entry in file_index.iter() {
            init_index.insert(entry.key().clone(), *entry.value());
        }
        let _ = self
            .message_queue
            .submit_and_wait(PeerAction::InitIndex(init_index));
    }
}

impl DfsServer for DfsNodeInConn {
    fn get_node_id(&self) -> NodeID {
        self.node_id
    }

    fn get_peers(&self) -> Arc<DashMap<NodeID, Arc<MessageQueue>>> {
        self.peers.clone()
    }

    fn get_file_index(&self) -> Arc<DashMap<String, NodeID>> {
        self.file_index.clone()
    }

    fn get_tcp_stream(&mut self) -> &mut TcpStream {
        &mut self.conn
    }

    fn get_root_path(&self) -> PathBuf {
        self.root_path.clone()
    }
}

impl DfsNodeInConn {
    pub fn new(
        node_id: NodeID,
        root_path: PathBuf,
        conn: TcpStream,
        peers: Arc<DashMap<NodeID, Arc<MessageQueue>>>,
        file_index: Arc<DashMap<String, NodeID>>,
    ) -> Self {
        DfsNodeInConn {
            node_id,
            root_path,
            conn,
            peers,
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
                    logger::debug!("Recieved file op from peer request: {:?}", req);
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
                        Action::Create(create) => {
                            self.handle_create(req.relpath, create.path);
                            self.handle_insert_index(req.relpath, create.path);
                        }
                        Action::Remove(remove) => {
                            self.handle_remove(req.relpath, remove.path);
                            self.handle_remove_index(req.relpath, remove.path);
                        }
                        Action::ReadDir(readdir) => {
                            self.handle_readdir(req.relpath, readdir.start_idx)
                        }

                        Action::Rename(rename) => {
                            self.handle_rename(req.relpath, rename.src_path, rename.dst_path);
                            self.handle_update_index(req.relpath, rename.src_path, rename.dst_path);
                        }
                    }
                }
                PeerAction::InsertIndex(insert_index) => {
                    self.handle_outer_insert_index(insert_index);
                    send_ok_to_conn_serialized(&mut self.conn, ());
                }
                PeerAction::RemoveIndex(remove_index) => {
                    self.handle_outer_remove_index(remove_index);
                    send_ok_to_conn_serialized(&mut self.conn, ());
                }
                PeerAction::UpdateIndex(update_index) => {
                    self.handle_outer_update_index(update_index);
                    send_ok_to_conn_serialized(&mut self.conn, ());
                }
                PeerAction::InitIndex(init_index) => {
                    self.handle_outer_init_index(init_index);
                    send_ok_to_conn_serialized(&mut self.conn, ());
                }
            }
            send_vec_to_conn(self.get_tcp_stream(), END_SERIAL.to_vec());
            logger::debug!("End serial sent succeed.")
        }
    }

    fn handle_outer_update_index(&self, index_map: DashMap<String, String>) {
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

    fn handle_outer_insert_index(&self, index_map: DashMap<String, NodeID>) {
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

    fn handle_outer_remove_index(&self, index_map: Vec<String>) {
        let file_index = &self.file_index;
        for entry in index_map.iter() {
            let _ = file_index.remove(entry).ok_or_else(|| {
                logger::error!(
                    "Unachieable circumstance: key doesn't exist in file index when removing!"
                )
            });
        }
    }

    fn handle_outer_init_index(&mut self, index_map: DashMap<String, NodeID>) {
        self.file_index = index_map.into();
    }
}
