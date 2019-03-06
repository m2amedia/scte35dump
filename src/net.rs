use crate::cli;
use crate::mpegts;
use mpeg2ts_reader::demultiplex;

pub fn main(cmd: &cli::NetCmd) {
    let udp = net2::UdpBuilder::new_v4()
        .expect("Failed to create IPv4 socket");
    udp.reuse_address(true)
        .expect("Failed to configure socket for address reuse");
    let sock = udp.bind(cmd.addr)
        .expect("failed to bind socket");
    if let Some(ref group) = cmd.group {
        sock.join_multicast_v4(&group.addr, &group.ifaddr)
            .expect("failed to join multicast group");
    }
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
