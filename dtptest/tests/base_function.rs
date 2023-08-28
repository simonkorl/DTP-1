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
// 通过diff命令比对产生的文件和生成的文件，可以看到是否有数据丢失
#[test]
fn test_transport_file() {
    common::log_init();
    env::set_var("LOG_PURE", "true");
    env::set_var("IS_STREAM_SEND", "false");
    env::set_var("IS_TEST_ORDER", "false");
    let handle = thread::spawn(|| {
        run_server("127.0.0.1:4433");
    });
    run_client("127.0.0.1:4433", "tests/1m.txt");
    handle.join().unwrap();
    println!("END TEST");
}
#[test]
// check the order of transport
// when server exit,need to check the result file.
//if the file's content is "World Hello",it means the order is ok.
//The test set "Hello" priority is 200,and "World" priority is 100
fn test_transport_order() {
    env::set_var("RUST_LOG", "Debug");
    common::log_init();

    env::set_var("LOG_PURE", "false");
    env::set_var("IS_STREAM_SEND", "false");
    env::set_var("IS_TEST_ORDER", "true");
    let handle = thread::spawn(|| {
        run_server("127.0.0.1:4433");
    });
    //don't need care filt_path ,it will be ignored
    run_client("127.0.0.1:4433", "tests/1G.txt");
    handle.join().unwrap();
    println!("END TEST");
}
#[test]
fn test_env_set() {
    env::set_var("RUST_ENV_TEST", "RUT_ENV_TEST");
    let handle = thread::spawn(|| {
        let env = env::var("RUST_ENV_TEST").unwrap();
        assert!(env == "RUT_ENV_TEST");
    });
    handle.join().unwrap();
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
