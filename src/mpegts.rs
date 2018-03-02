use std::cell;
use mpeg2ts_reader::demultiplex;
use mpeg2ts_reader::packet;
use mpeg2ts_reader::psi;
use mpeg2ts_reader::StreamType;
use std::collections::HashMap;
use hexdump;
use bitreader;

#[derive(Debug)]
enum EncryptionAlgorithm {
    None,
    DesEcb,
    DesCbc,
    TripleDesEde3Ecb,
    Reserved(u8),
    Private(u8),
}
impl EncryptionAlgorithm {
    pub fn from_id(id: u8) -> EncryptionAlgorithm {
        match id {
            0 => EncryptionAlgorithm::None,
            1 => EncryptionAlgorithm::DesEcb,
            2 => EncryptionAlgorithm::DesCbc,
            3 => EncryptionAlgorithm::TripleDesEde3Ecb,
            _ => {
                if id < 32 {
                    EncryptionAlgorithm::Reserved(id)
                } else {
                    EncryptionAlgorithm::Private(id)
                }
            }
        }
    }
}

#[derive(Debug)]
enum SpliceCommandType {
    SpliceNull,
    Reserved(u8),
    SpliceSchedule,
    SpliceInsert,
    TimeSignal,
    BandwidthReservation,
    PrivateCommand,
}
impl SpliceCommandType {
    fn from_id(id: u8) -> SpliceCommandType {
        match id {
            0x00 => SpliceCommandType::SpliceNull,
            0x04 => SpliceCommandType::SpliceSchedule,
            0x05 => SpliceCommandType::SpliceInsert,
            0x06 => SpliceCommandType::TimeSignal,
            0x07 => SpliceCommandType::BandwidthReservation,
            0xff => SpliceCommandType::PrivateCommand,
            _ => SpliceCommandType::Reserved(id),
        }
    }
}

struct SpliceInfoHeader<'a> {
    buf: &'a[u8],
}
impl<'a> SpliceInfoHeader<'a> {
    const HEADER_LENGTH: usize = 11;

    fn new(buf: &'a[u8]) -> (SpliceInfoHeader<'a>, &'a[u8]) {
        if buf.len() < 11 {
            panic!("buffer too short: {} (expected 11)", buf.len());
        }
        let (head, tail) = buf.split_at(11);
        (SpliceInfoHeader { buf: head }, tail)
    }

    pub fn protocol_version(&self) -> u8 {
        self.buf[0]
    }
    pub fn encrypted_packet(&self) -> bool {
        self.buf[1] & 0b1000_0000 != 0
    }
    pub fn encryption_algorithm(&self) -> EncryptionAlgorithm {
        EncryptionAlgorithm::from_id((self.buf[1] & 0b0111_1110) >> 1)
    }
    pub fn pts_adjustment(&self) -> u64 {
        u64::from((self.buf[1] & 1)) << 32
        |u64::from(self.buf[2]) << 24
        |u64::from(self.buf[3]) << 16
        |u64::from(self.buf[4]) << 8
        |u64::from(self.buf[5])
    }
    pub fn cw_index(&self) -> u8 {
        self.buf[6]
    }
    pub fn tier(&self) -> u16 {
        u16::from(self.buf[7]) << 4
        |u16::from(self.buf[8]) >> 4
    }
    pub fn splice_command_length(&self) -> u16 {
        u16::from((self.buf[8] & 0b00001111)) << 8
        |u16::from(self.buf[9])
    }
    pub fn splice_command_type(&self) -> SpliceCommandType {
        SpliceCommandType::from_id(self.buf[10])
    }
}

#[derive(Debug)]
pub enum SpliceCommand {
    SpliceNull { },
    SpliceInsert {
        splice_event_id: u32,
        reserved: u8,
        splice_detail: SpliceInsert,
    }
}

#[derive(Debug)]
pub enum NetworkIndicator {
    Out,
    In
}
impl NetworkIndicator {
    /// panics if `id` is something other than `0` or `1`
    pub fn from_flag(id: u8) -> NetworkIndicator {
        match id {
            0 => NetworkIndicator::In,
            1 => NetworkIndicator::Out,
            _ => panic!("Invalid out_of_network_indicator value: {} (expected 0 or 1)", id),
        }
    }
}

#[derive(Debug)]
pub enum SpliceInsert {
    Cancel,
    Insert {
        network_indicator: NetworkIndicator,
        splice_mode: SpliceMode,
        duration: Option<SpliceDuration>,
        unique_program_id: u16,
        avail_num: u8,
        avails_expected: u8,
    }
}

#[derive(Debug)]
pub enum SpliceTime {
    Immediate,
    Timed(Option<u64>),
}

#[derive(Debug)]
pub struct ComponentSplice {
    component_tag: u8,
    splice_time: SpliceTime,
}

#[derive(Debug)]
pub enum SpliceMode {
    Program(SpliceTime),
    Components(Vec<ComponentSplice>)
}

#[derive(Debug)]
pub enum ReturnMode {
    Automatic,
    Manual,
}
impl ReturnMode {
    fn from_flag(flag: u8) -> ReturnMode {
        match flag {
            0 => ReturnMode::Manual,
            1 => ReturnMode::Automatic,
            _ => panic!("Invalid auto_return value: {} (expected 0 or 1)", flag),
        }
    }
}

#[derive(Debug)]
pub struct SpliceDuration {
    return_mode: ReturnMode,
    duration: u64,
}

pub struct Scte35SectionProcessor {

}
impl psi::SectionProcessor<demultiplex::FilterChangeset> for Scte35SectionProcessor {
    fn process(&mut self, header: &psi::SectionCommonHeader, section_data: &[u8]) -> Option<demultiplex::FilterChangeset> {
        if header.table_id == 0xfc {
            if section_data.len() < SpliceInfoHeader::HEADER_LENGTH + 4 {
                println!("section data too short: {} (must be at least {})", section_data.len(), SpliceInfoHeader::HEADER_LENGTH + 4);
                return None
            }
            // trim off the 32-bit CRC, TODO: check the CRC!  (possibly in calling code rather than here?)
            let section_data = &section_data[..section_data.len()-4];
            let (splice_header, rest) = SpliceInfoHeader::new(section_data);
            //println!("splice header len={}, type={:?}", splice_header.splice_command_length(), splice_header.splice_command_type());
            let (payload, descriptors) = rest.split_at(splice_header.splice_command_length() as usize); //FIXME: validate indexes
            let splice_command = match splice_header.splice_command_type() {
                SpliceCommandType::SpliceNull => Some(Self::splice_null(&splice_header, payload, descriptors)),
                SpliceCommandType::SpliceInsert => Some(Self::splice_insert(&splice_header, payload, descriptors)),
                _ => None,
            };
            if let Some(splice_command) = splice_command {
                println!("scte35: got command {:#?}", splice_command);
            } else {
                println!("scte35: unhandled command {:?}", splice_header.splice_command_type());
            }
        } else {
            println!("bad table_id for scte35: {:#x} (expected 0xfc)", header.table_id);
        }
        None
    }
}
impl Scte35SectionProcessor {
    // FIXME: get rid of all unwrap()
    fn splice_null(splice_header: &SpliceInfoHeader, payload: &[u8], descriptors: &[u8]) -> SpliceCommand {
        if payload.len() > 0 {
            hexdump::hexdump(&payload);
        }
        let descriptor_loop_length = u16::from(descriptors[0]) << 8 | u16::from(descriptors[1]);
        if descriptor_loop_length > 0 {
            println!("scte35: {} splice_descriptor", descriptor_loop_length);
            hexdump::hexdump(&descriptors[2..]);
        }
        SpliceCommand::SpliceNull { }
    }

    fn splice_insert(splice_header: &SpliceInfoHeader, payload: &[u8], descriptors: &[u8]) -> SpliceCommand {
        let mut r = bitreader::BitReader::new(payload);

        if payload.len() > 0 {
            hexdump::hexdump(&payload);
        }
        let descriptor_loop_length = u16::from(descriptors[0]) << 8 | u16::from(descriptors[1]);
        if descriptor_loop_length > 0 {
            println!("scte35: {} splice_descriptor", descriptor_loop_length);
            hexdump::hexdump(&descriptors[2..]);
        }
        let splice_event_id = r.read_u32(32).unwrap();
        let splice_event_cancel_indicator = r.read_bool().unwrap();
        let reserved = r.read_u8(7).unwrap();
        let result = SpliceCommand::SpliceInsert {
            splice_event_id,
            reserved,
            splice_detail: Self::read_splice_detail(&mut r, splice_event_cancel_indicator)
        };
        assert_eq!(r.position() as usize, payload.len()*8);
        result
    }

    fn read_splice_detail(r: &mut bitreader::BitReader, splice_event_cancel_indicator: bool) -> SpliceInsert {
        if splice_event_cancel_indicator {
            SpliceInsert::Cancel
        } else {
            let network_indicator = NetworkIndicator::from_flag(r.read_u8(1).unwrap());
            let program_splice_flag = r.read_bool().unwrap();
            let duration_flag = r.read_bool().unwrap();
            let splice_immediate_flag = r.read_bool().unwrap();
            r.skip(4).unwrap();  // reserved

            SpliceInsert::Insert {
                network_indicator,
                splice_mode: Self::read_splice_mode(r, program_splice_flag, splice_immediate_flag),
                duration: if duration_flag { Some(Self::read_duration(r)) } else { None },
                unique_program_id: r.read_u16(16).unwrap(),
                avail_num: r.read_u8(8).unwrap(),
                avails_expected: r.read_u8(8).unwrap(),
            }
        }
    }

    fn read_splice_mode(r: &mut bitreader::BitReader, program_splice_flag: bool, splice_immediate_flag: bool) -> SpliceMode {
        if program_splice_flag {
            let time = if splice_immediate_flag {
                SpliceTime::Immediate
            } else {
                SpliceTime::Timed(Self::read_splice_time(r))
            };
            SpliceMode::Program(time)
        } else {
            let component_count = r.read_u8(8).unwrap();
            let compomemts = (0..component_count).map(|_| {
                let component_tag = r.read_u8(8).unwrap();
                let splice_time = if splice_immediate_flag {
                    SpliceTime::Immediate
                } else {
                    SpliceTime::Timed(Self::read_splice_time(r))
                };
                ComponentSplice { component_tag, splice_time }
            }).collect();
            SpliceMode::Components(compomemts)
        }
    }

    fn read_splice_time(r: &mut bitreader::BitReader) -> Option<u64> {
        if r.read_bool().unwrap_or(false) {
            r.skip(6).unwrap();  // reserved
            r.read_u64(33).ok()
        } else {
            r.skip(7).unwrap();  // reserved
            None
        }
    }

    fn read_duration(r: &mut bitreader::BitReader) -> SpliceDuration {
        let return_mode = ReturnMode::from_flag(r.read_u8(1).unwrap());
        r.skip(6).unwrap();
        SpliceDuration {
            return_mode,
            duration: r.read_u64(33).unwrap(),
        }
    }
}

struct Scte35StreamConsumer {
    section: psi::SectionPacketConsumer,
}
impl Default for Scte35StreamConsumer {
    fn default() -> Self {
        Scte35StreamConsumer {
            section: psi::SectionPacketConsumer::new(Scte35SectionProcessor { })
        }
    }
}
impl Scte35StreamConsumer {
    fn construct(stream_info: &demultiplex::StreamInfo) -> Box<cell::RefCell<demultiplex::PacketFilter>> {
        //for d in stream_info.descriptors() {
        //    println!("scte35 descriptor {:?}", d);
        //}
        let consumer = Scte35StreamConsumer::default();
        Box::new(cell::RefCell::new(consumer))
    }
}
impl packet::PacketConsumer<demultiplex::FilterChangeset> for Scte35StreamConsumer {
    fn consume(&mut self, pk: packet::Packet) -> Option<demultiplex::FilterChangeset> {
        self.section.consume(pk);
        None
    }
}

pub fn create_demux() -> demultiplex::Demultiplex {
    let mut table: HashMap<StreamType, fn(&demultiplex::StreamInfo)->Box<cell::RefCell<demultiplex::PacketFilter>>>
    = HashMap::new();

    table.insert(StreamType::Private(0x86), Scte35StreamConsumer::construct);
    let ctor = demultiplex::StreamConstructor::new(demultiplex::NullPacketFilter::construct, table);
    demultiplex::Demultiplex::new(ctor)
}

