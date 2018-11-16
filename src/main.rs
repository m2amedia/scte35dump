extern crate rtp_rs;
#[macro_use]
extern crate mpeg2ts_reader;
extern crate base64;
extern crate bitreader;
extern crate clap;
extern crate hex;
extern crate hexdump;
extern crate net2;
extern crate scte35_reader;

use mpeg2ts_reader::psi::SectionProcessor;
use std::fs::File;
use std::io::Read;

mod cli;
mod mpegts;

use mpeg2ts_reader::demultiplex;
use mpeg2ts_reader::psi;
use std::thread;

fn net2_main(cmd: &cli::NetCmd) {
    let udp = net2::UdpBuilder::new_v4().unwrap();
    udp.reuse_address(true).unwrap();
    let sock = udp.bind(cmd.addr).expect("failed to bind socket");
    if let Some(ref group) = cmd.group {
        sock.join_multicast_v4(&group.addr, &group.ifaddr)
            .expect("failed to join multicast group");
    }
    let mut buf = Vec::new();
    buf.resize(9000, 0);
    let mut ctx = mpegts::DumpDemuxContext::new(mpegts::DumpStreamConstructor);
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
                        //println!("got a packet from {:?}, seq {}", addr, rtp.sequence_number());
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

fn file_main(cmd: &cli::FileCmd) -> Result<(), std::io::Error> {
    let mut f = File::open(&cmd.name).expect(&format!("Problem reading {}", cmd.name));
    let mut buf = [0u8; 1880 * 1024];
    let mut ctx = mpegts::DumpDemuxContext::new(mpegts::DumpStreamConstructor);
    let mut demux = demultiplex::Demultiplex::new(&mut ctx);
    loop {
        match f.read(&mut buf[..])? {
            0 => break,
            // TODO: if not all bytes are consumed, track buf remainder
            n => demux.push(&mut ctx, &buf[0..n]),
        }
    }
    Ok(())
}

fn section_main(cmd: &cli::SectCmd) -> Result<(), String> {
    let data = match cmd.encoding {
        cli::SectEncoding::Base64 => base64::decode(cmd.value.as_bytes())
            .map_err(|e| format!("base64 decoding problem: {:?}", e))?,
        cli::SectEncoding::Hex => hex::decode(cmd.value.as_bytes())
            .map_err(|e| format!("hex decoding problem: {:?}", e))?,
    };
    let mut parser = scte35_reader::Scte35SectionProcessor::new(mpegts::DumpSpliceInfoProcessor);
    let header = psi::SectionCommonHeader::new(&data[..psi::SectionCommonHeader::SIZE]);
    let mut ctx = mpegts::DumpDemuxContext::new(mpegts::DumpStreamConstructor);
    parser.start_section(&mut ctx, &header, &data[..]);
    Ok(())
}
fn main() {
    let child = thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(move || {
        // 'scte35dump --help' would exhaust stack under windows -- try running on a thread with
        // much larger stack?
        cli::cli()
    });

    let child = match child {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to spawn thread");
            return;
        }
    };

    match child.join().unwrap() {
        Err(e) => {
            eprintln!("Invalid command line: {}", e);
            ::std::process::exit(1);
        }
        Ok(cli::CommandSpec::Net(cmd)) => {
            net2_main(&cmd);
        }
        Ok(cli::CommandSpec::File(cmd)) => file_main(&cmd).expect("file"),
        Ok(cli::CommandSpec::Section(cmd)) => {
            section_main(&cmd).expect("section");
        }
    }
    //tokio_main();
}
