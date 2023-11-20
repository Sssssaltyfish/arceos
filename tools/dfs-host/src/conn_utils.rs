use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::TcpStream;

#[cfg(not(feature = "axstd"))]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use alloc::sync::Arc;
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use axerrno::AxError;
use axfs::distfs::request::{DirEntry, NodeAttr, Request, Response};
use axfs::distfs::BINCODE_CONFIG;

use bincode::enc::write::Writer;
use bincode::Encode;
use dashmap::DashMap;

use crate::host::NodeID;
use crate::utils::{unix_ty_to_axty, Path, PathBuf};

use crate::queue_request::{self, MessageQueue, PeerAction, ReturnTypeYouNeed};
#[cfg(feature = "axstd")]
use crate::utils::*;

use crate::utils::io_err_to_axerr;

pub(super) struct Tcpio<'a>(pub &'a mut TcpStream);

impl Writer for Tcpio<'_> {
    fn write(&mut self, bytes: &[u8]) -> core::result::Result<(), bincode::error::EncodeError> {
        self.0
            .write_all(bytes)
            .map_err(|e| bincode::error::EncodeError::OtherString(e.to_string()))
    }
}

pub fn read_data_from_conn(buff: &mut [u8], conn: &mut TcpStream) -> usize {
    let bytes_read = conn
        .read(buff)
        .map_err(|e| {
            logger::error!("Error reading bytes from connection: {:?}", conn);
            send_err_to_conn(conn, io_err_to_axerr(e))
        })
        .unwrap();
    bytes_read
}

pub fn deserialize_client_request_from_buff<'a>(
    buff: &'a [u8],
    bytes_read: usize,
) -> axfs::distfs::request::Request<'a, 'a> {
    let (req, _) =
        bincode::borrow_decode_from_slice::<Request, _>(&buff[..bytes_read], BINCODE_CONFIG)
            .map_err(|e| {
                logger::error!("Error deserializing client request from connection: {}", e);
            })
            .unwrap();
    req
}

pub fn deserialize_node_request_from_buff(
    buff: &[u8],
    bytes_read: usize,
) -> queue_request::PeerAction {
    let (req, _) = bincode::serde::decode_from_slice::<queue_request::PeerAction, _>(
        &buff[..bytes_read],
        BINCODE_CONFIG,
    )
    .map_err(|e| {
        logger::error!("Error deserializing peer action from connection: {}", e);
    })
    .unwrap();
    req
}

pub fn deserialize_response_from_buff(buff: &[u8], bytes_read: usize) -> ReturnTypeYouNeed {
    let (req, _) = bincode::serde::decode_from_slice::<ReturnTypeYouNeed, _>(
        &buff[..bytes_read],
        BINCODE_CONFIG,
    )
    .map_err(|e| {
        logger::error!("Error deserializing peer response from connection: {}", e);
    })
    .unwrap();
    req
}

pub fn send_ok_to_conn<T: Encode>(conn: &mut TcpStream, con: T) {
    let res = Response::Ok(con);
    bincode::encode_into_writer(&res, Tcpio(conn), BINCODE_CONFIG).expect(&format!(
        "Error happen when writing back success response from connection: {:?}",
        conn
    ))
}

pub fn send_err_to_conn(conn: &mut TcpStream, e: AxError) {
    let res_err: Response<String> = Response::Err(e.code());
    bincode::encode_into_writer(&res_err, Tcpio(conn), BINCODE_CONFIG).expect(&format!(
        "Error happen when writing back error response from connection: {:?}",
        conn
    ));
}

pub fn send_serialized_data_to_conn(conn: &mut TcpStream, res: ReturnTypeYouNeed) {
    bincode::encode_into_writer(&res, Tcpio(conn), BINCODE_CONFIG).expect(&format!(
        "Error happen when writing back serialized response from connection: {:?}",
        conn
    ));
}

pub trait DfsServer {
    fn handle_open(&mut self, open_path: &str) {
        let modified_str = open_path.trim_start_matches(|c| c == '/');
        let file_path = self.get_root_path().join(modified_str);
        match File::open(&file_path) {
            Ok(_) => send_ok_to_conn(self.get_tcp_stream(), ()),
            Err(e) => send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)),
        };
    }

    fn handle_read(&mut self, read_path: &str, offset: u64, length: u64) {
        let modified_str = read_path.trim_start_matches(|c| c == '/');
        let file_path = self.get_root_path().join(modified_str);

        match File::open(file_path) {
            Ok(mut f) => {
                let mut buffer = vec![0; length as usize];
                // Seek to the specified offset
                // Read data into a buffer of the specified length
                f.seek(SeekFrom::Start(offset))
                    .map_err(|e| send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)))
                    .and_then(|_| {
                        let size = f
                            .read(&mut buffer)
                            .map_err(|e| send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e))).unwrap();
                        send_ok_to_conn(self.get_tcp_stream(), size as u64);
                        self.get_tcp_stream().write_all(&buffer[..size]).expect(
                            &format!(
                                "Error happen when writing back read content response from connection: {:?}",
                                self.get_tcp_stream()
                            ),
                        );
                        Ok(())
                    }).unwrap();
            }
            Err(e) => send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)),
        };
    }

    fn handle_write(&mut self, write_path: &str, offset: u64, content: &[u8]) {
        let modified_str = write_path.trim_start_matches('/');
        let file_path = self.get_root_path().join(modified_str);

        match OpenOptions::new().write(true).append(true).open(&file_path) {
            Ok(mut f) => {
                // Seek to the specified offset
                // Write data into file of the specified length
                f.seek(SeekFrom::Start(offset))
                    .map_err(|e| send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)))
                    .and_then(|_| {
                        f.write_all(content)
                            .map_err(|e| {
                                send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e))
                            })
                            .unwrap();
                        send_ok_to_conn(self.get_tcp_stream(), content.len() as u64);
                        Ok(())
                    })
                    .unwrap();
            }
            Err(e) => send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)),
        }
    }

    fn handle_lookup(&mut self, rel_path: &str, lookup_path: &str) {
        let path = PathBuf::new()
            .join(rel_path.trim_start_matches('/'))
            .join(lookup_path.trim_start_matches('/'));
        match File::open(self.get_root_path().join(path)) {
            Ok(_) => {
                let res = PathBuf::new()
                    .join(rel_path.trim_start_matches('/'))
                    .join(lookup_path.trim_start_matches('/'))
                    .to_string_lossy()
                    .to_string();
                send_ok_to_conn(self.get_tcp_stream(), res);
            }
            Err(e) => send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)),
        };
    }

    fn handle_create(&mut self, rel_path: &str, create_path: &str) {
        match File::create(
            self.get_root_path()
                .join(rel_path.trim_start_matches('/'))
                .join(create_path.trim_start_matches('/')),
        ) {
            Ok(_) => send_ok_to_conn(self.get_tcp_stream(), ()),
            Err(e) => send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)),
        }
    }

    fn handle_trunc(&mut self, rel_path: &str, size: u64) {
        let file_path = self.get_root_path().join(rel_path);
        match OpenOptions::new().write(true).open(file_path) {
            Ok(f) => {
                f.set_len(size)
                    .map_err(|e| send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)))
                    .unwrap();
                send_ok_to_conn(self.get_tcp_stream(), ());
            }
            Err(e) => send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)),
        }
    }

    fn handle_release(&mut self) {
        send_ok_to_conn(self.get_tcp_stream(), ());
    }

    fn handle_getattr(&mut self, rel_path: &str) {
        match File::open(self.get_root_path().join(rel_path.trim_start_matches('/'))) {
            Ok(f) => {
                let meta = f.metadata().unwrap();
                let mode = meta.permissions().mode() as u16 & 0o777;
                let ty = meta.file_type();
                let ty = unix_ty_to_axty(ty);
                let size = meta.len();
                let blocks = meta.blocks();
                let attr = NodeAttr {
                    mode,
                    ty,
                    size,
                    blocks,
                };
                send_ok_to_conn(self.get_tcp_stream(), attr);
            }
            Err(e) => send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)),
        }
    }

    fn handle_rename(&mut self, rel_path: &str, src_path: &str, dst_path: &str) {
        let src = self
            .get_root_path()
            .join(rel_path.trim_start_matches('/'))
            .join(src_path.trim_start_matches('/'));
        let dst = self
            .get_root_path()
            .join(rel_path.trim_start_matches('/'))
            .join(dst_path.trim_start_matches('/'));
        match fs::rename(src, dst) {
            Ok(_) => send_ok_to_conn(self.get_tcp_stream(), ()),
            Err(e) => send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)),
        }
    }

    fn handle_remove(&mut self, rel_path: &str, remove_path: &str) {
        let path = self
            .get_root_path()
            .join(rel_path.trim_start_matches('/'))
            .join(remove_path.trim_start_matches('/'));
        match fs::remove_file(path) {
            Ok(_) => send_ok_to_conn(self.get_tcp_stream(), ()),
            Err(e) => send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)),
        }
    }

    fn handle_readdir(&mut self, rel_path: &str, start_index: u64) {
        let base_path = self.get_root_path().join(rel_path.trim_start_matches('/'));
        let entities = fs::read_dir(&base_path).unwrap().skip(start_index as _);
        let entities_col: Vec<_> = entities.collect();
        send_ok_to_conn(self.get_tcp_stream(), entities_col.len());
        for entry in entities_col {
            match entry {
                Ok(e) => {
                    if e.path().is_dir() || e.path().is_file() {
                        bincode::serde::encode_into_writer(
                            &DirEntry {
                                ty: unix_ty_to_axty(e.file_type().unwrap()),
                                name: e.file_name().to_string_lossy().into(),
                            },
                            Tcpio(self.get_tcp_stream()),
                            BINCODE_CONFIG,
                        )
                        .expect("Error when writing back serialized entry data;");
                    } else {
                        unreachable!()
                    }
                }
                Err(e) => send_err_to_conn(self.get_tcp_stream(), io_err_to_axerr(e)),
            }
        }
    }

    fn handle_getparent(&mut self, rel_path: &str) {
        let parent_path = Path::new(rel_path)
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string();
        send_ok_to_conn(self.get_tcp_stream(), parent_path);
    }

    fn handle_fsync(&mut self) {
        send_ok_to_conn(self.get_tcp_stream(), ());
    }

    fn handle_insert_index(&self, rel_path: &str, create_path: &str) {
        let rel_path_str = PathBuf::new()
            .join(rel_path.trim_start_matches('/'))
            .join(create_path.trim_start_matches('/'));
        let rel_path = rel_path_str.to_string_lossy().to_string();
        let create_index = DashMap::new();
        create_index.insert(rel_path.clone(), self.get_node_id());
        (&self.get_file_index()).insert(rel_path, self.get_node_id());
        for peer in self.get_peers().iter() {
            let p = peer.value();
            p.submit_and_wait(PeerAction::InsertIndex(create_index.clone()))
                .expect(&format!(
                    "Error happen when inserting index in peer {:?}.",
                    peer.key()
                ));
        }
    }

    fn handle_remove_index(&self, rel_path: &str, remove_path: &str) {
        let rel_path_str = PathBuf::new()
            .join(rel_path.trim_start_matches('/'))
            .join(remove_path.trim_start_matches('/'));
        let rel_path = rel_path_str.to_string_lossy().to_string();
        let mut remove_index = Vec::new();
        remove_index.push(rel_path.clone());
        (&self.get_file_index()).remove(&rel_path);
        for peer in self.get_peers().iter() {
            let p = peer.value();
            p.submit_and_wait(PeerAction::RemoveIndex(remove_index.clone()))
                .expect(&format!(
                    "Error happen when updating index in peer {:?}.",
                    peer.key()
                ));
        }
    }

    fn handle_update_index(&self, rel_path: &str, src_path: &str, dst_path: &str) {
        let src_path_str = PathBuf::new()
            .join(rel_path.trim_start_matches('/'))
            .join(src_path.trim_start_matches('/'));
        let dst_path_str = PathBuf::new()
            .join(rel_path.trim_start_matches('/'))
            .join(dst_path.trim_start_matches('/'));
        let src_file_path = src_path_str.to_string_lossy().to_string();
        let dst_file_path = dst_path_str.to_string_lossy().to_string();
        let update_index = DashMap::new();
        update_index.insert(src_file_path.clone(), dst_file_path.clone());
        (&self.get_file_index()).remove(&src_file_path);
        (&self.get_file_index()).insert(dst_file_path, self.get_node_id());
        for peer in self.get_peers().iter() {
            let p = peer.value();
            p.submit_and_wait(PeerAction::UpdateIndex(update_index.clone()))
                .expect(&format!(
                    "Error happen when updating index in peer {:?}.",
                    peer.key()
                ));
        }
    }

    fn get_node_id(&self) -> NodeID;

    fn get_peers(&self) -> Arc<DashMap<NodeID, Arc<MessageQueue>>>;

    fn get_file_index(&self) -> Arc<DashMap<String, NodeID>>;

    fn get_tcp_stream(&mut self) -> &mut TcpStream;

    fn get_root_path(&self) -> PathBuf;
}
