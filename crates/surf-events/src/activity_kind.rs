use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    SolSent,
    SolReceived,
    SurfSent,
    SurfReceived,
    NameRegistered,
    Followed,
    Unfollowed,
}

#[derive(Debug, Error)]
#[error("Invalid activity kind")]
pub struct InvalidActivityKind;

impl ActivityKind {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::SolSent => 0,
            Self::SolReceived => 1,
            Self::SurfSent => 2,
            Self::SurfReceived => 3,
            Self::NameRegistered => 4,
            Self::Followed => 5,
            Self::Unfollowed => 6,
        }
    }

    pub fn from_u8(value: u8) -> Result<Self, InvalidActivityKind> {
        match value {
            0 => Ok(Self::SolSent),
            1 => Ok(Self::SolReceived),
            2 => Ok(Self::SurfSent),
            3 => Ok(Self::SurfReceived),
            4 => Ok(Self::NameRegistered),
            5 => Ok(Self::Followed),
            6 => Ok(Self::Unfollowed),
            _ => Err(InvalidActivityKind),
        }
    }
}
