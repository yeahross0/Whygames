use crate::{
    err::WhyResult,
    music::{MusicMaker, TICKS_PER_BEAT},
    sys::just_give_me_str_path,
};
use itertools::Itertools;
use macroquad::logging as log;
use midly::Smf;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use rustysynth::{MidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io,
    path::Path,
    sync::{Arc, Mutex},
};
use tinyaudio::{run_output_device, BaseAudioOutputDevice, OutputDeviceParameters};

pub fn default_smf<'a>() -> Smf<'a> {
    let format = midly::Format::Parallel;
    let timing = midly::Timing::Metrical(TICKS_PER_BEAT.into());
    let header = midly::Header::new(format, timing);
    Smf::new(header)
}

pub struct MidiFileContainer {
    container: Arc<Mutex<Option<MidiFile>>>,
}

impl MidiFileContainer {
    pub fn new(midi_data: &Smf) -> WhyResult<MidiFileContainer> {
        let mut midi_buffer = Vec::new();
        midly::write(&midi_data.header, &midi_data.tracks, &mut midi_buffer)?;

        let mut cursor = io::Cursor::new(midi_buffer);

        Ok(MidiFileContainer {
            container: Arc::new(Mutex::new(Some(MidiFile::new(&mut cursor).unwrap()))),
        })
    }

    pub fn update(&mut self, midi_data: &Smf) -> WhyResult<()> {
        let mut midi_buffer = Vec::new();
        midly::write(&midi_data.header, &midi_data.tracks, &mut midi_buffer)?;

        let mut cursor = io::Cursor::new(midi_buffer);

        let midi_file = MidiFile::new(&mut cursor).unwrap();

        let container = self.container.clone();
        let mut container = container.lock().unwrap();
        *container = Some(midi_file);

        Ok(())
    }
}

pub struct AudioPlayer {
    pub sink_player: Option<SinkPlayer>,
    pub live_player: LivePlayer,
    pub record_player: RecordPlayer,
}

impl AudioPlayer {
    pub async fn init(params: AudioParameters, sf2_data: Vec<u8>) -> WhyResult<AudioPlayer> {
        let sink_player = match OutputStream::try_default() {
            Ok((_stream, stream_handle)) => {
                let sink_music = Sink::try_new(&stream_handle)?;
                log::debug!("WORKED HERE?");

                // TODO: !!!
                let mut temp_loaded_sounds = HashMap::new();
                let data = macroquad::file::load_file("sounds/WhaleShort.ogg")
                    .await
                    .unwrap();
                temp_loaded_sounds.insert("WhaleShort".to_owned(), data);
                let data = macroquad::file::load_file("sounds/Whale3.ogg")
                    .await
                    .unwrap();
                temp_loaded_sounds.insert("Whale3".to_owned(), data);

                Some(SinkPlayer {
                    music_sink: sink_music,
                    sfx_sinks: Vec::new(),
                    stream_handle,
                    _stream,
                    temp_loaded_sounds,
                })
            }
            Err(e) => {
                log::debug!("Failed to initialise rodio audio: {}", e);
                None
            }
        };

        // Load the SoundFont.
        let mut sf2_cursor = io::Cursor::new(sf2_data);
        let sound_font = Arc::new(SoundFont::new(&mut sf2_cursor)?);

        let buffer = || vec![0_f32; params.channel_sample_count];

        let settings = SynthesizerSettings::new(params.sample_rate as i32);

        let live_synth = Arc::new(Mutex::new(Synthesizer::new(&sound_font, &settings)?));
        let synth_clone = live_synth.clone();

        let sequencer = Arc::new(Mutex::new(Some(MidiFileSequencer::new(Synthesizer::new(
            &sound_font,
            &settings,
        )?))));
        let sequencer_clone = sequencer.clone();

        let mut live_left = buffer();
        let mut live_right = buffer();
        let mut record_left = buffer();
        let mut record_right = buffer();

        let live_device = run_output_device(params.to_output_device_params(1.0), {
            move |data| {
                synth_clone
                    .lock()
                    .unwrap()
                    .render(&mut live_left[..], &mut live_right[..]);

                for (i, value) in live_left.iter().interleave(live_right.iter()).enumerate() {
                    data[i] = *value;
                }
            }
        })
        .unwrap();

        let record_device = run_output_device(params.to_output_device_params(1.0), {
            move |data| {
                if let Some(s) = sequencer_clone.lock().unwrap().as_mut() {
                    s.render(&mut record_left[..], &mut record_right[..])
                }

                for (i, value) in record_left
                    .iter()
                    .interleave(record_right.iter())
                    .enumerate()
                {
                    data[i] = *value;
                }
            }
        })
        .unwrap();

        let live_player = LivePlayer {
            synthesizer: live_synth,
            _device: live_device,
        };

        let record_player = RecordPlayer {
            sequencer,
            params,
            _device: record_device,
        };

        Ok(AudioPlayer {
            sink_player,
            live_player,
            record_player,
        })
    }

    pub fn set_playback_rate(&mut self, playback_rate: f32) {
        // TODO: Probably better way than recreating output device every time...
        let sequencer_clone = self.record_player.sequencer.clone();
        let buffer = || vec![0_f32; self.record_player.params.channel_sample_count];

        let mut record_left = buffer();
        let mut record_right = buffer();
        self.record_player._device = run_output_device(
            self.record_player
                .params
                .to_output_device_params(playback_rate),
            {
                move |data| {
                    if let Some(s) = sequencer_clone.lock().unwrap().as_mut() {
                        s.render(&mut record_left[..], &mut record_right[..])
                    }

                    for (i, value) in record_left
                        .iter()
                        .interleave(record_right.iter())
                        .enumerate()
                    {
                        data[i] = *value;
                    }
                }
            },
        )
        .unwrap();
    }

    pub fn play_music(&mut self, music_data: Option<Vec<u8>>) -> WhyResult<()> {
        let cursor = music_data.map(io::Cursor::new);
        // TODO: Return err
        let source = cursor.map(|cursor| Decoder::new(cursor).unwrap());
        //sink_music.set_speed(1.0);
        if let Some(source) = source {
            if let Some(sink_player) = &mut self.sink_player {
                log::debug!("SONG APPENDED");
                //audio_player.music_sink.set_speed(1.0);
                sink_player.music_sink.clear();
                sink_player.music_sink.append(source);
                sink_player.music_sink.play();
            }
        }
        Ok(())
    }

    pub fn stop_music(&mut self) {
        if let Some(sink_player) = &mut self.sink_player {
            sink_player.music_sink.stop();
        }
    }

    pub fn play_note(&mut self, channel: u8, note: u8, velocity: u8) {
        self.live_player.synthesizer.lock().unwrap().note_on(
            channel as i32,
            note as i32,
            velocity as i32,
        );
    }

    pub fn stop_note(&mut self, channel: u8, note: u8) {
        self.live_player
            .synthesizer
            .lock()
            .unwrap()
            .note_off(channel as i32, note as i32);
    }

    pub fn stop_all_notes(&mut self) {
        self.live_player
            .synthesizer
            .lock()
            .unwrap()
            .note_off_all(false);
    }

    pub fn switch_to_maker_instrument(&mut self, music_maker: &MusicMaker) {
        let ins = music_maker.current_instrument().preset;
        let channel = music_maker.channel();
        self.switch_instrument(channel, ins);
        self.switch_record_instrument(channel, ins);
    }

    pub fn switch_instrument(&mut self, channel: u8, instrument: u8) {
        self.live_player
            .synthesizer
            .lock()
            .unwrap()
            .process_midi_message(channel as i32, 192, instrument as i32, 0);
    }

    pub fn play_record(&mut self, midi_file_container: &MidiFileContainer, is_looped: bool) {
        self.record_player
            .sequencer
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .play(&midi_file_container.container, is_looped)
    }

    pub fn pause_record(&mut self) {
        if let Some(seq_con) = self
            .record_player
            .sequencer
            .clone()
            .lock()
            .unwrap()
            .as_mut()
        {
            seq_con.pause();
        }
    }

    pub fn stop_record(&mut self) {
        if let Some(seq_con) = self
            .record_player
            .sequencer
            .clone()
            .lock()
            .unwrap()
            .as_mut()
        {
            seq_con.stop();
        }
    }

    pub fn has_midi(&self) -> bool {
        self.record_player
            .sequencer
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.has_midi())
            == Some(true)
    }

    pub fn display_position(&self) -> f64 {
        self.record_player
            .sequencer
            .clone()
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| {
                if s.end_of_sequence() {
                    0.0
                } else {
                    s.get_position()
                }
            })
            .unwrap_or(0.0)
    }

    pub fn switch_record_instrument(&mut self, channel: u8, instrument: u8) {
        if let Some(s) = self
            .record_player
            .sequencer
            .clone()
            .lock()
            .unwrap()
            .as_mut()
        {
            s.get_mut_synthesizer()
                .process_midi_message(channel as i32, 192, instrument as i32, 0);
        }
    }

    pub fn set_record_volume(&mut self, volume: f32) {
        if let Some(s) = self
            .record_player
            .sequencer
            .clone()
            .lock()
            .unwrap()
            .as_mut()
        {
            s.get_mut_synthesizer().set_master_volume(volume)
        }
    }

    pub fn stop_all_record_notes(&mut self) {
        if let Some(s) = self
            .record_player
            .sequencer
            .clone()
            .lock()
            .unwrap()
            .as_mut()
        {
            s.get_mut_synthesizer().note_off_all(false)
        }
    }

    pub fn set_record_speed(&mut self, tempo: u32) {
        if let Some(s) = self
            .record_player
            .sequencer
            .clone()
            .lock()
            .unwrap()
            .as_mut()
        {
            s.set_speed(tempo as f64 / 120.0)
        }
    }

    pub fn reset_message_index(&mut self) {
        if let Some(s) = self
            .record_player
            .sequencer
            .clone()
            .lock()
            .unwrap()
            .as_mut()
        {
            s.reset_message_index()
        }
    }
}

pub struct LivePlayer {
    synthesizer: Arc<Mutex<Synthesizer>>,
    _device: Box<dyn BaseAudioOutputDevice>,
}

pub struct RecordPlayer {
    sequencer: Arc<Mutex<Option<MidiFileSequencer>>>,
    params: AudioParameters,
    _device: Box<dyn BaseAudioOutputDevice>,
}

pub struct SinkPlayer {
    pub music_sink: Sink,
    pub sfx_sinks: Vec<Sink>,
    pub stream_handle: OutputStreamHandle,
    pub _stream: OutputStream,
    pub temp_loaded_sounds: HashMap<String, Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
pub struct AudioParameters {
    pub channels_count: usize,
    pub sample_rate: usize,
    pub channel_sample_count: usize,
}

impl AudioParameters {
    pub async fn load(path: impl AsRef<Path>) -> WhyResult<AudioParameters> {
        let file_contents =
            macroquad::file::load_string(just_give_me_str_path(path.as_ref())?).await?;

        Self::from_file_contents(&file_contents)
    }

    pub fn from_file_contents(file_contents: &str) -> WhyResult<AudioParameters> {
        Ok(serde_json::from_str(file_contents)?)
    }

    pub fn to_output_device_params(&self, playback_rate: f32) -> OutputDeviceParameters {
        OutputDeviceParameters {
            channels_count: self.channels_count,
            // Placeholder for pitch changes
            sample_rate: (self.sample_rate as f32 * playback_rate) as usize,
            channel_sample_count: self.channel_sample_count,
        }
    }
}
