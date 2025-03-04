use midly::Smf;
use nih_plug::{context, midi, prelude::*};
use std::sync::Arc;

// This is a shortened version of the gain example with most comments removed, check out
// https://github.com/robbert-vdh/nih-plug/blob/master/plugins/examples/gain/src/lib.rs to get
// started

struct MmlPlugin {
    params: Arc<MmlPluginParams>,
    midi_handler: MidiHandler,
}

#[derive(Params, Default)]
struct MmlPluginParams {
    compiler_path: String,
    source_path: String,
    smf_path: Option<String>,
}

impl Default for MmlPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(MmlPluginParams::default()),
            midi_handler: MidiHandler {
                ticks_per_quarter: 0,
                midi_events: Vec::new(),
                note_states: [false; 128],
                current_tempo: 120,
            },
        }
    }
}

struct TimedEvent {
    time: u64,
    event: midly::TrackEvent<'static>,
}

struct MidiHandler {
    ticks_per_quarter: u16,
    midi_events: Vec<Option<TimedEvent>>,

    current_tempo: u32,
    note_states: [bool; 128],
}

impl MmlPlugin {
    fn init(&mut self) -> anyhow::Result<()> {
        // let smf_bytes = Box::new(std::fs::read(self.params.smf_path.as_ref().unwrap())?);
        let smf_bytes = Box::new(std::fs::read(
            "/Users/anosatsuk124/Work/music-scratch/2025/02/27/02/dists/main.mid",
        )?);
        let smf = Smf::parse(&smf_bytes)?;
        let ticks_per_quarter = match smf.header.timing {
            midly::Timing::Metrical(t) => t.as_int(),
            _ => {
                panic!("非Metricalタイミングは未対応です");
            }
        };

        let mut midi_events = Vec::new();
        {
            for track in smf.tracks.iter() {
                let mut abs_time = 0u64;
                for event in track.iter() {
                    let event = event.to_static();
                    abs_time += event.delta.as_int() as u64;
                    midi_events.push(Some(TimedEvent {
                        time: abs_time,
                        event,
                    }));
                }
            }
            midi_events.sort_by_key(|e| e.as_ref().unwrap().time);
        }

        let midi_handler = MidiHandler {
            ticks_per_quarter,
            midi_events,
            current_tempo: 120,
            note_states: [false; 128],
        };

        self.midi_handler = midi_handler;

        Ok(())
    }

    fn _process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let midi_handler = &mut self.midi_handler;

        let sample_rate = _context.transport().sample_rate;
        let is_playing = _context.transport().playing;
        let current_tempo = midi_handler.current_tempo as f64;
        let pos_seconds = _context.transport().pos_seconds().unwrap_or(0f64);

        if is_playing {
            for _ in 0..buffer.samples() {
                nih_log!("pos_seconds: {}", pos_seconds);
                for event in midi_handler.midi_events.iter_mut() {
                    let event_inner = match event {
                        Some(e) => e,
                        None => continue,
                    };

                    let event_delta_seconds = (event_inner.time as f64
                        / midi_handler.ticks_per_quarter as f64)
                        * (60f64 / current_tempo);
                    if pos_seconds > event_delta_seconds {
                        let timing_offset =
                            ((pos_seconds - event_delta_seconds) * sample_rate as f64) as u32;
                        match event_inner.event.kind {
                            midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(t)) => {
                                midi_handler.current_tempo = 60_000_000 / t.as_int();
                            }
                            midly::TrackEventKind::Midi { channel, message } => match message {
                                midly::MidiMessage::NoteOn { key, vel } => {
                                    if midi_handler.note_states[key.as_int() as usize] {
                                        continue;
                                    }

                                    midi_handler.note_states[key.as_int() as usize] = true;

                                    let note_on = NoteEvent::NoteOn {
                                        timing: timing_offset, // NOTE: All of the timings are sample offsets within the current buffer.
                                        voice_id: None,
                                        channel: channel.as_int(),
                                        note: key.as_int(),
                                        velocity: (vel.as_int() as f32) / 127f32,
                                    };
                                    _context.send_event(note_on);
                                }
                                midly::MidiMessage::NoteOff { key, vel } => {
                                    midi_handler.note_states[key.as_int() as usize] = false;
                                    let note_off = NoteEvent::NoteOff {
                                        timing: timing_offset,
                                        voice_id: None,
                                        channel: channel.as_int(),
                                        note: key.as_int(),
                                        velocity: (vel.as_int() as f32) / 127f32,
                                    };
                                    _context.send_event(note_off);
                                }
                                midly::MidiMessage::PitchBend { bend } => {
                                    let pitch_bend = NoteEvent::MidiPitchBend {
                                        timing: timing_offset,
                                        channel: channel.as_int(),
                                        value: bend.as_int() as f32 / 0x3FFF as f32,
                                    };
                                    _context.send_event(pitch_bend);
                                }
                                _ => {}
                            },

                            _ => {}
                        }
                        *event = None;
                    }
                }
            }
        }
        ProcessStatus::Normal
    }
}

impl Plugin for MmlPlugin {
    const NAME: &'static str = "Mml Plugin";
    const VENDOR: &'static str = "Satsuki Akiba";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "anosatsuk124@gmail.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),

        aux_input_ports: &[],
        aux_output_ports: &[],

        // Individual ports and the layout as a whole can be named here. By default these names
        // are generated as needed. This layout will be called 'Stereo', while a layout with
        // only one input and output channel would be called 'Mono'.
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::Basic;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    // If the plugin can send or receive SysEx messages, it can define a type to wrap around those
    // messages here. The type implements the `SysExMessage` trait, which allows conversion to and
    // from plain byte buffers.
    type SysExMessage = ();
    // More advanced plugins can use this to run expensive background tasks. See the field's
    // documentation for more information. `()` means that the plugin does not have any background
    // tasks.
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.

        if self.init().is_err() {
            return false;
        }

        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
        self.midi_handler.note_states = [false; 128];
        let _ = self.init();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self._process(buffer, _aux, _context)
    }
}

impl ClapPlugin for MmlPlugin {
    const CLAP_ID: &'static str = "io.anosatsuk124.mml-plugin";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A short description of your plugin");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    // Don't forget to change these features
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
}

impl Vst3Plugin for MmlPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"Exactly16Chars!!";

    // And also don't forget to change these categories
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nih_export_clap!(MmlPlugin);
nih_export_vst3!(MmlPlugin);
