extern crate alloc;

use host::DfsHost;
use std::env;

mod client_conn;
mod host;
mod node_conn;
mod utils;

fn main() {
    env_logger::builder()
        .filter_level(logger::LevelFilter::Debug)
        .init();
    let args: Vec<String> = env::args().collect();
    let node_id: u32 = args[1].parse().unwrap();
    let mut host = DfsHost::new(node_id, env::current_dir().unwrap());
    let _ = host.start_listening();
}
