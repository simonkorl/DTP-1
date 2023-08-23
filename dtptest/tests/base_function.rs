use env_logger;
use log::{debug, error, info, warn};
use quiche::{Connection, ConnectionId};
use std::{
    env,
    net::{SocketAddr, UdpSocket},
    thread,
};
mod common;
use common::{client_fun::run_client, server_fun::run_server};
// 测试使用及其注意事项
//test_connect()是用来测试传输数据的
//正如看到的，使用线程来启动服务端，然后客户端连接
//需要注意的是：可以通过环境变量来控制输出信息的格式
//例如：RUST_LOG=debug LOG_PURE=true cargo test -- --nocapture
//LOG_PURE 会生成一个纯净的日志，不包含stream_id，信息由sever端产生，每个连接的传输数据会被生成到result目录下
#[test]
fn test_connect() {
    common::log_init();
    let handle = thread::spawn(|| {
        run_server("127.0.0.1:4433");
    });
    run_client("127.0.0.1:4433", "tests/5m.txt");
    handle.join().unwrap();
    println!("END TEST");
}
#[test]
#[ignore = "the test for read a big file,and print it"]
fn test_readfile() {
    use std::io::prelude::*;
    let file = std::fs::File::open("tests/1G.txt").unwrap();
    let reader = std::io::BufReader::new(file);
    for line in reader.lines() {
        let line = line.unwrap();
        println!("{}", line);
    }
}
