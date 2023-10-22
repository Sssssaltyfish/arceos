use std::io::{Read, Result, Write};
use std::path::PathBuf;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use axfs::distfs::request::Create;

struct DfsClientConn {
    conn_id: u32,
    conn: TcpStream,
    buff: Vec<u8>,
}

impl DfsClientConn {
    fn handle_conn(&mut self) -> usize {
        let mut total_bytes_read: usize = 0;
        let mut buffer = [0; 1024];
        self.conn.read_to_end(buf)
        // Continuously read data from the TcpStream and store it in the buff.
        loop {
            match self.conn.read(&mut buffer) {
                Ok(bytes_read) => {
                    if bytes_read == 0 {
                        // No more data to read. Connection closed.
                        break;
                    }
                    // Extend the buff with the data read from the TcpStream.
                    self.buff.extend_from_slice(&buffer[..bytes_read]);
                    total_bytes_read += bytes_read;
                    
                    // Print the received data.
                    let received_data = &buffer[..bytes_read];
                    match std::str::from_utf8(received_data) {
                        Ok(decoded_str) => {
                            println!("Decoded string: {}", decoded_str);
                        }
                        Err(_) => {
                            println!("Unable to decode the received data as a valid string.");
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error reading from connection: {}", err);
                    break;
                }
            }
        }

        // You can print or process the received data if needed.
        println!("Received {} bytes of data", total_bytes_read);

        // You can also write data back to the connection if required.
        // For example:
        // self.conn.write_all(b"Response data").expect("Failed to write data");

        // Return the total number of bytes read.
        total_bytes_read
    }
}

pub struct DfsHost {
    next_client_id: u32,
    root_path: PathBuf,
    clients: Vec<Arc<Mutex<DfsClientConn>>>,
}

impl DfsHost {
    pub fn new(root_path: PathBuf) -> Self {
        DfsHost {
            next_client_id: 1,
            root_path,
            clients: Vec::new(),
        }
    }

    pub fn start_listening(&mut self, bind_address: &str) -> Result<()> {
        // Bind a TcpListener to listen on bind_address
        let listener = TcpListener::bind(bind_address).unwrap();
        
        println!("Listening for incoming connections on {bind_address}...");

        for stream in listener.incoming() {
            match stream {
                Ok(tcp_stream) => {
                    println!("Accepted a new connection from: {:?}", tcp_stream.peer_addr());
                    // Create a new DfsClientConn instance for each connection and store it
                    let new_client = Arc::new(Mutex::new(
                        DfsClientConn {
                            // Initialize fields for DfsClientConn as needed
                            conn_id: self.next_client_id,
                            conn: tcp_stream,
                            buff: vec![0u8; 1024],
                        }
                    ));
                    self.next_client_id += 1;
                    self.clients.push(Arc::clone(&new_client));
                    let thread_client = Arc::clone(&new_client);
                    // Distribute to handle thread
                    let handle_thread = thread::spawn({
                        move || { 
                            let mut thread_client = thread_client.lock().unwrap();
                            thread_client.handle_conn();
                        }
                    });
                    handle_thread.join().expect(&format!("Thread for handling client failed."));
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {}", e);
                }
            }
        }
        
        Ok(())
    }

}