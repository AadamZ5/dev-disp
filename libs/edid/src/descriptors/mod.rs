mod detailed_timing;
mod display_range_limits;

pub use detailed_timing::*;
pub use display_range_limits::*;

#[derive(Debug, Clone)]
pub enum EdidDescriptor {
    DetailedTiming(EdidDetailedTimingDescriptor),
}

impl EdidDescriptor {
    pub fn to_bytes(&self) -> [u8; 18] {
        match self {
            Self::DetailedTiming(t) => t.to_bytes(),
        }
    }
}
