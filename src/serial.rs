use crate::art::SpriteSize;
use crate::err::WhyResult;
use crate::nav::Link;
use crate::FileSystem;

use super::meta::{INNER_CENTRE, OUTER_CENTRE};
use super::pixels;
//use super::{colours, Colour};
use super::anim::AnimationStyle;
use super::art::Sprite;
use super::common::Speed;
use super::inp::Button;
use macroquad::logging as log;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use strum_macros::EnumString;

pub const ANIMATION_SPRITE_COUNT: usize = 8;
pub const CHORE_COUNT: usize = 6;
pub const QUESTION_COUNT: usize = 6;
pub const DEMAND_COUNT: usize = 6;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ImageString(pub String);

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SoundString(pub String);

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Music {
    pub data: SoundString,
    pub looped: bool,
}

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum Length {
    #[default]
    Short,
    Long,
    Infinite,
}

impl Length {
    pub fn last_frame(self) -> Option<usize> {
        match self {
            Length::Short => Some(240),
            Length::Long => Some(480),
            Length::Infinite => None,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, EnumString)]
pub enum GameSize {
    #[default]
    Small,
    Big,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, EnumString)]
pub enum IntroText {
    Same(String),
    Levels([String; 3]),
}

impl Default for IntroText {
    fn default() -> IntroText {
        IntroText::Same("".to_string())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct AssetFilenames {
    pub image: Option<String>,
    pub font: Option<String>,
    pub music: Option<String>,
    pub sounds: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Cartridge {
    pub format_version: usize,
    pub members: Vec<Member>,
    pub published: bool,
    pub length: Length,
    pub size: GameSize,
    #[serde(default)]
    pub intro_text: IntroText,
    pub font: ImageString,
    pub image: ImageString,
    pub music: Option<Music>,
    #[serde(default)]
    pub asset_filenames: AssetFilenames,
    pub sounds: HashMap<String, SoundString>,
}

impl Cartridge {
    pub fn new(size: GameSize, image_string: ImageString, font_string: ImageString) -> Cartridge {
        Cartridge {
            members: vec![Member {
                name: "Background".to_owned(),
                position: match size {
                    GameSize::Small => INNER_CENTRE,
                    GameSize::Big => OUTER_CENTRE,
                },
                text: Text {
                    contents: "".to_owned(),
                    colour: Colour {
                        r: 0.055,
                        g: 0.098,
                        b: 0.114,
                        a: 1.0,
                    },
                },
                sprite: Sprite {
                    index: 0,
                    size: match size {
                        GameSize::Small => SpriteSize::InnerBg,
                        GameSize::Big => SpriteSize::OuterBg,
                    },
                },
                ..Default::default()
            }],
            size,
            image: image_string,
            font: font_string,
            ..Default::default()
        }
    }

    pub async fn load(link: &Link, file_system: &FileSystem) -> WhyResult<Cartridge> {
        let filename = &link.to_filename();
        let file_contents = if file_system.memfs.contains_key(filename) {
            file_system.memfs[filename].clone()
        } else {
            macroquad::file::load_string(filename).await?
        };

        Self::from_file_contents(&file_contents)
    }

    pub fn from_file_contents(file_contents: &str) -> WhyResult<Cartridge> {
        serde_json::from_str(file_contents)
            .map_err(|e| format!("Error deserialising game: {:?}", e).into())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Member {
    pub name: String,
    pub position: pixels::Position,
    pub sprite: Sprite,
    pub text: Text,
    pub todo_list: Vec<Chore>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Chore {
    pub questions: Vec<Question>,
    pub demands: Vec<Demand>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, EnumString, Default)]
pub enum Question {
    #[default]
    None,
    IsTimeAt(When),
    IsMouseInteracting {
        which: WhichButton,
        state: Option<Button>,
        hover: Hover,
    },
    IsSwitchSetTo {
        name: String,
        switch: Switch,
    },
    IsWinStatusSetTo(WinStatus),
    IsSpriteSetTo(Sprite),
    IsAnimationFinished,
    IsCollidingWith(CollisionWith),
    IsTextSetTo {
        value: String,
    },
    IsVariableSetTo {
        name: String,
        value: String,
    },
    IsPagedVariableSelected {
        name: String,
        value: String,
    },
    IsPagedVariableValid {
        name: String,
        value: String,
    },
    IsAnimationSpriteValid {
        index: usize,
    },
    IsSubgamePlaying,
    IsSubgameEnding,
    IsShortcutUsed(Shortcut),
    IsOnDesktop,
    IsOnWeb,
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, EnumString, Default)]
pub enum Demand {
    #[default]
    None,
    // Game Stuff
    SetSprite(Sprite),
    SetSwitch(Switch),
    SetText(Text),
    Win,
    Lose,
    Animate {
        style: AnimationStyle,
        speed: Speed,
        sprites: Vec<Sprite>,
    },
    StopAnimation,
    PlaySound {
        name: String,
    },
    StopMusic,
    StopSounds,
    Motion(Motion),
    // Animation?
    SetAnimationSprite,
    AddAnimationSprite,
    RemoveAnimationSprite,
    MoveAnimationUp,
    MoveAnimationDown,
    // Editor Stuff
    New,
    Load,
    Save,
    EditText,
    SetVariable {
        name: String,
        value: String,
    },
    SetVariableFromText {
        name: String,
    },
    SetTextFromVariable {
        name: String,
    },
    SetTextFromPosition {
        axis: Axis,
        scale: i32,
    },
    SelectPagedVariable {
        name: String,
        value: String,
    },
    Add1ToVariable {
        name: String,
    },
    Sub1FromVariable {
        name: String,
    },
    PreviewMusic,
    PreviousPage,
    NextPage,
    SetImageFile,
    SetMusicFile,
    UpdateScratchFromMember,
    UpdateScratchFromQuestion,
    UpdateScratchFromDemand,
    SwitchMember,
    // Events
    AddMember,
    RemoveMember,
    CloneMember,
    RenameMember,
    RemoveChore,
    MoveChoreUp,
    MoveChoreDown,
    MoveQuestionUp,
    MoveQuestionDown,
    MoveDemandUp,
    MoveDemandDown,
    UpdateQuestion,
    UpdateDemand,
    SetStartSprite,
    // Menu Actions
    Quit,
    Stop,
    Play,
    Pause,
    MoveToGame {
        name: String,
    },
    FadeToGame {
        name: String,
    },
    FadeOut,
    BackInQueue,
    NextInQueue,
    AddToQueue {
        name: String,
    },
    ResetQueue,
    // TODO: TEMP,
    ClearArt,
    SaveArt,
    // Music Maker Stuff
    // TODO: PhraseControls :: Play | Pause | Stop?
    PlayPhrase,
    PausePhrase,
    StopPhrase,
    PreviousInstrument, //Temp
    NextInstrument,     // Temp
    PreviousTrack,      //Temp
    NextTrack,          // Temp
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, EnumString, Default)]
pub enum Motion {
    #[default]
    Stop,
    Go {
        direction: HashSet<Direction>,
        speed: Speed,
    },
    GoToPoint {
        point: pixels::Position,
        speed: Speed,
    },
    JumpTo(JumpLocation),
    Swap {
        name: String,
    },
    Roam {
        roam_type: RoamType,
        area: pixels::Rect,
        speed: Speed,
        movement_handling: MovementHandling,
    },
    ClampPosition {
        area: pixels::Rect,
    },
    Target {
        name: String,
        offset: pixels::Position,
        speed: Speed,
    },
    AttachFromPositions {
        name: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, EnumString, Default)]
pub enum RoamType {
    #[default]
    Wiggle,
    Insect,
    Reflect,
    Bounce,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, EnumString, Default)]
pub enum MovementHandling {
    #[default]
    Anywhere,
    TryNotToOverlap,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, EnumString, Default)]
pub enum When {
    #[default]
    Start,
    End,
    Exact {
        time: usize,
    },
    Random {
        start: usize,
        end: usize,
    },
}

// TODO: Make relative into Member + offset?
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, EnumString, Default)]
pub enum JumpLocation {
    #[default]
    Mouse,
    Point(pixels::Position),
    Area(pixels::Rect),
    Member {
        name: String,
    },
    Relative {
        offset: pixels::Position,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, EnumString, Default)]
pub enum Axis {
    #[default]
    X,
    Y,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Default)]
pub enum Direction {
    #[default]
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, EnumString)]
pub enum WhichButton {
    #[default]
    Left,
    Middle,
    Right,
}

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, EnumString)]
pub enum Hover {
    #[default]
    Anywhere,
    This,
    TopMember,
}

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, EnumString)]
pub enum Switch {
    #[default]
    Off,
    On,
    SwitchedOff,
    SwitchedOn,
}

impl Switch {
    pub fn apply(&mut self, applied_switch: Switch) {
        // TODO: Guaranteeing that applied_switch can't be On/Off instead of SwitchedOn/SwitchedOff
        if *self == Switch::SwitchedOff {
            *self = Switch::Off;
        }
        if *self == Switch::SwitchedOn {
            *self = Switch::On;
        }
        if applied_switch == Switch::SwitchedOn && *self != Switch::On {
            *self = Switch::SwitchedOn;
        }
        if applied_switch == Switch::SwitchedOff && *self != Switch::Off {
            *self = Switch::SwitchedOff;
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, Default, EnumString, PartialEq)]
pub enum WinStatus {
    #[default]
    NotYetWon,
    NotYetLost,
    JustWon,
    JustLost,
    Won,
    Lost,
}

#[derive(Clone, Debug, Serialize, Deserialize, EnumString, PartialEq)]
pub enum CollisionWith {
    Area(pixels::Rect),
    Member { name: String },
}

impl Default for CollisionWith {
    fn default() -> CollisionWith {
        CollisionWith::Area(pixels::Rect::default())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Colour {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Text {
    pub contents: String,
    pub colour: Colour,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, Eq, Hash, EnumString)]
pub enum Shortcut {
    #[default]
    Ok,
    Cancel,
}
