use axerrno::{ax_err_type, AxResult};
use axfs::distfs::request::Action::{
    self, Create, Lookup, Open as ActionOpen, Read as ActionRead, Write as ActionWrite,
};
use axfs::distfs::request::{NodeAttr, Request, Response};
use axfs::distfs::BINCODE_CONFIG;
use std::fs::{File, OpenOptions};
use std::io::{Read, Result, Seek, SeekFrom, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::prelude::MetadataExt;
use std::path::{Path, PathBuf};

use bincode::enc::write::Writer;

use crate::utils::{io_err_to_axerr, unix_ty_to_axty};

pub struct DfsClientConn {
    root_path: PathBuf,
    conn_id: u32,
    conn: TcpStream,
    buff: Vec<u8>,
    file_ptr: Option<File>,
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
    pub fn new(root_path: PathBuf, conn_id: u32, conn: TcpStream) -> Self {
        DfsClientConn {
            root_path,
            conn_id,
            conn,
            buff: vec![0u8; 1024],
            file_ptr: Option::None,
        }
    }

    pub fn handle_conn(&mut self) -> Result<()> {
        // mount 127.0.0.1:8000 /dist distfs
        // Continuously read data from the TcpStream and store it in the buff.
        loop {
            let read_res = self.read_data_from_conn();
            match read_res {
                Ok(bytes_read) => {
                    if bytes_read == 0 {
                        // No more data to read. Connection closed.
                        // Todo: add close connection here.
                        break;
                    }
                    let decode_data = self.deserialize_data_from_buff(bytes_read);
                    println!("Received: {:?}", decode_data);
                    match decode_data {
                        Ok((req, size)) => match req.action {
                            ActionOpen => {
                                let open_result = self.handle_open(req.relpath);
                                match open_result {
                                    Ok(f) => {
                                        self.file_ptr = Some(f);
                                        let res = Response::Ok(());
                                        bincode::encode_into_writer(
                                            &res,
                                            Tcpio(&mut self.conn),
                                            BINCODE_CONFIG,
                                        )
                                        .unwrap();
                                    }
                                    Err(err) => logger::error!(
                                        "Error when excuting open action: {}, relpath: {}",
                                        err,
                                        req.relpath
                                    ),
                                }
                            }
                            ActionRead(read_action) => {
                                let read_result = self.handle_read(
                                    req.relpath,
                                    read_action.offset,
                                    read_action.length,
                                );
                                match read_result {
                                    Ok((size, content)) => {
                                        let res = Response::Ok(size as u64);
                                        bincode::encode_into_writer(
                                            &res,
                                            Tcpio(&mut self.conn),
                                            BINCODE_CONFIG,
                                        )
                                        .unwrap();
                                        self.conn.write_all(&content[..size]).unwrap();
                                    }
                                    Err(err) => {
                                        eprintln!("Error when excuting read action: {}", err)
                                    }
                                }
                            }
                            ActionWrite(write_action) => {
                                let write_result = self.handle_write(
                                    req.relpath,
                                    write_action.offset,
                                    write_action.content,
                                );
                                match write_result {
                                    Ok(write_len) => {
                                        let res = Response::Ok(write_len as u64);
                                        bincode::encode_into_writer(
                                            &res,
                                            Tcpio(&mut self.conn),
                                            BINCODE_CONFIG,
                                        )
                                        .unwrap();
                                    }
                                    Err(err) => {
                                        eprintln!("Error when excuting write action: {}", err)
                                    }
                                }
                            }
                            Lookup(lookup) => {
                                let res = match File::open(
                                    self.root_path.join(req.relpath).join(lookup.path),
                                ) {
                                    Ok(_) => {
                                        let path = Path::new(req.relpath).join(lookup.path);
                                        Response::Ok(path.to_string_lossy().to_string())
                                    }
                                    Err(e) => Response::Err(io_err_to_axerr(e).code()),
                                };
                                bincode::encode_into_writer(
                                    &res,
                                    Tcpio(&mut self.conn),
                                    BINCODE_CONFIG,
                                )
                                .unwrap();
                            }
                            Create(create) => {
                                let _ = File::create(
                                    self.root_path.join(req.relpath).join(create.path),
                                )?;
                                let res = Response::Ok(());
                                bincode::encode_into_writer(
                                    &res,
                                    Tcpio(&mut self.conn),
                                    BINCODE_CONFIG,
                                )
                                .unwrap();
                            }
                            Action::Trunc(trunc) => {
                                let file_path = self.root_path.join(req.relpath);
                                let f = OpenOptions::new().write(true).open(file_path).unwrap();
                                f.set_len(trunc.size).unwrap();
                                let res = Response::Ok(());
                                bincode::encode_into_writer(
                                    &res,
                                    Tcpio(&mut self.conn),
                                    BINCODE_CONFIG,
                                )
                                .unwrap();
                            }
                            Action::GetAttr => match File::open(self.root_path.join(req.relpath)) {
                                Ok(file) => {
                                    let meta = file.metadata().unwrap();
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
                                    bincode::encode_into_writer(
                                        Response::Ok(attr),
                                        Tcpio(&mut self.conn),
                                        BINCODE_CONFIG,
                                    )
                                    .unwrap();
                                }
                                Err(e) => {
                                    let axerr: Response<()> =
                                        Response::Err(io_err_to_axerr(e).code());
                                    bincode::encode_into_writer(
                                        axerr,
                                        Tcpio(&mut self.conn),
                                        BINCODE_CONFIG,
                                    )
                                    .unwrap();
                                }
                            },
                            Action::Release => {
                                bincode::encode_into_writer(
                                    Response::Ok(()),
                                    Tcpio(&mut self.conn),
                                    BINCODE_CONFIG,
                                )
                                .unwrap();
                            }
                            _ => todo!(),
                        },
                        Err(err) => {
                            eprintln!("Error when decoding from data from connection: {}", err);
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error reading from connection: {}", err);
                    break;
                }
            }
        }

        // You can also write data back to the connection if required.
        // For example:
        // self.conn.write_all(b"Response data").expect("Failed to write data");

        // Return the total number of bytes read.
        Ok(())
    }

    fn read_data_from_conn(&mut self) -> Result<usize> {
        self.conn.read(&mut self.buff)
    }

    fn deserialize_data_from_buff(
        &self,
        bytes_read: usize,
    ) -> AxResult<(axfs::distfs::request::Request<'_, '_>, usize)> {
        let ret = bincode::borrow_decode_from_slice::<Request, _>(
            &self.buff[..bytes_read],
            BINCODE_CONFIG,
        )
        .map_err(|e| ax_err_type!(Io, e))?;
        Ok(ret)
    }

    fn handle_open(&self, open_path: &str) -> Result<File> {
        let modified_str = open_path.trim_start_matches(|c| c == '/');
        let file_path = self.root_path.join(modified_str);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&file_path)?;
        println!(
            "Successfully opened file: {}",
            file_path.as_os_str().to_string_lossy()
        );
        Ok(file)
    }

    fn handle_read(&self, read_path: &str, offset: u64, length: u64) -> Result<(usize, Vec<u8>)> {
        let modified_str = read_path.trim_start_matches(|c| c == '/');
        let file_path = self.root_path.join(modified_str);

        let mut file = File::open(file_path)?;

        // Seek to the specified offset
        file.seek(SeekFrom::Start(offset))?;

        // Read data into a buffer of the specified length
        let mut buffer = vec![0; length as usize];
        let size = file.read(&mut buffer)?;

        Ok((size, buffer))
    }

    fn handle_write(&self, write_path: &str, offset: u64, content: &[u8]) -> Result<usize> {
        let modified_str = write_path.trim_start_matches('/');
        let file_path = self.root_path.join(modified_str);

        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(file_path)?;

        println!("Open file succeed!");

        // Seek to the specified offset
        file.seek(SeekFrom::Start(offset))?;

        // Write the provided content at the specified offset
        file.write_all(content)?;

        println!("Write file succeed!");

        Ok(content.len())
    }
}
