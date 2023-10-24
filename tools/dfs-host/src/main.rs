use host::DfsHost;
use std::env;

mod host;
mod client_conn;
mod utils;

fn main() {
    env_logger::builder().filter_level(logger::LevelFilter::Debug).init();
    let mut host = DfsHost::new(env::current_dir().unwrap());
    let _ = host.start_listening("127.0.0.1:8000");
}
