#![feature(conservative_impl_trait)]

extern crate futures;
extern crate tokio_core;
extern crate rtp_rs;
extern crate mpeg2ts_reader;
extern crate hexdump;
extern crate bitreader;
extern crate net2;
extern crate clap;
extern crate base64;
extern crate hex;

use std::fs::File;
use std::io::Read;
use mpeg2ts_reader::psi::SectionProcessor;

mod mpegts;
mod tokio;
mod cli;


fn net2_main(cmd: &cli::NetCmd) {
    let udp = net2::UdpBuilder::new_v4().unwrap();
    udp.reuse_address(true).unwrap();
    let sock = udp.bind(cmd.addr).expect("failed to bind socket");
    if let Some(ref group) = cmd.group {
        sock.join_multicast_v4(&group.addr, &group.ifaddr).expect("failed to join multicast group");
    }
    let mut buf = Vec::new();
    buf.resize(9000, 0);
    let mut demux = mpegts::create_demux();
    loop {
        match sock.recv_from(&mut buf[..]) {
            Ok( (size, addr) ) => {
                let rtp = rtp_rs::RtpReader::new(&buf[..size]);
                match rtp {
                    Ok(rtp) => {
                        //println!("got a packet from {:?}, seq {}", addr, rtp.sequence_number());
                        demux.push(rtp.payload());
                    },
                    Err(e) => {
                        println!("rtp error from {:?}: {:?}", addr, e);
                    }
                }
            },
            Err(e) => {
                println!("recv_from() error: {:?}", e);
                return;
            }
        }
    }
}

fn file_main(cmd: &cli::FileCmd) -> Result<(), std::io::Error> {
    let mut f = File::open(&cmd.name).expect(&format!("Problem reading {}", cmd.name));
    let mut buf = [0u8; 188*1024];
    let mut demux = mpegts::create_demux();
    loop {
        match f.read(&mut buf[..])? {
            0 => break,
            // TODO: if not all bytes are consumed, track buf remainder
            n => demux.push(&buf[0..n]),
        }
    }
    Ok(())

}

fn section_main(cmd: &cli::SectCmd) -> Result<(), String> {
    let data = match cmd.encoding {
        cli::SectEncoding::Base64 => base64::decode(cmd.value.as_bytes()).map_err(|e| format!("base64 decoding problem: {:?}", e))?,
        cli::SectEncoding::Hex => hex::decode(cmd.value.as_bytes()).map_err(|e| format!("hex decoding problem: {:?}", e))?,
    };
    let mut parser = mpeg2ts_reader::psi::SectionParser::new(|header, buf| {
        mpegts::Scte35SectionProcessor{}.process(header, buf)
    });
    parser.begin_new_section(&data[..]);
    Ok(())
}
fn main() {
    match cli::cli() {
        Err(e) => {
            eprintln!("Invalid command line: {}", e);
            ::std::process::exit(1);
        },
        Ok(cli::CommandSpec::Net(cmd)) => {
            net2_main(&cmd);
        },
        Ok(cli::CommandSpec::File(cmd)) => {
            file_main(&cmd);
        },
        Ok(cli::CommandSpec::Section(cmd)) => {
            section_main(&cmd);
        },
    }
    //tokio_main();
}
