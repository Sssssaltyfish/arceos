use axerrno::{ax_err_type, AxResult};
use axfs::distfs::request::{Request, Response};
use axfs::distfs::request::Action::{Open as ActionOpen, Read as ActionRead, Write as ActionWrite};
use axfs::distfs::BINCODE_CONFIG;
use std::borrow::BorrowMut;
use std::io::{Read, Result, Write, SeekFrom, Seek};
use std::path::{PathBuf, Path};
use std::net::TcpStream;
use std::fs::{File, OpenOptions};

pub struct DfsClientConn {
    root_path: PathBuf, 
    conn_id: u32,
    conn: TcpStream,
    buff: Vec<u8>,
    file_ptr: Option<File>,
}

impl DfsClientConn {

    pub fn new(root_path: PathBuf, conn_id: u32, conn: TcpStream) -> Self {
        DfsClientConn {
            root_path,
            conn_id,
            conn,
            buff: vec![0u8; 1024],
            file_ptr: Option::None
        }
    }

    pub fn handle_conn(&mut self) -> Result<()> {
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
                    let decode_data = self.deserialize_data_from_buff();
                    match decode_data {
                        Ok((req, size)) => {
                            match req.action {
                                ActionOpen => {
                                    let open_result = self.handle_open(req.relpath);
                                    match open_result {
                                        Ok(f) => self.file_ptr = Some(f),
                                        Err(err) => eprintln!("Error when excuting open action: {}", err)
                                    }
                                }
                                ActionRead(read_action) => {
                                    let read_result = self.handle_read(req.relpath, read_action.offset, read_action.length);
                                    match read_result {
                                        Ok(content) => {
                                            todo!()
                                        }
                                        Err(err) => eprintln!("Error when excuting read action: {}", err)
                                    }
                                }
                                ActionWrite(write_action) => {
                                    let write_result = self.handle_write(req.relpath, write_action.offset, write_action.content);
                                    match write_result {
                                        Ok(_) => {
                                            todo!()
                                        }
                                        Err(err) => eprintln!("Error when excuting write action: {}", err)
                                    }
                                }
                                _ => todo!()
                            }
                        }
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
        self.conn.read_to_end(self.buff.borrow_mut())
    }

    fn deserialize_data_from_buff(&self) -> AxResult<(axfs::distfs::request::Request<'_, '_>, usize)> {
        let ret = bincode::borrow_decode_from_slice::<Request, _>(&self.buff, BINCODE_CONFIG)
            .map_err(|e| ax_err_type!(Io, e))?;
        Ok(ret)
    }

    fn handle_open(&self, open_path: &str) -> Result<File> {
        let modified_str = open_path.trim_start_matches(|c| c == '/');
        let file_path = self.root_path.join(modified_str);

        let file = OpenOptions::new().create(true).write(true).open(file_path)?;
        Ok(file)
    }

    fn handle_read(&self, read_path: &str, offset: u64, length: u64) -> Result<Vec<u8>> {
        let modified_str = read_path.trim_start_matches(|c| c == '/');
        let file_path = self.root_path.join(modified_str);

        let mut file = File::open(file_path)?;

        // Seek to the specified offset
        file.seek(SeekFrom::Start(offset))?;

        // Read data into a buffer of the specified length
        let mut buffer = vec![0; length as usize];
        file.read_exact(&mut buffer)?;

        Ok(buffer)
    }

    fn handle_write(&self, write_path: &str, offset: u64, content: &[u8]) -> Result<()> {
        let modified_str = write_path.trim_start_matches(|c| c == '/');
        let file_path = self.root_path.join(modified_str);

        let mut file = OpenOptions::new().write(true).append(true).open(file_path)?;

        // Seek to the specified offset
        file.seek(SeekFrom::Start(offset))?;

        // Write the provided content at the specified offset
        file.write_all(content)?;

        Ok(())
    }

}
