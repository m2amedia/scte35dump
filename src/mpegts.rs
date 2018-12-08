use mpeg2ts_reader::demultiplex;
use mpeg2ts_reader::descriptor;
use mpeg2ts_reader::packet;
use mpeg2ts_reader::psi;
use mpeg2ts_reader::StreamType;
use scte35_reader;

pub struct DumpSpliceInfoProcessor;
impl scte35_reader::SpliceInfoProcessor for DumpSpliceInfoProcessor {
    fn process(
        &self,
        header: scte35_reader::SpliceInfoHeader,
        command: scte35_reader::SpliceCommand,
        descriptors: scte35_reader::SpliceDescriptorIter,
    ) {
        println!("{:?} {:#?}", header, command);
        for d in descriptors {
            println!("  {:?}", d);
        }
    }
}

pub struct Scte35StreamConsumer {
    section: psi::SectionPacketConsumer<
        scte35_reader::Scte35SectionProcessor<DumpSpliceInfoProcessor, DumpDemuxContext>,
    >,
}
impl Default for Scte35StreamConsumer {
    fn default() -> Self {
        let parser = scte35_reader::Scte35SectionProcessor::new(DumpSpliceInfoProcessor);
        Scte35StreamConsumer {
            section: psi::SectionPacketConsumer::new(parser),
        }
    }
}

/// Check for registration descriptor per SCTE-35, section 8.1
fn is_scte35(pmt: &psi::pmt::PmtSection) -> bool {
    for d in pmt.descriptors() {
        if let Ok(descriptor::CoreDescriptors::Registration(
            descriptor::registration::RegistrationDescriptor { buf: b"CUEI" },
        )) = d
        {
            return true;
        }
    }
    false
}

impl Scte35StreamConsumer {
    fn construct(
        program_pid: packet::Pid,
        pmt: &psi::pmt::PmtSection,
        stream_info: &psi::pmt::StreamInfo,
    ) -> DumpFilterSwitch {
        if is_scte35(pmt) {
            println!(
                "Program {:?}: Found SCTE-35 data on {:?} ({:#x})",
                program_pid,
                stream_info.elementary_pid(),
                u16::from(stream_info.elementary_pid())
            );
            DumpFilterSwitch::Scte35(Scte35StreamConsumer::default())
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
    fn consume(&mut self, ctx: &mut Self::Ctx, pk: &packet::Packet) {
        self.section.consume(ctx, pk);
    }
}

packet_filter_switch!{
    DumpFilterSwitch<DumpDemuxContext> {
        Pat: demultiplex::PatPacketFilter<DumpDemuxContext>,
        Pmt: demultiplex::PmtPacketFilter<DumpDemuxContext>,
        Null: demultiplex::NullPacketFilter<DumpDemuxContext>,
        Scte35: Scte35StreamConsumer,
    }
}
demux_context!(DumpDemuxContext, DumpStreamConstructor);

pub struct DumpStreamConstructor;
impl demultiplex::StreamConstructor for DumpStreamConstructor {
    type F = DumpFilterSwitch;

    fn construct(&mut self, req: demultiplex::FilterRequest) -> Self::F {
        match req {
            demultiplex::FilterRequest::ByPid(packet::Pid::PAT) => {
                DumpFilterSwitch::Pat(demultiplex::PatPacketFilter::default())
            }
            demultiplex::FilterRequest::ByPid(_) => {
                DumpFilterSwitch::Null(demultiplex::NullPacketFilter::default())
            }
            demultiplex::FilterRequest::ByStream {
                program_pid,
                stream_type: StreamType::Private(0x86),
                pmt,
                stream_info,
            } => Scte35StreamConsumer::construct(program_pid, pmt, stream_info),
            demultiplex::FilterRequest::ByStream { .. } => {
                DumpFilterSwitch::Null(demultiplex::NullPacketFilter::default())
            }
            demultiplex::FilterRequest::Pmt {
                pid,
                program_number,
            } => DumpFilterSwitch::Pmt(demultiplex::PmtPacketFilter::new(pid, program_number)),
            demultiplex::FilterRequest::Nit { .. } => {
                DumpFilterSwitch::Null(demultiplex::NullPacketFilter::default())
            }
        }
    }
}
