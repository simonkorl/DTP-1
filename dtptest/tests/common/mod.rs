use log::{info, warn};
use std::net::SocketAddr;
pub mod client_fun;
pub mod server_fun;
pub fn log_init() {
    env_logger::builder().is_test(true).init();
}

pub fn log_socket_addr_type(addr: &std::net::SocketAddr) {
    match addr {
        SocketAddr::V4(addr) => {
            info!("will use v4 addr:{}", addr);
        },
        SocketAddr::V6(addr) => {
            info!("will use v6 addr:{}", addr);
        },
    }
}
