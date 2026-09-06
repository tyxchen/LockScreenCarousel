pub mod registry;
pub mod task_scheduler;
pub mod utils;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CarouselTrigger {
    Never = 0,
    Interval = 1,
    Lock = 2,
}

impl From<CarouselTrigger> for u32 {
    fn from(value: CarouselTrigger) -> Self {
        value as u32
    }
}
impl From<u32> for CarouselTrigger {
    fn from(value: u32) -> Self {
        match value {
            2 => Self::Lock,
            1 => Self::Interval,
            _ => Self::Never,
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CarouselChooseType {
    Iterate = 0,
    Random = 1
}

impl From<CarouselChooseType> for u32 {
    fn from(value: CarouselChooseType) -> Self {
        value as u32
    }
}
impl From<u32> for CarouselChooseType {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::Random,
            _ => Self::Iterate,
        }
    }
}
