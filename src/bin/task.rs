#![windows_subsystem = "windows"]

use windows::core::*;
use windows::System::UserProfile::*;
use windows::Storage::StorageFile;
use windows_window::{Window, quit};
use rand::seq::IteratorRandom;

use lock_screen_carousel::{CarouselChooseType, registry::*, wait};

fn choose_photo(path: &String) -> Result<()> {
    #[cfg(debug_assertions)] {
        println!("* {}", path);
    }

    let path = HSTRING::from(path);
    let file = StorageFile::GetFileFromPathAsync(&path)?;
    wait!(file);
    let file = file.GetResults()?;

    let action = LockScreen::SetImageFileAsync(&file)?;
    wait!(action);

    Ok(())
}

fn choose_next_photo(photos: Vec<String>, current_index: usize) -> Result<()> {
    let current_index: usize = if current_index + 1 < photos.len() {
        current_index + 1
    } else { 0 };
    set_carousel_current(current_index as u32)?;
    choose_photo(&photos[current_index])
}

fn choose_random_photo(photos: Vec<String>) -> Result<()> {
    if let Some(chosen_path) = photos.into_iter().choose(&mut rand::rng()) {
        return choose_photo(&chosen_path);
    }

    // no photos, treat as successful
    Ok(())
}

const WS_EX_TOOLWINDOW: u32 = 0x80;

fn main() -> Result<()> {
    let photos = get_carousel_collection()?;
    let choose_type = get_carousel_choose_type()?;

    match choose_type {
        CarouselChooseType::Iterate => {
            let current_index = get_carousel_current().unwrap_or(0);
            choose_next_photo(photos, current_index as usize)
        },
        CarouselChooseType::Random => choose_random_photo(photos),
    }?;

    let _window = Window::new("LockScreenCarousel - Task")
        .size(0, 0)
        .ex_style(WS_EX_TOOLWINDOW)
        .create()
        .unwrap();

    quit();
    Ok(())
}
