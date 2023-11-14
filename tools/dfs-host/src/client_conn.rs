use axerrno::AxError;
use axfs::distfs::request::{Action, DirEntry};
use axfs::distfs::request::{NodeAttr, Request, Response};
use axfs::distfs::BINCODE_CONFIG;
use bincode::Encode;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::prelude::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bincode::enc::write::Writer;

use crate::host::NodeID;
use crate::node_conn::DfsNodeConn;
use crate::utils::{io_err_to_axerr, unix_ty_to_axty};

pub struct DfsClientConn {
    root_path: PathBuf,
    conn: TcpStream,
    peers: Arc<Mutex<BTreeMap<NodeID, Arc<Mutex<DfsNodeConn>>>>>,
    file_index: Arc<Mutex<BTreeMap<String, NodeID>>>,
}

pub(super) struct Tcpio<'a>(pub &'a mut TcpStream);

impl Writer for Tcpio<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::result::Result<(), bincode::error::EncodeError> {
        self.0
            .write_all(bytes)
            .map_err(|e| bincode::error::EncodeError::OtherString(e.to_string()))
    }
}

impl DfsClientConn {
    pub fn new(
        root_path: PathBuf,
        conn: TcpStream,
        peers: Arc<Mutex<BTreeMap<NodeID, Arc<Mutex<DfsNodeConn>>>>>,
        file_index: Arc<Mutex<BTreeMap<String, NodeID>>>,
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
            let req = deserialize_data_from_buff(&buff, bytes_read);
            println!("Received: {:?}", req);
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

    fn handle_open(&mut self, open_path: &str) {
        let modified_str = open_path.trim_start_matches(|c| c == '/');
        let file_path = self.root_path.join(modified_str);
        match File::open(file_path) {
            Ok(_) => send_ok_to_conn(&mut self.conn, ()),
            Err(e) => send_err_to_conn(&mut self.conn, io_err_to_axerr(e)),
        };
    }

    fn handle_read(&mut self, read_path: &str, offset: u64, length: u64) {
        let modified_str = read_path.trim_start_matches(|c| c == '/');
        let file_path = self.root_path.join(modified_str);

        match File::open(file_path) {
            Ok(mut f) => {
                let mut buffer = vec![0; length as usize];
                // Seek to the specified offset
                // Read data into a buffer of the specified length
                f.seek(SeekFrom::Start(offset))
                    .map_err(|e| send_err_to_conn(&mut self.conn, io_err_to_axerr(e)))
                    .and_then(|_| {
                        let size = f
                            .read(&mut buffer)
                            .map_err(|e| send_err_to_conn(&mut self.conn, io_err_to_axerr(e))).unwrap();
                        send_ok_to_conn(&mut self.conn, size as u64);
                        self.conn.write_all(&buffer[..size]).expect(
                            &format!(
                                "Error happen when writing back read content response from connection: {:?}",
                                self.conn
                            ),
                        );
                        Ok(())
                    }).unwrap();
            }
            Err(e) => send_err_to_conn(&mut self.conn, io_err_to_axerr(e)),
        };
    }

    fn handle_write(&mut self, write_path: &str, offset: u64, content: &[u8]) {
        let modified_str = write_path.trim_start_matches('/');
        let file_path = self.root_path.join(modified_str);

        match OpenOptions::new().write(true).append(true).open(file_path) {
            Ok(mut f) => {
                // Seek to the specified offset
                // Write data into file of the specified length
                f.seek(SeekFrom::Start(offset))
                    .map_err(|e| send_err_to_conn(&mut self.conn, io_err_to_axerr(e)))
                    .and_then(|_| {
                        f.write_all(content)
                            .map_err(|e| send_err_to_conn(&mut self.conn, io_err_to_axerr(e)))
                            .unwrap();
                        send_ok_to_conn(&mut self.conn, content.len() as u64);
                        Ok(())
                    })
                    .unwrap();
            }
            Err(e) => send_err_to_conn(&mut self.conn, io_err_to_axerr(e)),
        }
    }

    fn handle_lookup(&mut self, rel_path: &str, lookup_path: &str) {
        let path = PathBuf::new()
            .join(rel_path.trim_start_matches('/'))
            .join(lookup_path.trim_start_matches('/'));
        match File::open(self.root_path.join(path)) {
            Ok(_) => {
                let res = PathBuf::new()
                    .join(rel_path.trim_start_matches('/'))
                    .join(lookup_path.trim_start_matches('/'))
                    .to_string_lossy()
                    .to_string();
                send_ok_to_conn(&mut self.conn, res);
            }
            Err(e) => send_err_to_conn(&mut self.conn, io_err_to_axerr(e)),
        };
    }

    fn handle_create(&mut self, rel_path: &str, create_path: &str) {
        match File::create(
            self.root_path
                .join(rel_path.trim_start_matches('/'))
                .join(create_path.trim_start_matches('/')),
        ) {
            Ok(_) => send_ok_to_conn(&mut self.conn, ()),
            Err(e) => send_err_to_conn(&mut self.conn, io_err_to_axerr(e)),
        }
    }

    fn handle_trunc(&mut self, rel_path: &str, size: u64) {
        let file_path = self.root_path.join(rel_path);
        match OpenOptions::new().write(true).open(file_path) {
            Ok(f) => {
                f.set_len(size)
                    .map_err(|e| send_err_to_conn(&mut self.conn, io_err_to_axerr(e)))
                    .unwrap();
                send_ok_to_conn(&mut self.conn, ());
            }
            Err(e) => send_err_to_conn(&mut self.conn, io_err_to_axerr(e)),
        }
    }

    fn handle_release(&mut self) {
        send_ok_to_conn(&mut self.conn, ());
    }

    fn handle_getattr(&mut self, rel_path: &str) {
        match File::open(self.root_path.join(rel_path.trim_start_matches('/'))) {
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
                send_ok_to_conn(&mut self.conn, attr);
            }
            Err(e) => send_err_to_conn(&mut self.conn, io_err_to_axerr(e)),
        }
    }

    fn handle_rename(&mut self, rel_path: &str, src_path: &str, dst_path: &str) {
        let src = self
            .root_path
            .join(rel_path.trim_start_matches('/'))
            .join(src_path.trim_start_matches('/'));
        let dst = self
            .root_path
            .join(rel_path.trim_start_matches('/'))
            .join(dst_path.trim_start_matches('/'));
        match fs::rename(src, dst) {
            Ok(_) => send_ok_to_conn(&mut self.conn, ()),
            Err(e) => send_err_to_conn(&mut self.conn, io_err_to_axerr(e)),
        }
    }

    fn handle_remove(&mut self, rel_path: &str, remove_path: &str) {
        let path = self
            .root_path
            .join(rel_path.trim_start_matches('/'))
            .join(remove_path.trim_start_matches('/'));
        match fs::remove_file(path) {
            Ok(_) => send_ok_to_conn(&mut self.conn, ()),
            Err(e) => send_err_to_conn(&mut self.conn, io_err_to_axerr(e)),
        }
    }

    fn handle_readdir(&mut self, rel_path: &str, start_index: u64) {
        let base_path = self.root_path.join(rel_path.trim_start_matches('/'));
        let entities = fs::read_dir(base_path).unwrap().skip(start_index as _);
        let entities_col: Vec<_> = entities.collect();
        send_ok_to_conn(&mut self.conn, entities_col.len());
        for entry in entities_col {
            match entry {
                Ok(e) => {
                    if e.path().is_dir() || e.path().is_file() {
                        bincode::serde::encode_into_writer(
                            &DirEntry {
                                ty: unix_ty_to_axty(e.file_type().unwrap()),
                                name: e.file_name().to_string_lossy().into(),
                            },
                            Tcpio(&mut self.conn),
                            BINCODE_CONFIG,
                        )
                        .expect("Error when writing back serialized entry data;");
                    } else {
                        unreachable!()
                    }
                }
                Err(e) => todo!(),
            }
        }
    }

    fn handle_getparent(&mut self, rel_path: &str) {
        let parent_path = Path::new(rel_path)
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string();
        send_ok_to_conn(&mut self.conn, parent_path);
    }

    fn handle_fsync(&mut self) {
        send_ok_to_conn(&mut self.conn, ());
    }
}

fn read_data_from_conn(buff: &mut [u8], conn: &mut TcpStream) -> usize {
    let bytes_read = conn
        .read(buff)
        .map_err(|e| {
            eprintln!("Error reading bytes from connection: {:?}", conn);
            send_err_to_conn(conn, io_err_to_axerr(e))
        })
        .unwrap();
    bytes_read
}

fn deserialize_data_from_buff<'a>(
    buff: &'a [u8],
    bytes_read: usize,
    // conn: &'a mut TcpStream,
) -> axfs::distfs::request::Request<'a, 'a> {
    let (req, _) =
        bincode::borrow_decode_from_slice::<Request, _>(&buff[..bytes_read], BINCODE_CONFIG)
            .map_err(|e| {
                eprintln!("Error deserializing from connection: {}", e);
                // send_err_to_conn(conn, io_err_to_axerr(ErrorKind::InvalidData.into()))
            })
            .unwrap();
    req
}

fn send_ok_to_conn<T: Encode>(conn: &mut TcpStream, con: T) {
    let res = Response::Ok(con);
    bincode::encode_into_writer(&res, Tcpio(conn), BINCODE_CONFIG).expect(&format!(
        "Error happen when writing back success response from connection: {:?}",
        conn
    ))
}

fn send_err_to_conn(conn: &mut TcpStream, e: AxError) {
    let res_err: std::result::Result<String, i32> = Response::Err(e.code());
    bincode::encode_into_writer(&res_err, Tcpio(conn), BINCODE_CONFIG).expect(&format!(
        "Error happen when writing back error response from connection: {:?}",
        conn
    ));
}
