# scte35dump
Dump [SCTE-35](http://www.scte.org/SCTEDocs/Standards/SCTE%2035%202016.pdf) data from a Transport Stream contained within a file or RTP network stream

```
USAGE:
    scte35dump [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    file       Read a transport stream from the named file
    help       Prints this message or the help of the given subcommand(s)
    net        Read an RTP-encapsulated transport stream from the network
    section    Decode a single splice_info section value given on the command line
```

## Install

 1. Install `rustup`: https://rustup.rs/ (which will give you a Rust build env, including the `cargo` tool)
 2. Run `cargo install scte35dump`
 3. Run `scte35dump --help` for usage instructions


## SCTE-35 spec coverage

Not all commands are currently implemented:
 - [x] `splice_null()`
 - [ ] `splice_schedule()`
 - [x] `splice_insert()`
 - [x] `time_signal()`
 - [x] `bandwidth_reservation()`
 - [ ] `private_command()`

# Examples

## The `file` subcommand

Dump from a local transport stream file

```
scte35dump file test-dump.ts
```

## The `net` subcommand

Dump from an RTP multicast stream (add the `--udp` option to use plain UDP without RTP encapsulation).

```
$ scte35dump net -m 234.10.10.1 -p 5001 --ifaddr 192.168.0.11
unhandled pid 264
unhandled pid 8191
unhandled pid 256
unhandled pid 1500
unhandled pid 4096
new table for pid 4096, program 1
new PMT entry PID 256 (in program_number 1)
new PMT entry PID 264 (in program_number 1)
new PMT entry PID 1500 (in program_number 1)
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceInsert {
    splice_event_id: 1,
    reserved: 127,
    splice_detail: Insert {
        network_indicator: In,
        splice_mode: Program(
            Immediate
        ),
        duration: Some(
            SpliceDuration {
                return_mode: Manual,
                duration: 10800000
            }
        ),
        unique_program_id: 1,
        avail_num: 0,
        avails_expected: 0
    }
}
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 0 } SpliceNull
```

## The `section` subcommand

Dump a base64-encoded section string passed as a command-line argument

```
$ scte35dump section --base64 "/DAlAAAAAAAAAP/wFAUAAAABf+/+LRQrAP4BI9MIAAEBAQAAfxV6SQ=="
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 4095 } SpliceInsert {
    splice_event_id: 1,
    reserved: 127,
    splice_detail: Insert {
        network_indicator: Out,
        splice_mode: Program(
            Timed(
                Some(
                    756296448
                )
            )
        ),
        duration: Some(
            SpliceDuration {
                return_mode: Automatic,
                duration: 19125000
            }
        ),
        unique_program_id: 1,
        avail_num: 1,
        avails_expected: 1
    }
}
```

Dump a hexidecimal-encoded section string passed as a command-line argument

```
$ scte35dump section --hex "fc302500000000000000fff01405000000017feffe2d142b00fe0123d3080001010100007f157a49"
SpliceInfoHeader { protocol_version: 0, encrypted_packet: false, encryption_algorithm: None, pts_adjustment: 0, cw_index: 0, tier: 4095 } SpliceInsert {
    splice_event_id: 1,
    reserved: 127,
    splice_detail: Insert {
        network_indicator: Out,
        splice_mode: Program(
            Timed(
                Some(
                    756296448
                )
            )
        ),
        duration: Some(
            SpliceDuration {
                return_mode: Automatic,
                duration: 19125000
            }
        ),
        unique_program_id: 1,
        avail_num: 1,
        avails_expected: 1
    }
}
```
