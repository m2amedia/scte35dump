use crate::cli;
use crate::mpegts;
use mpeg2ts_reader::demultiplex;
use smpte2022_1_fec::heap_pool::HeapPacket;
use smpte2022_1_fec::heap_pool::HeapPool;
use smpte2022_1_fec::BufferPool;
use smpte2022_1_fec::Decoder;
use smpte2022_1_fec::Packet;
use smpte2022_1_fec::PacketStatus;
use smpte2022_1_fec::Receiver;
use std::io;
use std::net;

pub fn main(cmd: &cli::NetCmd) {
    let sock = create_socket(cmd, cmd.addr.port()).expect("Failed to create socket");
    match cmd.fec {
        cli::Fec::None => simple_main(sock),
        cli::Fec::ProMpeg => fec_main(sock, cmd).unwrap(),
    }
}

/// Simple loop that blocks in recv_from() (which minimises the number of syscalls vs. something
/// that also does select/epoll/etc in addition to calling recv_from().
fn simple_main(sock: std::net::UdpSocket) {
    let mut buf = Vec::new();
    buf.resize(9000, 0);
    let mut ctx = mpegts::DumpDemuxContext::new();
    let mut demux = demultiplex::Demultiplex::new(&mut ctx);
    let mut expected = None;
    loop {
        match sock.recv_from(&mut buf[..]) {
            Ok((size, addr)) => {
                let rtp = rtp_rs::RtpReader::new(&buf[..size]);
                match rtp {
                    Ok(rtp) => {
                        let this_seq = rtp.sequence_number();
                        if let Some(seq) = expected {
                            if this_seq != seq {
                                println!(
                                    "RTP: sequence mismatch: expected {:?}, got {:?}",
                                    seq,
                                    rtp.sequence_number()
                                );
                            }
                        }
                        expected = Some(this_seq.next());
                        //println!("got a packet from {:?}, seq {:?}", addr, rtp.sequence_number());
                        demux.push(&mut ctx, rtp.payload());
                    }
                    Err(e) => {
                        println!("rtp error from {:?}: {:?}", addr, e);
                    }
                }
            }
            Err(e) => {
                println!("recv_from() error: {:?}", e);
                return;
            }
        }
    }
}

struct ScteFecReceiver {
    ctx: mpegts::DumpDemuxContext,
    demux: demultiplex::Demultiplex<mpegts::DumpDemuxContext>,
    expected_seq: Option<rtp_rs::Seq>,
}
impl Receiver<HeapPacket> for ScteFecReceiver {
    fn receive(&mut self, packets: impl Iterator<Item = (HeapPacket, PacketStatus)>) {
        for (pk, _pk_status) in packets {
            let rtp = rtp_rs::RtpReader::new(&pk.payload());
            match rtp {
                Ok(rtp) => {
                    let this_seq = rtp.sequence_number();
                    if let Some(seq) = self.expected_seq {
                        if this_seq != seq {
                            println!(
                                "RTP: sequence mismatch: expected {:?}, got {:?}",
                                seq,
                                rtp.sequence_number()
                            );
                        }
                    }
                    self.expected_seq = Some(this_seq.next());
                    //println!("got a packet from {:?}, seq {:?}", addr, rtp.sequence_number());
                    self.demux.push(&mut self.ctx, rtp.payload());
                }
                Err(e) => {
                    println!("rtp error: {:?}", e);
                }
            }
        }
    }
}

/// Supports FEC decoding, which means needing to read from multiple sockets, which can't really
/// be done with blocking as in simple_main()
fn fec_main(main_sock: std::net::UdpSocket, cmd: &cli::NetCmd) -> Result<(), std::io::Error> {
    const MAIN: mio::Token = mio::Token(0);
    const FEC_ONE: mio::Token = mio::Token(1);
    const FEC_TWO: mio::Token = mio::Token(2);

    const PACKET_SIZE_MAX: usize = 1500;
    const PACKET_COUNT_MAX: usize = 10 * 10 * 2 + 4 + 25;

    let main_sock =
        mio::net::UdpSocket::from_socket(main_sock).expect("Failed to create main socket");
    let fec_one = mio::net::UdpSocket::from_socket(
        create_socket(cmd, cmd.addr.port() + 2).expect("Failed to create FEC socket"),
    )
    .expect("Failed to create FEC socket");
    let fec_two = mio::net::UdpSocket::from_socket(
        create_socket(cmd, cmd.addr.port() + 4).expect("Failed to create FEC socket"),
    )
    .expect("Failed to create FEC socket");

    let buffer_pool = HeapPool::new(PACKET_COUNT_MAX, PACKET_SIZE_MAX);
    let mut ctx = mpegts::DumpDemuxContext::new();
    let demux = demultiplex::Demultiplex::new(&mut ctx);
    let recv = ScteFecReceiver {
        ctx,
        demux,
        expected_seq: None,
    };
    let mut decoder = Decoder::new(buffer_pool.clone(), recv);

    let poll = mio::Poll::new()?;
    poll.register(
        &main_sock,
        MAIN,
        mio::Ready::readable(),
        mio::PollOpt::edge(),
    )?;
    poll.register(
        &fec_one,
        FEC_ONE,
        mio::Ready::readable(),
        mio::PollOpt::edge(),
    )?;
    poll.register(
        &fec_two,
        FEC_TWO,
        mio::Ready::readable(),
        mio::PollOpt::edge(),
    )?;

    let mut events = mio::Events::with_capacity(1024);
    loop {
        poll.poll(&mut events, None)?;
        for event in &events {
            match event.token() {
                MAIN => loop {
                    let mut pk = buffer_pool.allocate().expect("allocating main buffer");
                    let size = match main_sock.recv(pk.payload_mut()) {
                        Ok(s) => s,
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            break;
                        }
                        e => panic!("err={:?}", e),
                    };
                    pk.truncate(size);
                    decoder
                        .add_main_packets(vec![pk].into_iter())
                        .expect("decoding main packet");
                },
                FEC_ONE => loop {
                    let mut pk = buffer_pool.allocate().expect("allocating fec1 buffer");
                    let size = match fec_one.recv(pk.payload_mut()) {
                        Ok(s) => s,
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            break;
                        }
                        e => panic!("err={:?}", e),
                    };
                    pk.truncate(size);
                    decoder
                        .add_column_packets(vec![pk].into_iter())
                        .expect("decoding column packet");
                },
                FEC_TWO => loop {
                    let mut pk = buffer_pool.allocate().expect("allocating fec2 buffer");
                    let size = match fec_two.recv(pk.payload_mut()) {
                        Ok(s) => s,
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            break;
                        }
                        e => panic!("err={:?}", e),
                    };
                    pk.truncate(size);
                    decoder
                        .add_row_packets(vec![pk].into_iter())
                        .expect("decoding row packet");
                },
                t => panic!("unexpected {:?}", t),
            }
        }
    }
}

fn create_socket(cmd: &cli::NetCmd, port: u16) -> Result<std::net::UdpSocket, io::Error> {
    let udp = net2::UdpBuilder::new_v4()?;
    udp.reuse_address(true)?; // TODO: only if mcast?

    let addr = net::SocketAddr::new(cmd.addr.ip(), port);
    let sock = udp.bind(addr)?;
    if let Some(ref group) = cmd.group {
        sock.join_multicast_v4(&group.addr, &group.ifaddr)?;
    }
    Ok(sock)
}
