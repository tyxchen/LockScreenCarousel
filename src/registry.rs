use windows_registry::*;
use windows::core::Result;
use crate::{CarouselChooseType, CarouselTrigger};

pub const LOCK_SCREEN_CAROUSEL_KEY: &str = "SOFTWARE\\LockScreenCarousel";

macro_rules! conv_result {
    ($blk:block) => {
        match (|| -> windows_registry::Result<_> { $blk })() {
            Err(e) => unsafe { Err(std::mem::transmute::<_, windows::core::Error>(e)) },
            Ok(x) => Ok(x),
        }
    };
}

pub fn get_carousel_interval() -> Result<u32> {
    conv_result!({
        let key = CURRENT_USER.create(LOCK_SCREEN_CAROUSEL_KEY)?;
        key.get_u32("Interval")
    })
}
pub fn set_carousel_interval(interval: u32) -> Result<()> {
    conv_result!({
        let key = CURRENT_USER.create(LOCK_SCREEN_CAROUSEL_KEY)?;
        key.set_u32("Interval", interval)
    })
}
pub fn get_carousel_trigger() -> Result<CarouselTrigger> {
    conv_result!({
        let key = CURRENT_USER.create(LOCK_SCREEN_CAROUSEL_KEY)?;
        key.get_u32("Trigger").map(CarouselTrigger::from)
    })
}
pub fn set_carousel_trigger(trigger: CarouselTrigger) -> Result<()> {
    conv_result!({
        let key = CURRENT_USER.create(LOCK_SCREEN_CAROUSEL_KEY)?;
        key.set_u32("Trigger", trigger as u32)
    })
}
pub fn get_carousel_collection() -> Result<Vec<String>> {
    conv_result!({
        let key = CURRENT_USER.create(LOCK_SCREEN_CAROUSEL_KEY)?;
        key.get_multi_string("Photos")
    })
}
pub fn set_carousel_collection(photos: &Vec<String>) -> Result<()> {
    conv_result!({
        let key = CURRENT_USER.create(LOCK_SCREEN_CAROUSEL_KEY)?;
        let photos = photos.iter().map(|p| p.as_str()).collect::<Vec<_>>();
        key.set_multi_string("Photos", &photos)
    })
}
pub fn get_carousel_current() -> Result<u32> {
    conv_result!({
        let key = CURRENT_USER.create(LOCK_SCREEN_CAROUSEL_KEY)?;
        key.get_u32("CurrentPhoto")
    })
}
pub fn set_carousel_current(index: u32) -> Result<()> {
    conv_result!({
        let key = CURRENT_USER.create(LOCK_SCREEN_CAROUSEL_KEY)?;
        key.set_u32("CurrentPhoto", index)
    })
}
pub fn get_carousel_choose_type() -> Result<CarouselChooseType> {
    conv_result!({
        let key = CURRENT_USER.create(LOCK_SCREEN_CAROUSEL_KEY)?;
        key.get_u32("ChooseType").map(CarouselChooseType::from)
    })
}
pub fn set_carousel_choose_type(choose_type: CarouselChooseType) -> Result<()> {
    conv_result!({
        let key = CURRENT_USER.create(LOCK_SCREEN_CAROUSEL_KEY)?;
        key.set_u32("ChooseType", choose_type as u32)
    })
}
