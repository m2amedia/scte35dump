use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;
use argh::FromArgs;

/// Extract SCTE-35 information from MPEG Transport Streams
#[derive(FromArgs, Debug)]
pub(crate) struct Cli {
    #[argh(subcommand)]
    pub nested: CommandSpec,
}

#[derive(Debug)]
pub enum Fec {
    ProMpeg,
}
impl FromStr for Fec {
    type Err = FecErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "prompeg" => Ok(Fec::ProMpeg),
            _ => Err(FecErr(s.to_string()))
        }
    }
}
impl fmt::Display for Fec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("prompeg")
    }
}

pub struct FecErr(String);
impl fmt::Display for FecErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("Unsupported Forward Error Correction mode ")?;
        f.write_str(self.0.as_ref())
    }
}


/// Read a transport stream from the network
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "net")]
pub struct NetCmd {
    /// UDP port to bind to
    #[argh(option, short = 'p')]
    pub port: u16,
    /// multicast group to join
    #[argh(option, short = 'm')]
    pub mcast: Option<String>,
    /// IP address of the network interface to be joined to a multicast group
    #[argh(option)]
    pub ifaddr: Option<String>,
    /// style of Forward Error Correction to apply (no FEC if omitted)
    #[argh(option)]
    pub fec: Option<Fec>,
    /// use TS over UDP transport (detault is TS over RTP)
    #[argh(switch)]
    pub udp: bool,
}

/// Read a transport stream from the named file
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "file")]
pub struct FileCmd {
    /// the mpegts file name
    #[argh(positional)]
    pub name: String,
}

/// Decode a single splice_info section value given on the command line
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "section")]
pub struct SectCmd {
    /// the provided section data is hexidecimal encoded
    #[argh(switch)]
    pub hex: bool,
    /// the provided section data is base64 encoded
    #[argh(switch)]
    pub base64: bool,
    /// A SCTE-35 splice_info section value
    #[argh(positional)]
    pub value: String,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
pub enum CommandSpec {
    Net(NetCmd),
    File(FileCmd),
    Section(SectCmd),
}

/*
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
    let matches =
        Command::new("scte35dump")
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
                    .arg(Arg::new("ifaddr").long("ifaddr").num_args(1).help(
                        "IP address of the network interface to be joined to a multicast group",
                    ))
                    .arg(
                        Arg::new("fec")
                            .long("fec")
                            .num_args(1)
                            .value_names(["prompeg"])
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
                matches
                    .get_one::<String>("port")
                    .unwrap()
                    .parse::<u16>()
                    .map_err(|_| "invalid port")?,
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
*/