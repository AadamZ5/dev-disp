mod detailed_timing_descriptor;

pub use detailed_timing_descriptor::*;

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
