use env_logger;
use log::{debug, error, info, warn};
use quiche::{Connection, ConnectionId};
use std::{
    env,
    net::{SocketAddr, UdpSocket},
};
mod common;
use common::server_fun::run_server;
#[test]
fn test_connect() {
    common::log_init();
    run_server();
}
