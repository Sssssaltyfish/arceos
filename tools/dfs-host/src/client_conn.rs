use std::io::{Write};
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

use crate::utils::*;

use crate::conn_utils::*;
use crate::host::NodeID;
use crate::queue_request::{MessageQueue, PeerAction, ReturnTypeYouNeed};
use crate::utils::PathBuf;

pub struct DfsClientConn {
    node_id: NodeID,
    root_path: PathBuf,
    conn: TcpStream,
    peers: Arc<DashMap<NodeID, Arc<MessageQueue>>>,
    file_index: Arc<DashMap<String, NodeID>>,
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
        node_id: NodeID,
        root_path: PathBuf,
        conn: TcpStream,
        peers: Arc<DashMap<NodeID, Arc<MessageQueue>>>,
        file_index: Arc<DashMap<String, NodeID>>,
    ) -> Self {
        DfsClientConn {
            node_id,
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
                Action::Open => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(_) => send_ok_to_conn(self.get_tcp_stream(), ()),
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                }
                Action::Release => self.handle_release(),
                Action::GetAttr => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(node_id) => {
                            if node_id == self.node_id {
                                self.handle_getattr(req.relpath)
                            } else {
                                let res = self.switch_to_peer(&node_id, PeerAction::SerializedAction(buff[..bytes_read].to_vec()));
                                send_serialized_data_to_conn(self.get_tcp_stream(), res);
                            }
                        }
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                },
                Action::Read(read) => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(node_id) => {
                            if node_id == self.node_id {
                                self.handle_read(file_path_str, read.offset, read.length);
                            } else {
                                let res = self.switch_to_peer(&node_id, PeerAction::SerializedAction(buff[..bytes_read].to_vec()));
                                send_serialized_data_to_conn(self.get_tcp_stream(), res);
                            }
                        }
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                }
                Action::Write(write) => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(node_id) => {
                            if node_id == self.node_id {
                                self.handle_write(file_path_str, write.offset, write.content);
                            } else {
                                let res = self.switch_to_peer(&node_id, PeerAction::SerializedAction(buff[..bytes_read].to_vec()));
                                send_serialized_data_to_conn(self.get_tcp_stream(), res);
                            }
                        }
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                }
                Action::Fsync => self.handle_fsync(),
                Action::Trunc(trunc) => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(node_id) => {
                            if node_id == self.node_id {
                                self.handle_trunc(req.relpath, trunc.size)
                            } else {
                                let res = self.switch_to_peer(&node_id, PeerAction::SerializedAction(buff[..bytes_read].to_vec()));
                                send_serialized_data_to_conn(self.get_tcp_stream(), res);
                            }
                        }
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                }
                Action::GetParent => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(node_id) => {
                            if node_id == self.node_id {
                                self.handle_getparent(file_path_str);
                            } else {
                                let res = self.switch_to_peer(&node_id, PeerAction::SerializedAction(buff[..bytes_read].to_vec()));
                                send_serialized_data_to_conn(self.get_tcp_stream(), res);
                            }
                        }
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                },
                Action::Lookup(lookup) => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(node_id) => {
                            if node_id == self.node_id {
                                self.handle_lookup(req.relpath, lookup.path);
                            } else {
                                let res = self.switch_to_peer(&node_id, PeerAction::SerializedAction(buff[..bytes_read].to_vec()));
                                send_serialized_data_to_conn(self.get_tcp_stream(), res);
                            }
                        }
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                }
                Action::Create(create) => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(node_id) => {
                            if node_id == self.node_id {
                                self.handle_create(req.relpath, create.path);
                            } else {
                                let res = self.switch_to_peer(&node_id, PeerAction::SerializedAction(buff[..bytes_read].to_vec()));
                                send_serialized_data_to_conn(self.get_tcp_stream(), res);
                            }
                        }
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                }
                Action::Remove(remove) => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(node_id) => {
                            if node_id == self.node_id {
                                self.handle_remove(req.relpath, remove.path);
                            } else {
                                let res = self.switch_to_peer(&node_id, PeerAction::SerializedAction(buff[..bytes_read].to_vec()));
                                send_serialized_data_to_conn(self.get_tcp_stream(), res);
                            }
                        }
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                }
                Action::ReadDir(readdir) => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(node_id) => {
                            if node_id == self.node_id {
                                self.handle_readdir(req.relpath, readdir.start_idx);
                            } else {
                                let res = self.switch_to_peer(&node_id, PeerAction::SerializedAction(buff[..bytes_read].to_vec()));
                                send_serialized_data_to_conn(self.get_tcp_stream(), res);
                            }
                        }
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                }
                Action::Rename(rename) => {
                    let file_path_str = req.relpath.trim_start_matches(|c| c == '/');
                    let file_node = self.get_node_of_file(file_path_str);
                    match file_node {
                        Some(node_id) => {
                            if node_id == self.node_id {
                                self.handle_rename(req.relpath, rename.src_path, rename.dst_path)
                            } else {
                                let res = self.switch_to_peer(&node_id, PeerAction::SerializedAction(buff[..bytes_read].to_vec()));
                                send_serialized_data_to_conn(self.get_tcp_stream(), res);
                            }
                        }
                        None => send_err_to_conn(
                            self.get_tcp_stream(),
                            io_err_to_axerr(io::ErrorKind::NotFound.into()),
                        ),
                    }
                }
            }
        }
        // You can also write data back to the connection if required.
        // For example:
        // self.conn.write_all(b"Response data").expect("Failed to write data");

        // Return the total number of bytes read.
    }

    fn switch_to_peer(&mut self, node_id: &NodeID, action: PeerAction) -> ReturnTypeYouNeed {
        let peer_worker = self
            .peers
            .get(node_id)
            .expect("Unable to get peer worker in client thread.");
        peer_worker.submit_and_wait(action)
    }

    fn get_node_of_file(&self, file_path: &str) -> Option<NodeID> {
        self.file_index.get(file_path).map(|ref_id| *ref_id)
    }
}
