#![deny(rust_2018_idioms, future_incompatible)]

use mpeg2ts_reader::psi::WholeCompactSyntaxPayloadParser;
use std::fs::File;
use std::io::Read;

mod cli;
mod mpegts;
mod net;

use mpeg2ts_reader::demultiplex;
use mpeg2ts_reader::psi;
use std::cell;
use std::rc;

fn file_main(cmd: &cli::FileCmd) -> Result<(), std::io::Error> {
    let mut f = File::open(&cmd.name).unwrap_or_else(|_| panic!("Problem reading {}", cmd.name));
    let mut buf = [0u8; 1880 * 1024];
    let mut ctx = mpegts::DumpDemuxContext::new();
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
    let mut parser = scte35_reader::Scte35SectionProcessor::new(mpegts::DumpSpliceInfoProcessor {
        last_pcr: rc::Rc::new(cell::Cell::new(None)),
    });
    let header = psi::SectionCommonHeader::new(&data[..psi::SectionCommonHeader::SIZE]);
    let mut ctx = mpegts::DumpDemuxContext::new();
    parser.section(&mut ctx, &header, &data[..]);
    Ok(())
}
fn main() {
    env_logger::init();
    match cli::cli() {
        Err(e) => {
            eprintln!("Invalid command line: {}", e);
            ::std::process::exit(1);
        }
        Ok(cli::CommandSpec::Net(cmd)) => {
            net::main(&cmd);
        }
        Ok(cli::CommandSpec::File(cmd)) => file_main(&cmd).expect("file"),
        Ok(cli::CommandSpec::Section(cmd)) => {
            section_main(&cmd).expect("section");
        }
    }
    //tokio_main();
}
