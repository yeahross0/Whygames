use super::art::Sprite;
use super::play;
use super::play::{ChoreId, DemandId, QuestionId};
use super::serial::{Demand, Question};
use super::MakerNote;
use super::Vec2;
use crate::music::PointInMusic;
use std::fmt;
use std::ops::Not;

#[derive(Clone, Debug)]
pub enum Event {
    AddMember {
        index: Option<usize>,
        member: play::Member,
    },
    RemoveMember {
        index: usize,
    },
    MoveMember {
        index: usize,
        from: Vec2,
        to: Vec2,
    },
    RenameMember {
        index: usize,
        from: String,
        to: String,
    },
    UpdateChore {
        id: ChoreId,
        chore: Box<play::Chore>,
    },
    MoveChoreUp {
        id: ChoreId,
    },
    MoveChoreDown {
        id: ChoreId,
    },
    UpdateQuestion {
        id: QuestionId,
        question: Question,
    },
    UpdateDemand {
        id: DemandId,
        demand: Demand,
    },
    AddCharacter {
        index: usize,
        ch: char,
    },
    RemoveCharacter {
        index: usize,
    },
    SetStartSprite {
        index: usize,
        from: Sprite,
        to: Sprite,
    },
    // Music Stuff
    AddNote {
        editing_position: PointInMusic,
        note: MakerNote,
    },
    RemoveNote {
        editing_position: PointInMusic,
        note: MakerNote,
    },
    SwitchToExtendedKeyboard {
        editing_position: PointInMusic,
        old_notes: Vec<MakerNote>,
    },
    SwitchToStandardKeyboard {
        editing_position: PointInMusic,
        old_notes: Vec<MakerNote>,
    },
    SwitchToAlternativeSignature {
        editing_position: PointInMusic,
        old_notes: Vec<MakerNote>,
    },
    SwitchToStandardSignature {
        editing_position: PointInMusic,
        old_notes: Vec<MakerNote>,
    },
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::AddMember { .. } => {
                write!(f, "Add Member")
            }
            Event::RemoveMember { .. } => {
                write!(f, "Remove Member")
            }
            Event::MoveMember { from, to, .. } => {
                //write!(f, "Move Member")
                write!(f, "Move Member from {:?} to {:?}", from, to)
            }
            Event::RenameMember { from, to, .. } => {
                //write!(f, "Move Member")
                write!(f, "Rename Member from {} to {}", from, to)
            }
            Event::UpdateChore { .. } => {
                write!(f, "Update Chore")
            }
            Event::MoveChoreUp { .. } => {
                write!(f, "Move Chore Up")
            }
            Event::MoveChoreDown { .. } => {
                write!(f, "Move Chore Down")
            }
            Event::UpdateQuestion { id, question } => {
                write!(f, "Update Question: {:?}, {:?}", id, question)
            }
            Event::UpdateDemand { id, demand } => {
                write!(f, "Update Demand: {:?}, {:?}", id, demand)
            }
            Event::AddCharacter { .. } => {
                write!(f, "Add letter")
            }
            Event::RemoveCharacter { .. } => {
                write!(f, "Remove letter")
            }
            Event::SetStartSprite { .. } => {
                write!(f, "Set start sprite")
            }
            Event::AddNote { .. } => {
                write!(f, "Add note")
            }
            Event::RemoveNote { .. } => {
                write!(f, "Remove note")
            }
            // TODO: Keyboard & signatures
            event => {
                write!(f, "{:?}", event)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Step {
    pub forward: Event,
    pub back: Event,
    pub direction: StepDirection,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum StepDirection {
    Forward,
    Back,
}

impl Not for StepDirection {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            StepDirection::Forward => StepDirection::Back,
            StepDirection::Back => StepDirection::Forward,
        }
    }
}
