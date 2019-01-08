use clap::{App, Arg, ArgMatches, SubCommand};
use std::net::{Ipv4Addr, SocketAddr};

pub struct Group {
    pub addr: Ipv4Addr,
    pub ifaddr: Ipv4Addr,
}

pub struct NetCmd {
    pub addr: SocketAddr,
    pub group: Option<Group>,
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

fn group(matches: &ArgMatches<'_>) -> Option<Group> {
    matches.value_of("mcast").map(|mcast| {
        let ifaddr = if let Some(addr) = matches.value_of("ifaddr") {
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

pub fn cli() -> Result<CommandSpec, &'static str> {
    let matches = App::new("scte35dump")
        .author("David Holroyd")
        .about("Extract SCTE-35 information from MPEG Transport Streams")
        .subcommand(
            SubCommand::with_name("net")
                .about("Read an RTP-encapsulated transport stream from the network")
                .arg(
                    Arg::with_name("port")
                        .short("p")
                        .long("port")
                        .help("UDP port to bind to")
                        .takes_value(true)
                        .required(true),
                ).arg(
                    Arg::with_name("bind")
                        .short("b")
                        .long("bind")
                        .takes_value(true)
                        .help("IP address to bind to (defaults to 0.0.0.0)"),
                ).arg(
                    Arg::with_name("mcast")
                        .short("m")
                        .help("Multicast group to join")
                        .takes_value(true)
                        .required(false),
                ).arg(
                    Arg::with_name("ifaddr")
                        .long("ifaddr")
                        .takes_value(true)
                        .help(
                            "IP address of the network interface to be joined to a multicast group",
                        ),
                ),
        ).subcommand(
            SubCommand::with_name("file")
                .about("Read a transport stream from the named file")
                .arg(Arg::with_name("NAME").required(true)),
        ).subcommand(
            SubCommand::with_name("section")
                .about("Decode a single splice_info section value given on the command line")
                .arg(
                    Arg::with_name("base64")
                        .help("The provided section data is base64 encoded")
                        .long("base64")
                        .takes_value(false)
                        .required(false),
                ).arg(
                    Arg::with_name("hex")
                        .help("The provided section data is hexidecimal encoded")
                        .long("hex")
                        .takes_value(false)
                        .required(false),
                ).arg(
                    Arg::with_name("SECTION")
                        .help("A SCTE-35 splice_info section value")
                        .required(true),
                ),
        ).get_matches();

    let cmd = if let Some(matches) = matches.subcommand_matches("net") {
        let addr = match matches.value_of("bind") {
            Some(a) => a,
            None => "0.0.0.0",
        };
        CommandSpec::Net(NetCmd {
            addr: SocketAddr::new(
                addr.parse().map_err(|_| "invalid bind address")?,
                matches
                    .value_of("port")
                    .unwrap()
                    .parse()
                    .map_err(|_| "invalid port")?,
            ),
            group: group(matches),
        })
    } else if let Some(matches) = matches.subcommand_matches("file") {
        CommandSpec::File(FileCmd {
            name: matches.value_of("NAME").unwrap().to_string(),
        })
    } else if let Some(matches) = matches.subcommand_matches("section") {
        let enc = if matches.is_present("hex") {
            SectEncoding::Hex
        } else if matches.is_present("base64") {
            SectEncoding::Base64
        } else {
            return Err("Either --hex or --base64 must be specified");
        };
        CommandSpec::Section(SectCmd {
            value: matches.value_of("SECTION").unwrap().to_string(),
            encoding: enc,
        })
    } else {
        return Err("subcommand must be specified");
    };

    Ok(cmd)
}
