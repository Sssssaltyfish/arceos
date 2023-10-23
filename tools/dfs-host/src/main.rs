use host::DfsHost;
use std::env;

mod host;
mod client_conn;

fn main() {
    let mut host = DfsHost::new(env::current_dir().unwrap());
    let _ = host.start_listening("127.0.0.1:8000");
}
