use std::io::Result;
use std::path::PathBuf;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::client_conn::DfsClientConn;

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
                        DfsClientConn::new(
                            // Initialize fields for DfsClientConn as needed
                            self.root_path.clone(),
                            self.next_client_id,
                            tcp_stream,
                        )
                    ));
                    self.next_client_id += 1;
                    self.clients.push(Arc::clone(&new_client));
                    let thread_client = Arc::clone(&new_client);
                    // Distribute to handle thread
                    thread::spawn({
                        move || { 
                            let mut thread_client = thread_client.lock().unwrap();
                            thread_client.handle_conn();
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {}", e);
                }
            }
        }
        
        Ok(())
    }

}