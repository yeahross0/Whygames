use macroquad::logging as log;
use midly::{MetaMessage, MidiMessage, TrackEvent, TrackEventKind};
use std::collections::{HashMap, HashSet};

use crate::{
    art::{Sprite, SpriteSize},
    aud::{AudioPlayer, MidiFileContainer},
    drawer::{sheet_source_rect, DrawParams},
    edit::get_typed_variable,
    err::WhyResult,
    history::Event,
    inp::Input,
    maths::Vec2,
    meta::{Environment, MUSIC_MAKER_NAME},
    pixels,
    play::{self, is_position_in_sprite_sheet_image},
    rend::Image,
    time::TimeKeeping,
};

pub const TICKS_PER_BEAT: u16 = 960;
pub const POTENTIAL_NOTE_OFFSET: u8 = 7;
const INSTRUMENT_TRACK_COUNT: usize = 4;
const ALL_TRACK_COUNT: usize = 5;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MakerNote {
    pub offset: u8,
    pub pitch: u8,
    pub length: u8,
}

impl Default for MakerNote {
    fn default() -> Self {
        MakerNote {
            offset: 0,
            pitch: 0,
            length: 1,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlayingPitch {
    pub offset: u8,
    pub pitch: u8,
    pub length: u8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelativeNote {
    pub offset: i32,
    pub pitch: u8,
    pub length: u8,
}

pub fn make_track<'b>(music_maker: &mut MusicMaker, track_index: TrackIndex) -> midly::Track<'b> {
    let temp = music_maker.editing_position.track_index;

    music_maker.editing_position.track_index = track_index;

    let mut track = vec![];

    if music_maker.channel() == 0 {
        track.push(TrackEvent {
            delta: 0.into(),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(500000.into())),
        });
    }

    let ins = music_maker.current_instrument().preset;

    log::debug!("tr: {:?}, INS: {}", track_index, ins);
    track.push(TrackEvent {
        delta: 0.into(),
        kind: TrackEventKind::Midi {
            channel: music_maker.channel().into(),
            message: MidiMessage::ProgramChange {
                program: ins.into(),
            },
        },
    });

    let mut last: Option<(usize, u8, u8, u8)> = None;
    let vel = 127;
    let mut current_point = 0;
    music_maker.notes_mut().sort();
    for note in music_maker.notes().iter() {
        if note.offset > 32 {
            continue;
        }

        let adjusted_note = note.pitch + music_maker.current_instrument().lowest_note;

        if let Some((last_x, last_key, last_vel, last_length)) = last {
            let delta = (note.offset as u32 - last_x as u32) * (TICKS_PER_BEAT as u32 / 4);
            let diff = delta - delta.min(TICKS_PER_BEAT as u32 / 4 * last_length as u32);
            let delta = delta.min(TICKS_PER_BEAT as u32 / 4 * last_length as u32);
            track.push(TrackEvent {
                delta: delta.into(),
                kind: TrackEventKind::Midi {
                    channel: music_maker.channel().into(),
                    message: MidiMessage::NoteOff {
                        key: last_key.into(),
                        vel: last_vel.into(),
                    },
                },
            });
            track.push(TrackEvent {
                delta: diff.into(),
                kind: TrackEventKind::Midi {
                    channel: music_maker.channel().into(),
                    message: MidiMessage::NoteOn {
                        key: adjusted_note.into(),
                        vel: vel.into(),
                    },
                },
            });
            current_point += delta + diff;
        } else {
            // TODO: delta isn't always 0
            let delta = note.offset as u32 * (TICKS_PER_BEAT as u32 / 4);
            track.push(TrackEvent {
                delta: delta.into(),
                kind: TrackEventKind::Midi {
                    channel: music_maker.channel().into(),
                    message: MidiMessage::NoteOn {
                        key: adjusted_note.into(),
                        vel: vel.into(),
                    },
                },
            });
            current_point += delta;
        }
        last = Some((note.offset as usize, adjusted_note, vel, note.length));
    }

    // TODO: delta offs
    let x = 32;
    if let Some((last_x, last_key, last_vel, last_length)) = last {
        let delta = (x as u32 - last_x as u32) * (TICKS_PER_BEAT as u32 / 4);
        let delta = delta.min(TICKS_PER_BEAT as u32 / 4 * last_length as u32);
        track.push(TrackEvent {
            delta: delta.into(),
            kind: TrackEventKind::Midi {
                channel: music_maker.channel().into(),
                message: MidiMessage::NoteOff {
                    key: last_key.into(),
                    vel: last_vel.into(),
                },
            },
        });

        current_point += delta;
    }

    let measures_in_phrase = music_maker.measures_in_phrase();
    let delta = TICKS_PER_BEAT as u32 * measures_in_phrase - current_point;
    track.push(TrackEvent {
        delta: delta.into(),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });

    music_maker.editing_position.track_index = temp;

    track
}

#[derive(Debug, Copy, Clone)]
pub enum TrackIndex {
    Instrument(usize),
    Drums,
}

impl TrackIndex {
    pub fn simple_index(self) -> usize {
        match self {
            TrackIndex::Instrument(i) => i,
            TrackIndex::Drums => 4,
        }
    }

    pub fn all() -> [TrackIndex; ALL_TRACK_COUNT] {
        [
            TrackIndex::Instrument(0),
            TrackIndex::Instrument(1),
            TrackIndex::Instrument(2),
            TrackIndex::Instrument(3),
            TrackIndex::Drums,
        ]
    }

    fn go_to_next_track(&mut self) {
        *self = match self {
            TrackIndex::Instrument(index) => {
                *index += 1;
                if *index < INSTRUMENT_TRACK_COUNT {
                    TrackIndex::Instrument(*index)
                } else {
                    // TODO: TEMP UNTIL DRUMS
                    TrackIndex::Instrument(0)
                    //TrackIndex::Drums
                }
            }
            TrackIndex::Drums => TrackIndex::Instrument(0),
        }
    }

    fn go_to_previous_track(&mut self) {
        *self = match self {
            TrackIndex::Instrument(index) => {
                if *index > 0 {
                    *index -= 1;
                    TrackIndex::Instrument(*index)
                } else {
                    // TODO: TEMP UNTIL DRUMS
                    TrackIndex::Instrument(INSTRUMENT_TRACK_COUNT - 1)
                    //TrackIndex::Drums
                }
            }
            TrackIndex::Drums => TrackIndex::Instrument(INSTRUMENT_TRACK_COUNT - 1),
        }
    }

    fn channel(self) -> u8 {
        match self {
            TrackIndex::Instrument(i) => i as u8,
            TrackIndex::Drums => 10,
        }
    }
}

impl Default for TrackIndex {
    fn default() -> TrackIndex {
        TrackIndex::Instrument(0)
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct PointInMusic {
    pub phrase_index: usize,
    pub track_index: TrackIndex,
    pub page: u32,
}

#[derive(Debug, Default, PartialEq)]
pub enum KeyCount {
    #[default]
    Normal = 25,
    Extended = 37,
}

#[derive(Debug, Default, PartialEq)]
pub enum TimeSignature {
    #[default]
    FourFour,
    ThreeFour,
}

#[derive(Debug, Default)]
pub struct MakerPhrase {
    pub tracks: [MakerTrack; INSTRUMENT_TRACK_COUNT],
    pub drums: MakerTrack,
    pub key_count: KeyCount,
    pub time_signature: TimeSignature,
}

impl MakerPhrase {
    fn is_extended(&self) -> bool {
        self.key_count == KeyCount::Extended
    }

    fn is_alternative(&self) -> bool {
        self.time_signature == TimeSignature::ThreeFour
    }
}

#[derive(Debug, Default)]
pub struct MakerTrack {
    pub notes: Vec<MakerNote>,
    pub instrument_index: usize,
    //channel: u8,
}

#[derive(Debug, Default)]
struct DrumTrack {
    drums: Vec<MakerNote>,
    instrument_index: usize,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Playback {
    Solo,
    Multi,
}

#[derive(Debug, Default)]
pub struct Tracker {
    pub end_time_of_played_note: Option<f64>,
    pub last_piano_key: Option<u8>,
    pub last_preview_note: MakerNote,
    pub has_valid_intended_note: bool,
    pub has_preview_note: bool,
    pub last_maker_key: u8,
}

#[derive(Debug)]
pub struct MusicMaker {
    pub actions: HashSet<Action>,
    //notes: Vec<MakerNote>,
    pub instruments: Vec<Instrument>,
    //instrument_index: usize,
    pub intended_note: MakerNote,
    pub tempo: u32,
    //channel: u8,
    pub editing_position: PointInMusic,
    pub phrases: Vec<MakerPhrase>,
    pub playback: Playback,
    pub tracker: Tracker,
}

impl MusicMaker {
    pub fn init() -> MusicMaker {
        MusicMaker {
            actions: HashSet::new(),
            instruments: default_instruments(),
            intended_note: MakerNote::default(),
            tempo: 120,
            editing_position: PointInMusic::default(),
            phrases: vec![MakerPhrase::default()],
            playback: Playback::Solo,
            tracker: Tracker::default(),
        }
    }

    pub fn is_extended_keyboard(&self) -> bool {
        self.phrase().is_extended()
    }

    pub fn is_alternative_signature(&self) -> bool {
        self.phrase().is_alternative()
    }

    pub fn switch_to_extended_keyboard(&mut self) {
        self.phrase_mut().key_count = KeyCount::Extended
    }

    pub fn switch_to_standard_keyboard(&mut self) {
        self.phrase_mut().key_count = KeyCount::Normal
    }

    pub fn switch_to_alternative_signature(&mut self) {
        self.phrase_mut().time_signature = TimeSignature::ThreeFour
    }

    pub fn switch_to_standard_signature(&mut self) {
        self.phrase_mut().time_signature = TimeSignature::FourFour
    }

    pub fn instrument_index(&self) -> usize {
        self.track().instrument_index
    }

    pub fn go_to_next_instrument(&mut self) {
        self.track_mut().instrument_index += 1;
        self.track_mut().instrument_index %= self.instruments.len();
    }

    pub fn go_to_previous_instrument(&mut self) {
        if self.track().instrument_index == 0 {
            self.track_mut().instrument_index = self.instruments.len() - 1;
        } else {
            self.track_mut().instrument_index -= 1;
        }
    }

    pub fn current_instrument(&self) -> &Instrument {
        &self.instruments[self.instrument_index()]
    }

    pub fn current_instrument_name(&self) -> &str {
        &self.instruments[self.instrument_index()].name
    }

    pub fn note_height(&self) -> i32 {
        // TODO: Drums
        if self.phrase().is_extended() {
            4
        } else {
            6
        }
    }

    pub fn note_count(&self) -> i32 {
        // TODO: Drums
        if self.phrase().is_extended() {
            37
        } else {
            25
        }
    }

    pub fn note_adjust(&self) -> u8 {
        if self.phrase().is_extended() {
            0
        } else {
            POTENTIAL_NOTE_OFFSET
        }
    }

    pub fn max_offset(&self) -> u8 {
        if self.phrase().is_alternative() {
            12
        } else {
            16
        }
    }

    pub fn offset(&self) -> u8 {
        if self.editing_position.page == 1 {
            self.max_offset()
        } else {
            0
        }
    }

    pub fn measures_in_phrase(&self) -> u32 {
        8
    }

    pub fn is_note_possible(&self, x: u8) -> bool {
        self.notes().iter().all(|n| {
            n.offset > x + self.intended_note.length - 1
                || n.offset + n.length - 1 < x
                || n.offset == x
        })
    }

    pub fn remove_notes_at(&mut self, x: u8) {
        self.notes_mut().retain(|n| n.offset != x);
    }

    pub fn find_note_at(&self, x: u8, pitch: u8) -> Option<MakerNote> {
        self.notes()
            .iter()
            .find(|n| {
                // TODO:
                !(x < n.offset || x >= n.offset + n.length || n.pitch != pitch)
            })
            .copied()
    }

    pub fn phrase(&self) -> &MakerPhrase {
        &self.phrases[self.editing_position.phrase_index]
    }

    pub fn phrase_mut(&mut self) -> &mut MakerPhrase {
        &mut self.phrases[self.editing_position.phrase_index]
    }

    pub fn track(&self) -> &MakerTrack {
        match self.editing_position.track_index {
            TrackIndex::Instrument(index) => &self.phrase().tracks[index],
            TrackIndex::Drums => &self.phrase().drums,
        }
    }

    pub fn notes(&self) -> &Vec<MakerNote> {
        &self.track().notes
    }

    pub fn track_mut(&mut self) -> &mut MakerTrack {
        match self.editing_position.track_index {
            TrackIndex::Instrument(index) => &mut self.phrase_mut().tracks[index],
            TrackIndex::Drums => &mut self.phrase_mut().drums,
        }
    }

    pub fn notes_mut(&mut self) -> &mut Vec<MakerNote> {
        &mut self.track_mut().notes
    }

    pub fn channel(&self) -> u8 {
        self.editing_position.track_index.channel()
    }

    pub fn go_to_next_track(&mut self) {
        self.editing_position.track_index.go_to_next_track();
    }

    pub fn go_to_previous_track(&mut self) {
        self.editing_position.track_index.go_to_previous_track();
    }

    pub fn is_solo_playback(&self) -> bool {
        self.playback == Playback::Solo
    }

    pub fn refresh_song(&mut self) {
        self.actions.insert(Action::RefreshSong);
    }

    pub fn update_note_length(&mut self, environment: &mut Environment, mouse_scroll: f32) {
        let update_len = |context: &mut HashMap<String, String>, note_length: u8| {
            context.insert("Note Length".to_string(), note_length.to_string())
        };
        if let Some(length) = get_typed_variable::<u8>(&environment.context, "Note Length") {
            let new_length = length.min(16).max(1);
            if self.intended_note.length != new_length {
                self.intended_note.length = new_length;
                update_len(&mut environment.context, self.intended_note.length);
            } else if new_length != length {
                update_len(&mut environment.context, self.intended_note.length);
            }
        }

        if mouse_scroll > 0.0 {
            self.intended_note.length = (self.intended_note.length + 1).min(16);
            update_len(&mut environment.context, self.intended_note.length);
        } else if mouse_scroll < 0.0 {
            self.intended_note.length = (self.intended_note.length - 1).max(1);
            update_len(&mut environment.context, self.intended_note.length);
        }
    }

    pub fn update_keyboard(&mut self, environment: &Environment, events_to_apply: &mut Vec<Event>) {
        let new_value = environment.context["Keyboard"] == "Extended";

        if !self.is_extended_keyboard() && new_value {
            events_to_apply.push(Event::SwitchToExtendedKeyboard {
                editing_position: self.editing_position,
                old_notes: self.notes().clone(),
            });
            self.switch_to_extended_keyboard();
        }

        if self.is_extended_keyboard() && !new_value {
            events_to_apply.push(Event::SwitchToStandardKeyboard {
                editing_position: self.editing_position,
                old_notes: self.notes().clone(),
            });
            self.switch_to_standard_keyboard();
        }
    }

    pub fn update_signature(
        &mut self,
        environment: &Environment,
        events_to_apply: &mut Vec<Event>,
    ) {
        // TODO:
        let new_value = environment.context["Signature"] == "3/4";

        if !self.is_alternative_signature() && new_value {
            events_to_apply.push(Event::SwitchToAlternativeSignature {
                editing_position: self.editing_position,
                old_notes: self.notes().clone(),
            });
            self.switch_to_alternative_signature();
        }

        if self.is_alternative_signature() && !new_value {
            events_to_apply.push(Event::SwitchToStandardSignature {
                editing_position: self.editing_position,
                old_notes: self.notes().clone(),
            });
            self.switch_to_standard_signature();
        }
    }

    pub fn try_to_place_note(
        &mut self,
        environment: &Environment,
        input: &Input,
        audio_player: &mut AudioPlayer,
        time_keeping: TimeKeeping,
        events_to_apply: &mut Vec<Event>,
    ) {
        let lowest_note = self.current_instrument().lowest_note;

        let note_height = self.note_height();
        let note_count = self.note_count();
        let note_adjust = self.note_adjust();
        let max_offset_here = self.max_offset() as usize;

        let y = ((216 - input.outer.position.y) - 40) / note_height;
        let note = y.max(0).min(note_count - 1) as u8 + note_adjust;

        let adjusted_note = note + lowest_note;

        let x = {
            let music_maker_offset = self.offset() as usize;

            let x = input.outer.position.x - 64;
            if x >= 0 {
                let x = x as usize;
                let x = x / 16;
                if x < max_offset_here && y >= 0 && y < self.note_count() {
                    Some(x + music_maker_offset)
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(x) = x {
            self.tracker.has_valid_intended_note = true;
            self.intended_note.offset = x as u8;

            let is_possible = self.is_note_possible(x as u8);

            if environment.context["Music Mode"] == "Add"
                && (input.outer.left_button.is_pressed()
                    || (input.outer.left_button.is_down()
                        && self.intended_note != self.tracker.last_preview_note))
            {
                self.tracker.last_preview_note = self.intended_note;
                self.tracker.has_preview_note = true;

                if is_possible {
                    audio_player.stop_note(self.channel(), self.tracker.last_maker_key);
                    log::debug!(
                        "OLD NOTE: {}, adj {}",
                        self.tracker.last_maker_key,
                        adjusted_note
                    );
                    self.tracker.last_maker_key = adjusted_note;

                    self.tracker.end_time_of_played_note =
                        Some(time_keeping.total_elapsed + self.intended_note.length as f64 * 0.125);

                    log::debug!("ADJUSTED: {}", adjusted_note);

                    audio_player.play_note(self.channel(), adjusted_note, 127);
                }
            }

            if environment.context["Music Mode"] == "Add" && input.outer.left_button.is_released() {
                //let x = game.members[1].position.x.floor() - 72.0;
                let is_possible = self.is_note_possible(x as u8);
                if is_possible {
                    self.remove_notes_at(x as u8);
                    events_to_apply.push(Event::AddNote {
                        editing_position: self.editing_position,
                        note: MakerNote {
                            pitch: note,
                            length: self.intended_note.length,
                            offset: x as u8,
                        },
                    });
                    log::debug!("{}, NOTES: {:?}", note, self.notes());
                }
            }

            if (environment.context["Music Mode"] == "Remove" && input.outer.left_button.is_down())
                || input.outer.right_button.is_down()
            {
                if let Some(note) = self.find_note_at(x as u8, note) {
                    events_to_apply.push(Event::RemoveNote {
                        editing_position: self.editing_position,
                        note,
                    });
                }
                self.tracker.has_preview_note = false;

                //log::debug!("{}, NOTES: {:?}", note, notes);
            }
        }
    }

    pub fn should_note_be_stopped(&self, time_keeping: TimeKeeping) -> bool {
        if let Some(end_time) = self.tracker.end_time_of_played_note {
            time_keeping.total_elapsed > end_time
        } else {
            false
        }
    }

    pub fn stop_last_maker_key(&mut self, audio_player: &mut AudioPlayer) {
        audio_player.stop_note(self.channel(), self.tracker.last_maker_key);
        self.tracker.end_time_of_played_note = None;
    }

    pub fn is_mouse_hovering(game: &play::Game, mouse_position: pixels::Position) -> bool {
        let m_position = game.music_maker_member().unwrap().position;

        const MUSIC_MAKER_WIDTH: u32 = 256;
        const MUSIC_MAKER_HEIGHT: u32 = 150;
        let rect = pixels::Rect::from_centre(
            m_position.into(),
            pixels::Size::new(MUSIC_MAKER_WIDTH, MUSIC_MAKER_HEIGHT),
        );
        rect.contains_point(mouse_position)
    }

    pub fn update_tempo(&mut self, environment: &mut Environment, tempo: u32) {
        let update_tempo = |context: &mut HashMap<String, String>, tempo: u32| {
            context.insert("Tempo".to_string(), tempo.to_string())
        };
        let new_tempo = tempo.min(240).max(60);
        if self.tempo != new_tempo {
            self.tempo = new_tempo;
            self.refresh_song();
            update_tempo(&mut environment.context, self.tempo);
        } else if new_tempo != tempo {
            update_tempo(&mut environment.context, self.tempo);
        }
    }

    pub fn update_playback(&mut self, environment: &Environment) {
        if (environment.context["Playback"] == "Solo") != self.is_solo_playback() {
            self.refresh_song();
        }
        if environment.context["Playback"] == "Solo" {
            self.playback = Playback::Solo;
        } else {
            self.playback = Playback::Multi;
        }
    }

    pub fn handle_actions(
        &mut self,
        midi_file_container: &mut MidiFileContainer,
        smf: &mut midly::Smf,
        audio_player: &mut AudioPlayer,
        game: &play::Game,
        music_image: &Image,
        input: &Input,
    ) -> WhyResult<()> {
        if self.actions.remove(&Action::RefreshSong) {
            audio_player.stop_all_record_notes();
            audio_player.set_record_volume(0.5);
            audio_player.set_record_speed(self.tempo);

            smf.tracks = vec![];
            if self.is_solo_playback() {
                let trindex = self.editing_position.track_index;
                smf.tracks.push(make_track(self, trindex));
            } else {
                for i in TrackIndex::all() {
                    smf.tracks.push(make_track(self, i));
                }
            }

            midi_file_container.update(smf)?;

            audio_player.reset_message_index();
        }

        if self.actions.remove(&Action::NextTrack) {
            self.go_to_next_track();

            self.refresh_song();
        }

        if self.actions.remove(&Action::PreviousTrack) {
            self.go_to_previous_track();

            self.refresh_song();
        }

        if self.actions.remove(&Action::NextInstrument) {
            self.go_to_next_instrument();
            audio_player.switch_to_maker_instrument(self);

            self.refresh_song();
            audio_player.stop_all_record_notes();
        }

        if self.actions.remove(&Action::PreviousInstrument) {
            self.go_to_previous_instrument();
            audio_player.switch_to_maker_instrument(self);

            self.refresh_song();
            audio_player.stop_all_record_notes();
        }

        // TODO:
        if self.actions.remove(&Action::PlayPhrase) {
            log::debug!("PLAYING (MIDI)");

            smf.tracks = vec![];
            if self.is_solo_playback() {
                let trindex = self.editing_position.track_index;
                smf.tracks.push(make_track(self, trindex));
            } else {
                for i in TrackIndex::all() {
                    smf.tracks.push(make_track(self, i));
                }
            }

            log::debug!("{:?}", smf.tracks);

            midi_file_container.update(smf)?;

            audio_player.play_record(midi_file_container, true);

            audio_player.set_record_speed(self.tempo);
        }

        if self.actions.remove(&Action::PausePhrase) {
            // TODO: Pause method
            audio_player.stop_record();
        }

        if self.actions.remove(&Action::StopPhrase) {
            audio_player.stop_record();
        }

        for member in &game.members {
            if member.text.contents == MUSIC_MAKER_NAME {
                let rough_octave_span = rough_octave_span(self);
                let sprite_offset = sprite_offset(self);

                for octave in 0..rough_octave_span {
                    let octave_height = octave_height(self);
                    let rough_offset = rough_offset(octave, octave_height);
                    let note_positions = note_positions(self);

                    for (semitone, ((x, y), sprite_index)) in note_positions.into_iter().enumerate()
                    {
                        let y_offset = y + rough_offset;
                        if is_outside_edge(self, octave, semitone, y_offset) {
                            continue;
                        }
                        let sprite_index = sprite_index + sprite_offset;
                        let y_offset = y_offset - 9.0;
                        let sprite = Sprite {
                            index: sprite_index,
                            size: SpriteSize::Square(32),
                        };
                        let source = sheet_source_rect(sprite);
                        let _params = DrawParams {
                            source: Some(source),
                            ..Default::default()
                        };

                        //log::debug!("{:?}", source);

                        let touched_key = octave as u8 * 12 + semitone as u8;
                        let is_hit = is_position_in_sprite_sheet_image(
                            input.outer.position,
                            (member.position + Vec2::new(x, y_offset)).into(),
                            sprite,
                            music_image,
                        );
                        let pressed_note = input.outer.left_button.is_pressed() && is_hit;
                        let held_down_new_note = input.outer.left_button.is_held_down()
                            && is_hit
                            && self.tracker.last_piano_key != Some(touched_key);
                        if pressed_note || held_down_new_note {
                            audio_player.stop_all_notes();
                            self.tracker.last_piano_key = Some(touched_key);
                            log::debug!("NO {} {}", octave, semitone);
                            let lowest_note = self.current_instrument().lowest_note;
                            let adjusted_note = lowest_note + touched_key;

                            audio_player.play_note(self.channel(), adjusted_note, 127);
                        }

                        if self.tracker.last_piano_key.is_some()
                            && input.outer.left_button.is_released()
                        {
                            audio_player.stop_all_notes();
                            self.tracker.last_piano_key = None;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

pub fn rough_octave_span(music_maker: &MusicMaker) -> i32 {
    if music_maker.is_extended_keyboard() {
        4
    } else {
        3
    }
}

pub fn octave_height(music_maker: &MusicMaker) -> f32 {
    if music_maker.is_extended_keyboard() {
        48.0
    } else {
        72.0
    }
}

pub fn rough_offset(octave: i32, octave_height: f32) -> f32 {
    octave as f32 * -octave_height + octave_height
}

pub fn sprite_offset(music_maker: &MusicMaker) -> u32 {
    if music_maker.is_extended_keyboard() {
        36
    } else {
        0
    }
}

fn note_positions(music_maker: &MusicMaker) -> [((f32, f32), u32); 12] {
    if music_maker.is_extended_keyboard() {
        [
            ((-140.0, 26.0), 23),
            ((-146.0, 23.0), 16),
            ((-140.0, 19.0), 22),
            ((-146.0, 15.0), 16),
            ((-140.0, 13.0), 21),
            ((-140.0, 5.0), 20),
            ((-146.0, 3.0), 16),
            ((-140.0, 0.0), 19),
            ((-146.0, -5.0), 16),
            ((-140.0, -9.0), 18),
            ((-146.0, -13.0), 16),
            ((-140.0, -17.0), 17),
        ]
    } else {
        [
            ((-140.0, 43.0), 23),
            ((-146.0, 38.0), 16),
            ((-140.0, 33.0), 22),
            ((-146.0, 26.0), 16),
            ((-140.0, 23.0), 21),
            ((-140.0, 13.0), 20),
            ((-146.0, 8.0), 16),
            ((-140.0, 2.0), 19),
            ((-146.0, -4.0), 16),
            ((-140.0, -10.0), 18),
            ((-146.0, -16.0), 16),
            ((-140.0, -21.0), 17),
        ]
    }
}

pub fn note_positions_draw(music_maker: &MusicMaker) -> [((f32, f32), u32); 12] {
    if music_maker.is_extended_keyboard() {
        [
            ((-140.0, 26.0), 23),
            ((-140.0, 19.0), 22),
            ((-140.0, 13.0), 21),
            ((-140.0, 5.0), 20),
            ((-140.0, 0.0), 19),
            ((-140.0, -9.0), 18),
            ((-140.0, -17.0), 17),
            ((-146.0, 23.0), 16),
            ((-146.0, 15.0), 16),
            ((-146.0, 3.0), 16),
            ((-146.0, -5.0), 16),
            ((-146.0, -13.0), 16),
        ]
    } else {
        [
            ((-140.0, 43.0), 23),
            ((-140.0, 33.0), 22),
            ((-140.0, 23.0), 21),
            ((-140.0, 13.0), 20),
            ((-140.0, 2.0), 19),
            ((-140.0, -10.0), 18),
            ((-140.0, -21.0), 17),
            ((-146.0, 38.0), 16),
            ((-146.0, 26.0), 16),
            ((-146.0, 8.0), 16),
            ((-146.0, -4.0), 16),
            ((-146.0, -16.0), 16),
        ]
    }
}

pub fn is_outside_edge(
    music_maker: &MusicMaker,
    octave: i32,
    semitone: usize,
    offset: f32,
) -> bool {
    if music_maker.is_extended_keyboard() {
        if octave == 3 && semitone != 0 {
            return true;
        }
    } else if !(-70.0..=76.0).contains(&offset) {
        return true;
    }
    false
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Action {
    PlayPhrase,
    PausePhrase,
    StopPhrase,
    PreviousInstrument,
    NextInstrument,
    PreviousTrack,
    NextTrack,
    RefreshSong,
}

#[derive(Debug)]
pub struct Instrument {
    pub name: String,
    pub preset: u8,
    pub lowest_note: u8,
}

// TODO: Read from file
pub fn default_instruments() -> Vec<Instrument> {
    // TODO :Temp lowest notes
    vec![
        // Group 1
        Instrument {
            name: "Piano".to_owned(),
            preset: 0,
            lowest_note: 48,
        },
        Instrument {
            name: "Organ".to_owned(),
            preset: 18,
            lowest_note: 48,
        },
        Instrument {
            name: "Harpsichord".to_owned(),
            preset: 6,
            lowest_note: 48,
        },
        Instrument {
            name: "Harmonica".to_owned(),
            preset: 22,
            lowest_note: 48,
        },
        Instrument {
            name: "Flute".to_owned(),
            preset: 73,
            lowest_note: 60,
        },
        Instrument {
            name: "Trumpet".to_owned(),
            preset: 56,
            lowest_note: 48,
        },
        Instrument {
            name: "Saxophone".to_owned(),
            preset: 65,
            lowest_note: 48,
        },
        Instrument {
            name: "Pan Flute".to_owned(),
            preset: 75,
            lowest_note: 48,
        },
        // Group 2
        Instrument {
            name: "Acoustic Guitar".to_owned(),
            preset: 24,
            lowest_note: 48,
        },
        Instrument {
            name: "Electric Guitar".to_owned(),
            preset: 29,
            lowest_note: 36,
        },
        Instrument {
            name: "Banjo".to_owned(),
            preset: 105,
            lowest_note: 36,
        },
        Instrument {
            name: "Bass Guitar".to_owned(),
            preset: 33,
            lowest_note: 24,
        },
        Instrument {
            name: "Violin".to_owned(),
            preset: 40,
            lowest_note: 60,
        },
        Instrument {
            name: "Xylophone".to_owned(),
            preset: 12,
            lowest_note: 60,
        },
        Instrument {
            name: "Vibraphone".to_owned(),
            preset: 11,
            lowest_note: 60,
        },
        Instrument {
            name: "Timpani".to_owned(),
            preset: 47,
            lowest_note: 36,
        },
        // Nearby Galaxy
        Instrument {
            name: "Polysynth".to_owned(),
            preset: 90,
            lowest_note: 48,
        },
        Instrument {
            name: "Space Voice".to_owned(),
            preset: 91,
            lowest_note: 60,
        },
        Instrument {
            name: "Halo Pad".to_owned(),
            preset: 94,
            lowest_note: 60,
        },
        Instrument {
            name: "Synth Bass".to_owned(),
            preset: 39,
            lowest_note: 24,
        },
        Instrument {
            name: "Echo Drops".to_owned(),
            preset: 102,
            lowest_note: 60,
        },
        Instrument {
            name: "Call Me".to_owned(),
            preset: 124,
            lowest_note: 48,
        },
        // Magic
        Instrument {
            name: "Dog".to_owned(),
            preset: 61,
            lowest_note: 48,
        },
        Instrument {
            name: "Bagpipes".to_owned(),
            preset: 109,
            lowest_note: 48,
        },
        Instrument {
            name: "Squeal".to_owned(),
            preset: 120,
            lowest_note: 48,
        },
        Instrument {
            name: "Windchime".to_owned(),
            preset: 104,
            lowest_note: 60,
        },
        Instrument {
            name: "Ocarina".to_owned(),
            preset: 79,
            lowest_note: 48,
        },
        Instrument {
            name: "Harp".to_owned(),
            preset: 46,
            lowest_note: 48,
        },
        Instrument {
            name: "Brakes".to_owned(),
            preset: 127,
            lowest_note: 48,
        },
        Instrument {
            name: "Bell Tower".to_owned(),
            preset: 112,
            lowest_note: 24,
        },
        // Voice and Leftovers
        Instrument {
            name: "Choir".to_owned(),
            preset: 52,
            lowest_note: 48,
        },
        Instrument {
            name: "Oohs".to_owned(),
            preset: 53,
            lowest_note: 48,
        },
        Instrument {
            name: "Synth Voice".to_owned(),
            preset: 54,
            lowest_note: 48,
        },
        Instrument {
            name: "Whistlin'".to_owned(),
            preset: 78,
            lowest_note: 72,
        },
        Instrument {
            name: "Celesta".to_owned(),
            preset: 8,
            lowest_note: 72,
        },
        Instrument {
            name: "Music Box".to_owned(),
            preset: 10,
            lowest_note: 60,
        },
        Instrument {
            name: "Ukelele".to_owned(),
            preset: 106,
            lowest_note: 48,
        },
        Instrument {
            name: "Taiko Drum".to_owned(),
            preset: 116,
            lowest_note: 36,
        },
        // 8-bit
        Instrument {
            name: "Lead".to_owned(),
            preset: 83,
            lowest_note: 48,
        },
        Instrument {
            name: "Former".to_owned(),
            preset: 84,
            lowest_note: 36,
        },
        Instrument {
            name: "Blower".to_owned(),
            preset: 85,
            lowest_note: 48,
        },
        Instrument {
            name: "Elec".to_owned(),
            preset: 86,
            lowest_note: 24,
        },
        Instrument {
            name: "Elec Lead".to_owned(),
            preset: 87,
            lowest_note: 36,
        },
        Instrument {
            name: "High".to_owned(),
            preset: 88,
            lowest_note: 72,
        },
        Instrument {
            name: "Space".to_owned(),
            preset: 89,
            lowest_note: 60,
        },
        Instrument {
            name: "Light Elec".to_owned(),
            preset: 93,
            lowest_note: 36,
        },
        // 7-bit
        Instrument {
            name: "Era".to_owned(),
            preset: 95,
            lowest_note: 48,
        },
        Instrument {
            name: "Mid".to_owned(),
            preset: 96,
            lowest_note: 48,
        },
        Instrument {
            name: "Soft".to_owned(),
            preset: 97,
            lowest_note: 48,
        },
        Instrument {
            name: "Melon".to_owned(),
            preset: 98,
            lowest_note: 36,
        },
        Instrument {
            name: "Gen".to_owned(),
            preset: 99,
            lowest_note: 48,
        },
        Instrument {
            name: "Rev".to_owned(),
            preset: 100,
            lowest_note: 48,
        },
        Instrument {
            name: "Alt".to_owned(),
            preset: 101,
            lowest_note: 48,
        },
        Instrument {
            name: "Band".to_owned(),
            preset: 103,
            lowest_note: 36,
        },
        Instrument {
            name: "Chip Organ".to_owned(),
            preset: 108,
            lowest_note: 48,
        },
        Instrument {
            name: "Test Ins 1".to_owned(),
            preset: 107,
            lowest_note: 24,
        },
        Instrument {
            name: "Test Ins 2".to_owned(),
            preset: 110,
            lowest_note: 36,
        },
        Instrument {
            name: "Slap Bass".to_owned(),
            preset: 36,
            lowest_note: 24,
        },
        Instrument {
            name: "Accordian".to_owned(),
            preset: 21,
            lowest_note: 48,
        },
    ]
}
