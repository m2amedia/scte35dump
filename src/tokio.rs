use std::net::SocketAddr;
use tokio_core::net::UdpSocket;
use tokio_core::reactor::Core;
use futures::{Future, Stream};
use futures::stream;
use std::net::Ipv4Addr;
use std::io;
use rtp_rs;
use mpegts;

fn for_each_rtp<F>(socket: UdpSocket, f: F) -> impl Future
    where
        F: FnMut(Result<rtp_rs::RtpReader,io::Error>, SocketAddr) + Sized
{
    let mut buf = Vec::new();
    buf.resize(9000, 0);
    let recv = stream::unfold((socket, buf, f), |(socket, buf, mut f)| {
        let fut = socket.recv_dgram(buf).and_then(|(sock, buf, size, addr)| {
            f(rtp_rs::RtpReader::new(&buf[..size]), addr);
            Ok( ((), (sock, buf, f)) )
        });
        Some(fut)
    }).for_each(|_| { Ok(()) });
    recv
}

fn tokio_main() {
    let addr = "0.0.0.0:1234".parse::<SocketAddr>().unwrap();
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let socket = UdpSocket::bind(&addr, &handle).unwrap();
    let group = Ipv4Addr::new(239,100,0,1);
    let iface = Ipv4Addr::new(0,0,0,0);
    socket.join_multicast_v4(&group, &iface).expect("failed to join multicast group");

    let mut demux = mpegts::create_demux();
    let recv = for_each_rtp(socket, move |rtp, addr| {
        match rtp {
            Ok(rtp) => {
                //println!("got a packet from {:?}, seq {}", addr, rtp.sequence_number());
                demux.push(rtp.payload());
            },
            Err(e) => {
                println!("rtp error from {:?}: {:?}", addr, e);
            }
        }
    });
    core.run(recv);
}
