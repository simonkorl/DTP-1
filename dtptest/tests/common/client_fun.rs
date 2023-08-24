use crate::common::{
    dtp_config, hex_dump, HTTP_REQ_STREAM_ID, MAX_DATAGRAM_SIZE,
};
use log::{debug, error, info};
use quiche::Error;
use ring::rand::{SecureRandom, SystemRandom};
use std::fs::File;
use std::io::prelude::*;
use std::net::SocketAddr;
pub fn run_client(peer_addr: &str, file_path: &str) {
    let mut buf = [0; 65535];
    let mut out = [0; MAX_DATAGRAM_SIZE];

    // Setup the event loop.
    let mut poll = mio::Poll::new().unwrap();
    let mut events = mio::Events::with_capacity(1024);

    // Resolve server address.
    let peer_addr: SocketAddr =
        peer_addr.parse().expect("Unable to parse socket address");

    // Bind to INADDR_ANY or IN6ADDR_ANY depending on the IP family of the
    // server address. This is needed on macOS and BSD variants that don't
    // support binding to IN6ADDR_ANY for both v4 and v6.
    let bind_addr = match peer_addr {
        std::net::SocketAddr::V4(_) => "0.0.0.0:0",
        std::net::SocketAddr::V6(_) => "[::]:0",
    };

    // Create the UDP socket backing the QUIC connection, and register it with
    // the event loop.
    let mut socket =
        mio::net::UdpSocket::bind(bind_addr.parse().unwrap()).unwrap();
    poll.registry()
        .register(
            &mut socket,
            mio::Token(0),
            mio::Interest::READABLE | mio::Interest::WRITABLE,
        )
        .unwrap();
    // Create the configuration for the QUIC connection.
    // *CAUTION*: verify method should not be set to `false` in production!!!
    let mut config = dtp_config(false);

    // Generate a random source connection ID for the connection.
    let mut scid = [0; quiche::MAX_CONN_ID_LEN];
    SystemRandom::new().fill(&mut scid[..]).unwrap();

    let scid = quiche::ConnectionId::from_ref(&scid);

    // Get local address.
    let local_addr = socket.local_addr().unwrap();

    // Create a QUIC connection and initiate handshake.
    let mut conn =
        quiche::connect(None, &scid, local_addr, peer_addr, &mut config).unwrap();

    info!(
        "connecting to {:} from {:} with scid {}",
        peer_addr,
        socket.local_addr().unwrap(),
        hex_dump(&scid)
    );

    let (write, send_info) = conn.send(&mut out).expect("initial send failed");

    while let Err(e) = socket.send_to(&out[..write], send_info.to) {
        if e.kind() == std::io::ErrorKind::WouldBlock {
            debug!("send() would block");
            continue;
        }

        panic!("send() failed: {:?}", e);
    }

    debug!("written {}", write);

    let req_start = std::time::Instant::now();
    let mut send_offset = 0;
    let mut send_block_count = 1;
    'outmost: loop {
        poll.poll(&mut events, conn.timeout()).unwrap();
        for event in &events {
            // Read incoming UDP packets from the socket and feed them to quiche,
            // until there are no more packets to read.
            if event.is_readable() {
                'read: loop {
                    // If the event loop reported no events, it means that the timeout
                    // has expired, so handle it without attempting to read packets. We
                    // will then proceed with the send loop.
                    if events.is_empty() {
                        debug!("timed out");

                        conn.on_timeout();
                        break 'read;
                    }

                    let (len, from) = match socket.recv_from(&mut buf) {
                        Ok(v) => v,

                        Err(e) => {
                            // There are no more UDP packets to read, so end the read
                            // loop.
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                debug!("recv() would block");
                                break 'read;
                            }

                            panic!("recv() failed: {:?}", e);
                        },
                    };

                    debug!("got {} bytes", len);

                    let recv_info = quiche::RecvInfo {
                        to: socket.local_addr().unwrap(),
                        from,
                    };

                    // Process potentially coalesced packets.
                    let read = match conn.recv(&mut buf[..len], recv_info) {
                        Ok(v) => v,

                        Err(e) => {
                            error!("recv failed: {:?}", e);
                            continue 'read;
                        },
                    };

                    debug!("processed {} bytes", read);
                }
                debug!("done reading");

                if conn.is_closed() {
                    info!("connection closed, {:?}", conn.stats());
                    break;
                }

                // Process all readable streams.
                for s in conn.readable() {
                    while let Ok((read, fin)) = conn.stream_recv(s, &mut buf) {
                        debug!("received {} bytes", read);

                        let stream_buf = &buf[..read];

                        debug!(
                            "stream {} has {} bytes (fin? {})",
                            s,
                            stream_buf.len(),
                            fin
                        );

                        print!("{}", unsafe {
                            std::str::from_utf8_unchecked(stream_buf)
                        });

                        // The server reported that it has no more data to send, which
                        // we got the full response. Close the connection.
                        if s == HTTP_REQ_STREAM_ID && fin {
                            info!(
                                "response received in {:?}, closing...",
                                req_start.elapsed()
                            );

                            conn.close(true, 0x00, b"kthxbye").unwrap();
                        }
                    }
                }

                // Generate outgoing QUIC packets and send them on the UDP socket, until
                // quiche reports that there are no more packets to be sent.
                loop {
                    let (write, send_info) = match conn.send(&mut out) {
                        Ok(v) => v,

                        Err(quiche::Error::Done) => {
                            debug!("done writing");
                            break;
                        },

                        Err(e) => {
                            error!("send failed: {:?}", e);

                            conn.close(false, 0x1, b"fail").ok();
                            break;
                        },
                    };

                    if let Err(e) = socket.send_to(&out[..write], send_info.to) {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            debug!("send() would block");
                            break;
                        }

                        panic!("send() failed: {:?}", e);
                    }

                    debug!("written {}", write);
                }

                if conn.is_closed() {
                    info!("connection closed, {:?}", conn.stats());
                    break;
                }
            } else if event.is_writable() {
                if conn.is_established() {
                    // Send an HTTP request as soon as the connection is established.
                    let mut file = File::open(file_path).unwrap();
                    //get the file data size
                    let file_size = file.metadata().unwrap().len();

                    let stream_capcity = 1024;
                    let mut buf = Vec::with_capacity(stream_capcity);
                    file.seek(std::io::SeekFrom::Start(send_offset)).unwrap();
                    let ret = file
                        .take(stream_capcity as u64)
                        .read_to_end(&mut buf)
                        .unwrap();
                    info!("read file ret:{}", ret);
                    if ret == 0 {
                        conn.close(true, 0x00, b"finished").unwrap();
                        break 'outmost;
                    }

                    // println!("buf:{}", String::from_utf8(buf.clone()).unwrap());
                    let block = std::sync::Arc::new(quiche::Block {
                        size: file_size,
                        priority: 0,
                        deadline: 0xffffffff,
                    });
                    //if use stream_send set true,else set false
                    //check env IS_STREAM_SEND
                    if std::env::var("IS_STREAM_SEND").unwrap() == "true" {
                        if let Err(e) =
                            conn.stream_send(HTTP_REQ_STREAM_ID, &buf, false)
                        {
                            error!("send failed: {:?}", e);
                        } else {
                            send_offset += 1024;
                        }
                    } else {
                        // use block_sends
                        match conn.block_send(
                            HTTP_REQ_STREAM_ID,
                            &buf,
                            false,
                            block,
                        ) {
                            Err(e) => {
                                error!("send failed: {:?}", e);
                            },
                            Ok(v) => {
                                println!("block send success send v:{}", v);
                                if v == 0 {
                                    conn.close(true, 0x00, b"finished").unwrap();
                                    break 'outmost;
                                }
                                send_offset += 1024;
                                // send_block_count += 1;
                            },
                        }
                        // if let Err(e) = conn.block_send(
                        //     // HTTP_REQ_STREAM_ID * send_block_count,
                        //     HTTP_REQ_STREAM_ID,
                        //     &buf,
                        //     false,
                        //     block,
                        // ) {
                        //     error!("send failed: {:?}", e);
                        // } else {
                        //     send_offset += 1024;
                        //     // send_block_count += 1;
                        // }
                    }
                }
            } else {
                panic!("unexpected event {:?}", event);
            }
        }
    }
}
