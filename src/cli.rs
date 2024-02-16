use clap::{Arg, ArgMatches, Command};
use std::net::{Ipv4Addr, SocketAddr};

pub struct Group {
    pub addr: Ipv4Addr,
    pub ifaddr: Ipv4Addr,
}

pub enum Fec {
    None,
    ProMpeg,
}

pub struct NetCmd {
    pub addr: SocketAddr,
    pub group: Option<Group>,
    pub fec: Fec,
    pub udpts: bool,
}

pub struct FileCmd {
    pub name: String,
}

pub enum SectEncoding {
    Hex,
    Base64,
}

pub struct SectCmd {
    pub value: String,
    pub encoding: SectEncoding,
}

pub enum CommandSpec {
    Net(NetCmd),
    File(FileCmd),
    Section(SectCmd),
}

fn group(matches: &ArgMatches) -> Option<Group> {
    matches.get_one::<String>("mcast").map(|mcast| {
        let ifaddr = if let Some(addr) = matches.get_one::<String>("ifaddr") {
            addr.parse().unwrap()
        } else {
            "0.0.0.0".parse().unwrap()
        };
        Group {
            addr: mcast.parse().unwrap(),
            ifaddr,
        }
    })
}

fn fec(matches: &ArgMatches) -> Fec {
    match matches.get_one::<String>("fec").map(AsRef::as_ref) {
        Some("prompeg") => Fec::ProMpeg,
        Some(other) => panic!("unsupported FEC mode {:?}", other),
        None => Fec::None,
    }
}

pub fn cli() -> Result<CommandSpec, &'static str> {
    let matches = Command::new("scte35dump")
        .author("David Holroyd")
        .about("Extract SCTE-35 information from MPEG Transport Streams")
        .subcommand(
            Command::new("net")
                .about("Read an RTP-encapsulated transport stream from the network")
                .arg(
                    Arg::new("udp")
                        .short('u')
                        .long("udp")
                        .help("Use TS over UDP transport")
                        .num_args(0)
                        .required(false),
                )
                .arg(
                    Arg::new("port")
                        .short('p')
                        .long("port")
                        .help("UDP port to bind to")
                        .num_args(1)
                        .required(true),
                )
                .arg(
                    Arg::new("bind")
                        .short('b')
                        .long("bind")
                        .num_args(1)
                        .help("IP address to bind to (defaults to 0.0.0.0)"),
                )
                .arg(
                    Arg::new("mcast")
                        .short('m')
                        .help("Multicast group to join")
                        .num_args(1)
                        .required(false),
                )
                .arg(
                    Arg::new("ifaddr")
                        .long("ifaddr")
                        .num_args(1)
                        .help(
                            "IP address of the network interface to be joined to a multicast group",
                        ),
                )
                .arg(
                    Arg::new("fec")
                        .long("fec")
                        .num_args(1)
                        .value_names(&["prompeg"])
                        .help("Style of Forward Error Correction to apply (no FEC if omitted)"),
                ),
        )
        .subcommand(
            Command::new("file")
                .about("Read a transport stream from the named file")
                .arg(Arg::new("NAME").required(true)),
        )
        .subcommand(
            Command::new("section")
                .about("Decode a single splice_info section value given on the command line")
                .arg(
                    Arg::new("base64")
                        .help("The provided section data is base64 encoded")
                        .long("base64")
                        .num_args(0)
                        .required(false),
                )
                .arg(
                    Arg::new("hex")
                        .help("The provided section data is hexidecimal encoded")
                        .long("hex")
                        .num_args(0)
                        .required(false),
                )
                .arg(
                    Arg::new("SECTION")
                        .help("A SCTE-35 splice_info section value")
                        .required(true),
                ),
        )
        .get_matches();

    let cmd = if let Some(matches) = matches.subcommand_matches("net") {
        let addr = match matches.get_one::<String>("bind") {
            Some(a) => a,
            None => "0.0.0.0",
        };
        let udp = matches.get_flag("udp");
        CommandSpec::Net(NetCmd {
            addr: SocketAddr::new(
                addr.parse().map_err(|_| "invalid bind address")?,
                *matches
                    .get_one::<u16>("port")
                    .unwrap(),
            ),
            group: group(matches),
            fec: fec(matches),
            udpts: udp,
        })
    } else if let Some(matches) = matches.subcommand_matches("file") {
        CommandSpec::File(FileCmd {
            name: matches.get_one::<String>("NAME").unwrap().to_string(),
        })
    } else if let Some(matches) = matches.subcommand_matches("section") {
        let enc = if matches.get_flag("hex") {
            SectEncoding::Hex
        } else if matches.get_flag("base64") {
            SectEncoding::Base64
        } else {
            return Err("Either --hex or --base64 must be specified");
        };
        CommandSpec::Section(SectCmd {
            value: matches.get_one::<String>("SECTION").unwrap().to_string(),
            encoding: enc,
        })
    } else {
        return Err("subcommand must be specified");
    };

    Ok(cmd)
}
