use crate::common::{self, dtp_config, MAX_DATAGRAM_SIZE};
// pub use super::log_socket_addr_type;
pub use log::{debug, error, info, trace, warn};
use mio::net::UdpSocket;
pub use mio::{Events, Poll, Token};
pub use quiche::{Config, MAX_CONN_ID_LEN, MIN_CLIENT_INITIAL_LEN};
pub use ring::rand::*;
pub use std::collections::HashMap;
use std::{fmt::format, net};
pub const SERVER: Token = Token(0);
// pub const MAX_DATAGRAM_SIZE: usize = 1350;
pub use std::fs::File;
pub use std::io::Write;
struct PartialResponse {
    body: Vec<u8>,
    written: usize,
}

struct Client {
    conn: quiche::Connection,
    file: Option<File>,
    partial_responses: HashMap<u64, PartialResponse>,
}
type ClientMap = HashMap<quiche::ConnectionId<'static>, Client>;

// use for record stream id and data to file.then can use python to analysis
fn handle_stream(client: &mut Client, stream_id: u64, buf: &[u8]) {
    let conn = &mut client.conn;
    debug!(
        "{} got GET request for {:?} on stream {}",
        conn.trace_id(),
        buf,
        stream_id
    );
    if client.file.is_some() {
        let file = &client.file;
        let mut file = std::io::BufWriter::new(file.as_ref().unwrap());
        // Get environment variable,if LOG_PURE is true,then write pure data to file
        let log_pure = std::env::var("LOG_PURE").unwrap_or("false".to_string());
        if log_pure == "true" {
            file.write_all(buf).unwrap();
        } else {
            let data = format!("{} {}", stream_id, String::from_utf8_lossy(buf));
            file.write_all(data.as_bytes()).unwrap();
        }
        // file.write_all(b"\n").unwrap();
        file.flush().unwrap();
    }

    // let written = match conn.stream_send(stream_id, &body, true) {
    //     Ok(v) => v,

    //     Err(quiche::Error::Done) => 0,

    //     Err(e) => {
    //         error!("{} stream send failed {:?}", conn.trace_id(), e);
    //         return;
    //     },
    // };

    // let block = std::sync::Arc::new(quiche::Block {
    //     size: buf.len() as u64,
    //     priority: 0,
    //     deadline: 200,
    // });

    // let written = match conn.block_send(stream_id, &buf, true, block) {
    //     Ok(v) => v,

    //     Err(quiche::Error::Done) => 0,

    //     Err(e) => {
    //         error!("{} stream send failed {:?}", conn.trace_id(), e);
    //         return;
    //     },
    // };

    // if written < buf.len() {
    //     let response = PartialResponse {
    //         body: buf.to_vec(),
    //         written,
    //     };
    //     client.partial_responses.insert(stream_id, response);
    // }
}

/// Handles newly writable streams.
fn handle_writable(client: &mut Client, stream_id: u64) {
    let conn = &mut client.conn;

    debug!("{} stream {} is writable", conn.trace_id(), stream_id);

    if !client.partial_responses.contains_key(&stream_id) {
        return;
    }

    let resp = client.partial_responses.get_mut(&stream_id).unwrap();
    let body = &resp.body[resp.written..];
    let block = std::sync::Arc::new(quiche::Block {
        size: body.len() as u64,
        priority: 0,
        deadline: 200,
    });

    let written = match conn.block_send(stream_id, &body, true, block) {
        Ok(v) => v,

        Err(quiche::Error::Done) => 0,

        Err(e) => {
            error!("{} stream send failed {:?}", conn.trace_id(), e);
            return;
        },
    };

    // let written = match conn.stream_send(stream_id, body, true) {
    //     Ok(v) => v,

    //     Err(quiche::Error::Done) => 0,

    //     Err(e) => {
    //         client.partial_responses.remove(&stream_id);

    //         error!("{} stream send failed {:?}", conn.trace_id(), e);
    //         return;
    //     },
    // };

    resp.written += written;

    if resp.written == resp.body.len() {
        client.partial_responses.remove(&stream_id);
    }
}

pub fn run_server(listen_addr: &str) {
    let mut buf = [0; 65535];
    let mut out = [0; MAX_DATAGRAM_SIZE];
    // Setup the event loop.
    let mut poll = mio::Poll::new().unwrap();
    let mut events = mio::Events::with_capacity(1024);

    // Create the UDP listening socket, and register it with the event loop.
    let mut socket =
        mio::net::UdpSocket::bind(listen_addr.parse().unwrap()).unwrap();
    poll.registry()
        .register(&mut socket, mio::Token(0), mio::Interest::READABLE)
        .unwrap();

    // Create the configuration for the QUIC connections.
    let mut config = dtp_config(true);

    let mut clients = ClientMap::new();

    let local_addr = socket.local_addr().unwrap();

    'outmost: loop {
        // Find the shorter timeout from all the active connections.
        let timeout = clients.values().filter_map(|c| c.conn.timeout()).min();

        poll.poll(&mut events, timeout).unwrap();

        // Read incoming UDP packets from the socket and feed them to quiche,
        // until there are no more packets to read.
        'read: loop {
            // If the event loop reported no events, it means that the timeout
            // has expired, so handle it without attempting to read packets. We
            // will then proceed with the send loop.
            if events.is_empty() {
                debug!("timed out");

                clients.values_mut().for_each(|c| c.conn.on_timeout());

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

            let pkt_buf = &mut buf[..len];

            // Parse the QUIC packet's header.
            let hdr = match quiche::Header::from_slice(
                pkt_buf,
                quiche::MAX_CONN_ID_LEN,
            ) {
                Ok(v) => v,

                Err(e) => {
                    error!("Parsing packet header failed: {:?}", e);
                    continue 'read;
                },
            };

            trace!("got packet {:?}", hdr);
            let conn_id = common::generate_conn_id(&hdr).into();
            // Lookup a connection based on the packet's connection ID. If there
            // is no connection matching, create a new one.
            let client = if !clients.contains_key(&hdr.dcid)
                && !clients.contains_key(&conn_id)
            {
                if hdr.ty != quiche::Type::Initial {
                    error!("Packet is not Initial");
                    continue 'read;
                }

                if !quiche::version_is_supported(hdr.version) {
                    warn!("Doing version negotiation");

                    let len =
                        quiche::negotiate_version(&hdr.scid, &hdr.dcid, &mut out)
                            .unwrap();

                    let out = &out[..len];

                    if let Err(e) = socket.send_to(out, from) {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            debug!("send() would block");
                            break;
                        }

                        panic!("send() failed: {:?}", e);
                    }
                    continue 'read;
                }

                let mut scid = [0; quiche::MAX_CONN_ID_LEN];
                scid.copy_from_slice(&conn_id);

                let scid = quiche::ConnectionId::from_ref(&scid);

                // Token is always present in Initial packets.
                let token = hdr.token.as_ref().unwrap();

                // Do stateless retry if the client didn't send a token.
                if token.is_empty() {
                    warn!("Doing stateless retry");

                    let new_token = common::mint_token(&hdr, &from);

                    let len = quiche::retry(
                        &hdr.scid,
                        &hdr.dcid,
                        &scid,
                        &new_token,
                        hdr.version,
                        &mut out,
                    )
                    .unwrap();

                    let out = &out[..len];

                    if let Err(e) = socket.send_to(out, from) {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            debug!("send() would block");
                            break;
                        }

                        panic!("send() failed: {:?}", e);
                    }
                    continue 'read;
                }

                let odcid = common::validate_token(&from, token);

                // The token was not valid, meaning the retry failed, so
                // drop the packet.
                if odcid.is_none() {
                    error!("Invalid address validation token");
                    continue 'read;
                }

                if scid.len() != hdr.dcid.len() {
                    error!("Invalid destination connection ID");
                    continue 'read;
                }

                // Reuse the source connection ID we sent in the Retry packet,
                // instead of changing it again.
                let scid = hdr.dcid.clone();

                debug!("New connection: dcid={:?} scid={:?}", hdr.dcid, scid);

                let conn = quiche::accept(
                    &scid,
                    odcid.as_ref(),
                    local_addr,
                    from,
                    &mut config,
                )
                .unwrap();
                //create file to record stream id and data for test analysis
                let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
                let file_name = format! { "{}{}{}{}",manifest_dir,"/tests/result/",conn.trace_id(),".txt"};
                let file = File::options()
                    .write(true)
                    .create(true)
                    .open(file_name)
                    .ok();
                if file.is_none() {
                    error!("create file failed");
                }
                let client = Client {
                    conn,
                    file,
                    partial_responses: HashMap::new(),
                };

                clients.insert(scid.clone(), client);

                clients.get_mut(&scid).unwrap()
            } else {
                match clients.get_mut(&hdr.dcid) {
                    Some(v) => v,

                    None => clients.get_mut(&conn_id).unwrap(),
                }
            };

            let recv_info = quiche::RecvInfo {
                to: socket.local_addr().unwrap(),
                from,
            };

            // Process potentially coalesced packets.
            let read = match client.conn.recv(pkt_buf, recv_info) {
                Ok(v) => v,

                Err(e) => {
                    error!("{} recv failed: {:?}", client.conn.trace_id(), e);
                    continue 'read;
                },
            };

            debug!("{} processed {} bytes", client.conn.trace_id(), read);

            if client.conn.is_in_early_data() || client.conn.is_established() {
                // Handle writable streams.
                for stream_id in client.conn.writable() {
                    handle_writable(client, stream_id);
                }

                // Process all readable streams.
                for s in client.conn.readable() {
                    while let Ok((read, fin)) =
                        client.conn.stream_recv(s, &mut buf)
                    {
                        debug!(
                            "{} received {} bytes",
                            client.conn.trace_id(),
                            read
                        );

                        let stream_buf = &buf[..read];

                        debug!(
                            "{} stream {} has {} bytes (fin? {})",
                            client.conn.trace_id(),
                            s,
                            stream_buf.len(),
                            fin
                        );
                        //record client data handle and react according to client data
                        handle_stream(client, s, stream_buf);
                    }
                }
            }
        }

        // Generate outgoing QUIC packets for all active connections and send
        // them on the UDP socket, until quiche reports that there are no more
        // packets to be sent.
        for client in clients.values_mut() {
            loop {
                let (write, send_info) = match client.conn.send(&mut out) {
                    Ok(v) => v,

                    Err(quiche::Error::Done) => {
                        debug!("{} done writing", client.conn.trace_id());
                        break;
                    },

                    Err(e) => {
                        error!("{} send failed: {:?}", client.conn.trace_id(), e);

                        client.conn.close(false, 0x1, b"fail").ok();
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

                debug!("{} written {} bytes", client.conn.trace_id(), write);
            }
        }

        let mut exit_falg = false;
        // Garbage collect closed connections.
        clients.retain(|_, ref mut c| {
            debug!("Collecting garbage");

            if c.conn.is_closed() {
                debug!(
                    "{} connection collected {:?}",
                    c.conn.trace_id(),
                    c.conn.stats()
                );
                exit_falg = true;
                println!(
                    "\nPlease check the result in tests/result,file name is {}",
                    c.conn.trace_id()
                );
            }

            !c.conn.is_closed()
        });
        if exit_falg {
            break 'outmost;
        }
    }
    println!("server exit");
}
