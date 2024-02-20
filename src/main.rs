#![deny(rust_2018_idioms, future_incompatible)]

use mpeg2ts_reader::psi::WholeCompactSyntaxPayloadParser;
use std::fs::File;
use std::io::Read;

mod cli;
mod mpegts;
mod net;

use base64::Engine as _;
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
    if cmd.hex && cmd.base64 {
        return Err("Only specify one of either --base64 or --hex".to_string());
    }
    if !cmd.hex && !cmd.base64 {
        return Err("Specify at least one of either --base64 or --hex".to_string());
    }
    let data = if cmd.base64 {
        base64::engine::GeneralPurpose::new(
            &base64::alphabet::STANDARD,
            base64::engine::general_purpose::PAD,
        )
            .decode(cmd.value.as_bytes())
            .map_err(|e| format!("base64 decoding problem: {:?}", e))?
    } else {
        hex::decode(cmd.value.as_bytes())
            .map_err(|e| format!("hex decoding problem: {:?}", e))?
    };
    let mut parser = scte35_reader::Scte35SectionProcessor::new(mpegts::DumpSpliceInfoProcessor {
        elementary_pid: None,
        last_pcr: rc::Rc::new(cell::Cell::new(None)),
    });
    let header = psi::SectionCommonHeader::new(&data[..psi::SectionCommonHeader::SIZE]);
    let mut ctx = mpegts::DumpDemuxContext::new();
    parser.section(&mut ctx, &header, &data[..]);
    Ok(())
}
fn main() {
    env_logger::init();
    match argh::from_env::<cli::Cli>().nested {
        cli::CommandSpec::Net(cmd) => {
            net::main(&cmd);
        }
        cli::CommandSpec::File(cmd) => file_main(&cmd).expect("file"),
        cli::CommandSpec::Section(cmd) => {
            section_main(&cmd).expect("section");
        }
    }
}
