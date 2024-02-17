use mpeg2ts_reader::demultiplex;
use mpeg2ts_reader::packet;
use mpeg2ts_reader::packet::Pid;
use mpeg2ts_reader::psi;

use std::cell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct DumpSpliceInfoProcessor {
    pub elementary_pid: Option<Pid>,
    pub last_pcr: Rc<cell::Cell<Option<packet::ClockRef>>>,
}
impl scte35_reader::SpliceInfoProcessor for DumpSpliceInfoProcessor {
    fn process(
        &self,
        header: scte35_reader::SpliceInfoHeader<'_>,
        command: scte35_reader::SpliceCommand,
        descriptors: scte35_reader::SpliceDescriptors<'_>,
    ) {
        if let Some(elementary_pid) = self.elementary_pid {
            print!("{:?} ", elementary_pid);
        }
        if let Some(pcr) = self.last_pcr.as_ref().get() {
            print!("Last {:?}: ", pcr)
        }
        print!("{:?} {:#?}", header, command);
        if let scte35_reader::SpliceCommand::SpliceInsert {
            splice_detail:
                scte35_reader::SpliceInsert::Insert {
                    splice_mode:
                        scte35_reader::SpliceMode::Program(scte35_reader::SpliceTime::Timed(Some(time))),
                    ..
                },
            ..
        } = command
        {
            let time_ref = mpeg2ts_reader::packet::ClockRef::from_parts(time, 0);
            if let Some(pcr) = self.last_pcr.as_ref().get() {
                let mut diff = time_ref.base() as i64 - pcr.base() as i64;
                if diff < 0 {
                    diff += (std::u64::MAX / 2) as i64;
                }
                print!(" {}ms after most recent PCR", diff / 90);
            }
        }
        println!();
        for d in &descriptors {
            println!(" - {:#?}", d);
        }
    }
}

pub struct Scte35StreamConsumer {
    section: psi::SectionPacketConsumer<
        psi::CompactSyntaxSectionProcessor<
            psi::BufferCompactSyntaxParser<
                scte35_reader::Scte35SectionProcessor<DumpSpliceInfoProcessor, DumpDemuxContext>,
            >,
        >,
    >,
}

impl Scte35StreamConsumer {
    fn new(elementary_pid: Pid, last_pcr: Rc<cell::Cell<Option<packet::ClockRef>>>) -> Self {
        let parser = scte35_reader::Scte35SectionProcessor::new(DumpSpliceInfoProcessor {
            elementary_pid: Some(elementary_pid),
            last_pcr,
        });
        Scte35StreamConsumer {
            section: psi::SectionPacketConsumer::new(psi::CompactSyntaxSectionProcessor::new(
                psi::BufferCompactSyntaxParser::new(parser),
            )),
        }
    }

    fn construct(
        last_pcr: Rc<cell::Cell<Option<packet::ClockRef>>>,
        program_pid: packet::Pid,
        pmt: &psi::pmt::PmtSection<'_>,
        stream_info: &psi::pmt::StreamInfo<'_>,
    ) -> DumpFilterSwitch {
        if scte35_reader::is_scte35(pmt) {
            println!(
                "Program {:?}: Found SCTE-35 data on {:?} ({:#x})",
                program_pid,
                stream_info.elementary_pid(),
                u16::from(stream_info.elementary_pid())
            );
            DumpFilterSwitch::Scte35(Scte35StreamConsumer::new(
                stream_info.elementary_pid(),
                last_pcr,
            ))
        } else {
            println!("Program {:?}: {:?} has type {:?}, but PMT lacks 'CUEI' registration_descriptor that would indicate SCTE-35 content",
                     program_pid,
                     stream_info.elementary_pid(),
                     stream_info.stream_type());
            DumpFilterSwitch::Null(demultiplex::NullPacketFilter::default())
        }
    }
}
impl demultiplex::PacketFilter for Scte35StreamConsumer {
    type Ctx = DumpDemuxContext;
    fn consume(&mut self, ctx: &mut Self::Ctx, pk: &packet::Packet<'_>) {
        self.section.consume(ctx, pk);
    }
}

pub struct PcrWatch(Rc<cell::Cell<Option<packet::ClockRef>>>);
impl demultiplex::PacketFilter for PcrWatch {
    type Ctx = DumpDemuxContext;
    fn consume(&mut self, _ctx: &mut Self::Ctx, pk: &packet::Packet<'_>) {
        if let Some(af) = pk.adaptation_field() {
            if let Ok(pcr) = af.pcr() {
                self.0.set(Some(pcr));
            }
        }
    }
}

mpeg2ts_reader::packet_filter_switch! {
    DumpFilterSwitch<DumpDemuxContext> {
        Pat: demultiplex::PatPacketFilter<DumpDemuxContext>,
        Pmt: demultiplex::PmtPacketFilter<DumpDemuxContext>,
        Null: demultiplex::NullPacketFilter<DumpDemuxContext>,
        Scte35: Scte35StreamConsumer,
        Pcr: PcrWatch,
    }
}
pub struct DumpDemuxContext {
    changeset: demultiplex::FilterChangeset<DumpFilterSwitch>,
    last_pcrs: HashMap<packet::Pid, Rc<cell::Cell<Option<packet::ClockRef>>>>,
}
impl DumpDemuxContext {
    pub fn new() -> Self {
        DumpDemuxContext {
            changeset: demultiplex::FilterChangeset::default(),
            last_pcrs: HashMap::new(),
        }
    }
    pub fn last_pcr(&self, program_pid: packet::Pid) -> Rc<cell::Cell<Option<packet::ClockRef>>> {
        self.last_pcrs
            .get(&program_pid)
            .expect("last_pcrs entry didn't exist on call to last_pcr()")
            .clone()
    }
}
impl demultiplex::DemuxContext for DumpDemuxContext {
    type F = DumpFilterSwitch;

    fn filter_changeset(&mut self) -> &mut demultiplex::FilterChangeset<Self::F> {
        &mut self.changeset
    }

    fn construct(&mut self, req: demultiplex::FilterRequest<'_, '_>) -> Self::F {
        match req {
            demultiplex::FilterRequest::ByPid(packet::Pid::PAT) => {
                DumpFilterSwitch::Pat(demultiplex::PatPacketFilter::default())
            }
            demultiplex::FilterRequest::ByPid(_) => {
                DumpFilterSwitch::Null(demultiplex::NullPacketFilter::default())
            }
            demultiplex::FilterRequest::ByStream {
                program_pid,
                stream_type: scte35_reader::SCTE35_STREAM_TYPE,
                pmt,
                stream_info,
            } => Scte35StreamConsumer::construct(
                self.last_pcr(program_pid),
                program_pid,
                pmt,
                stream_info,
            ),
            demultiplex::FilterRequest::ByStream { program_pid, .. } => {
                DumpFilterSwitch::Pcr(PcrWatch(self.last_pcr(program_pid)))
            }
            demultiplex::FilterRequest::Pmt {
                pid,
                program_number,
            } => {
                // prepare structure needed to print PCR values later on
                self.last_pcrs.insert(pid, Rc::new(cell::Cell::new(None)));
                DumpFilterSwitch::Pmt(demultiplex::PmtPacketFilter::new(pid, program_number))
            }
            demultiplex::FilterRequest::Nit { .. } => {
                DumpFilterSwitch::Null(demultiplex::NullPacketFilter::default())
            }
        }
    }
}
