use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use windows::core::*;
use windows::Win32::{System::Com::*, UI::Shell::*};
use windows::Storage::Streams::*;
use windows::System::UserProfile::*;
use windows_reactor::*;

use lock_screen_carousel::{CarouselChooseType, CarouselTrigger, registry::*, task_scheduler::*, wait};

macro_rules! label {
    ($el:literal) => { label!(TextBlock::new().text($el)) };
    ($el:expr) => { $el.grid_column(0).vertical_alignment(VerticalAlignment::Center).horizontal_alignment(HorizontalAlignment::Left) };
}
macro_rules! action {
    ($el:expr) => { $el.grid_column(1).vertical_alignment(VerticalAlignment::Center).horizontal_alignment(HorizontalAlignment::Right) };
}

fn item_row() -> Grid {
    Grid::new().columns([GridLength::Auto, GridLength::STAR])
}

fn open_file_dialog() -> Result<Vec<String>> {
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;
        if let Some(home_dir) = std::env::home_dir().and_then(|e| e.into_string().ok()) {
            let home_dir = HSTRING::from(home_dir);
            let home_dir: IShellItem = SHCreateItemFromParsingName(PCWSTR::from_raw(home_dir.as_ptr()), None)?;
            dialog.SetDefaultFolder(&home_dir)?;
        }
        dialog.SetOptions(FOS_ALLOWMULTISELECT)?;

        let file_types = [
            Common::COMDLG_FILTERSPEC {
                pszName: w!("Image files"),
                pszSpec: w!("*.bmp;*.jpg;*.jpeg;*.png"),
            }
        ];
        dialog.SetFileTypes(&file_types)?;

        let mut paths = vec![];

        if dialog.Show(None).is_ok() {
            let results = dialog.GetResults()?;
            paths = (0..results.GetCount()?).filter_map(|i| {
                let result = results.GetItemAt(i).ok()?;
                let path = result.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
                CoTaskMemFree((!path.0.is_null()).then(move || path.0 as _));
                Some(path.to_hstring().to_string_lossy())
            }).collect::<Vec<_>>();
        }

        Ok(paths)
    }
}

fn get_image_data(stream: &IRandomAccessStream) -> Result<Vec<u8>> {
    let image_size_bytes = stream.Size()? as u32;
    let data_reader = DataReader::CreateDataReader(stream)?;
    let image_bytes_loaded = data_reader.LoadAsync(image_size_bytes)?;
    wait!(image_bytes_loaded);
    let image_bytes_loaded = image_bytes_loaded.GetResults()? as usize;
    let mut out_buf: Vec<u8> = Vec::with_capacity(image_bytes_loaded);
    out_buf.resize(image_bytes_loaded, 0_u8);
    data_reader.ReadBytes(&mut out_buf)?;

    Ok(out_buf)
}

fn get_current_lock_screen_image() -> Result<(Vec<u8>, HSTRING)> {
    let image_stream = LockScreen::GetImageStream()?;
    let uri = LockScreen::OriginalImageFile()?.Path()?;
    let data = get_image_data(&image_stream)?;
    Ok((data, uri))
}

fn get_primary_display_dimensions() -> Result<(u32, u32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::*;
    unsafe {
        let handle = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info: MONITORINFO = Default::default();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoA(handle, &mut info).ok().map(move |_| (info.rcMonitor.right as u32, info.rcMonitor.bottom as u32))
    }
}

fn get_cur_time_and_date() -> (HSTRING, HSTRING) {
    unsafe {
        use windows::Win32::Globalization::*;
        const LOCALE_NAME_MAX_LENGTH: usize = 85;
        let mut locale = vec![0_u16; LOCALE_NAME_MAX_LENGTH];
        let len = GetUserDefaultLocaleName(&mut locale);
        locale.truncate(len as usize);
        let locale = PCWSTR::from_raw(locale.as_ptr());

        let len = GetTimeFormatEx(locale, TIME_NOSECONDS, None, None, None);
        debug_assert!(len > 0, "{:?}", Error::from_thread());
        let mut cur_time = vec![0_u16; len as usize];
        let _ = GetTimeFormatEx(locale, TIME_NOSECONDS, None, None, Some(&mut cur_time));

        // let len = GetDateFormatEx(locale, DATE_MONTHDAY, None, None, None, None);
        let date_format = w!("dddd, MMMM d");
        let len = GetDateFormatEx(locale, DATE_AUTOLAYOUT, None, date_format, None, None);
        debug_assert!(len > 0, "{:?}", Error::from_thread());
        let mut cur_date = vec![0_u16; len as usize];
        // let _ = GetDateFormatEx(locale, DATE_MONTHDAY, None, None, Some(&mut cur_date), None);
        let _ = GetDateFormatEx(locale, DATE_AUTOLAYOUT, None, date_format, Some(&mut cur_date), None);

        (HSTRING::from_wide(&cur_time), HSTRING::from_wide(&cur_date))
    }
}

// fn default_fade_in() -> Option<AnimationConfig> {
//     Some(AnimationConfig::fade_in(std::time::Duration::from_millis(150)))
// }
// fn default_fade_out() -> ExitTransition {
//     ExitTransition::fade(std::time::Duration::from_millis(150))
// }

fn card(el: impl Into<View>) -> View {
    Border::new()
        .background(ThemeBrush::CardBackground)
        .padding(12.0).margin(Thickness::xy(0.0, 4.0))
        .border_thickness(1.0).border_brush(ThemeBrush::CardStroke).corner_radius(8.0)
        .content(el)
}

fn drop_down_menu<E, C>(labels: &'static [&str], selected: E, on_click_callback: impl Fn(E) -> AppMessage + 'static, cx: &mut ViewContext<C>) -> View
where
    E: From<u32> + Into<u32>,
    C: Component<Message = AppMessage>,
{
    action!(DropDownButton::new()).grid_row(0)
        .content(labels[selected.into() as usize])
        .menu(Menu::new(
            labels.iter().map(|t| MenuItem::item(*t, *t)),
            cx.callback(move |item_text: String| {
                for (i, text) in labels.iter().enumerate() {
                    if item_text == *text {
                        return on_click_callback((i as u32).into());
                    }
                }
                AppMessage::Noop
            })
        ))
}

fn image_tooltip(path: &String) -> Tooltip {
    let iife = move || -> Result<View> {
        let stream = FileRandomAccessStream::OpenAsync(&HSTRING::from(path), windows::Storage::FileAccessMode::Read)?;
        wait!(stream);
        let data = get_image_data(&stream.GetResults()?)?;

        Ok(Image::new().height(180.0).source_data(EncodedImage::new(data)).into())
    };

    Tooltip::rich(iife().unwrap_or("Image could not be loaded".into()))
}

#[derive(Clone, Debug)]
enum AppMessage {
    Trigger(CarouselTrigger),
    Interval(u32),
    Collection(Vec<String>),
    AddToCollection(Vec<String>),
    RemoveFromCollection(usize),
    Select(Option<usize>),
    Type(CarouselChooseType),
    Resize(WindowSize),
    RefreshPreview,
    LockScreenLoaded(Result<(Vec<u8>, HSTRING)>),
    Noop,
}

struct AppComponent {
    first_run: Rc<Cell<bool>>,
    trigger: CarouselTrigger,
    interval: u32,
    collection: Vec<String>,
    collection_dirty: Cell<bool>,
    selected_photo: Option<usize>,
    choose_type: CarouselChooseType,
    window_size: WindowSize,
    lock_screen_image: Option<Arc<[u8]>>,
    lock_screen_path: Option<String>,
    primary_monitor_dimensions: (u32, u32),
}

impl Component for AppComponent {
    type Input = ();
    type Message = AppMessage;

    fn create(_input: &Self::Input, cx: &ComponentContext<Self>) -> Self {
        cx.sender().send(Self::Message::RefreshPreview);
        Self {
            first_run: Rc::new(Cell::new(true)),
            trigger: get_carousel_trigger().unwrap_or(CarouselTrigger::Never),
            interval: get_carousel_interval().unwrap_or(15),
            collection: get_carousel_collection().unwrap_or_default(),
            collection_dirty: Cell::new(false),
            selected_photo: None,
            choose_type: get_carousel_choose_type().unwrap_or(CarouselChooseType::Iterate),
            window_size: WindowSize { width: 1200.0, height: 800.0 },
            lock_screen_image: None,
            lock_screen_path: None,
            primary_monitor_dimensions: get_primary_display_dimensions().unwrap_or((1920, 1080)),
        }
    }

    fn update(&mut self, message: Self::Message, cx: &ComponentContext<Self>) {
        #[cfg(debug_assertions)] {
            if matches!(message, Self::Message::LockScreenLoaded(_)) {
                println!("update: LockScreenLoaded");
            } else {
                println!("update: {:?}", message);
            }
        }
        match message {
            Self::Message::Trigger(trigger) => self.trigger = trigger,
            Self::Message::Interval(interval) => self.interval = interval,
            Self::Message::Collection(collection) => { self.collection = collection; self.collection_dirty.set(true); },
            Self::Message::AddToCollection(mut add) => {
                self.collection.append(&mut add);
                self.collection_dirty.set(true);
            },
            Self::Message::RemoveFromCollection(idx) => {
                self.collection.remove(idx);
                self.collection_dirty.set(true);
            },
            Self::Message::Select(idx) => self.selected_photo = idx,
            Self::Message::Type(typ) => self.choose_type = typ,
            Self::Message::Resize(window_size) => self.window_size = window_size,
            Self::Message::RefreshPreview => {
                cx.spawn_background(|_| Self::Message::LockScreenLoaded(get_current_lock_screen_image()));
            },
            Self::Message::LockScreenLoaded(data) => {
                (self.lock_screen_image, self.lock_screen_path) = match data {
                    Ok((data, uri)) => (
                        Some(data.into()),
                        std::path::Path::new(&uri.to_os_string())
                            .file_name()
                            .and_then(|s| s.to_str())
                            .map(|s| String::from(s))
                    ),
                    Err(e) => panic!("{:?}", e),
                }
            },
            _ => (),
        };
    }

    fn view(&self, _input: &Self::Input, cx: &mut ViewContext<Self>) -> View {
        {
            let trigger = self.trigger;
            let interval = self.interval;
            let first_run = Rc::clone(&self.first_run);
            cx.use_effect("trigger", (self.trigger, self.interval), move || {
                set_carousel_trigger(trigger).unwrap();
                set_carousel_interval(interval).unwrap();
                if !first_run.get() {
                    schedule_task(trigger, interval).unwrap();
                } else {
                    first_run.set(false);
                }
                None
            });
            let choose_type = self.choose_type;
            cx.use_effect("choose_type", self.choose_type, move || {
                set_carousel_choose_type(choose_type).unwrap();
                None
            });

            if self.collection_dirty.get() {
                set_carousel_collection(&self.collection).unwrap();
                self.collection_dirty.set(false);
            }
        }
        let choose_photos = {
            cx.callback(|_| {
                if let Ok(photos) = open_file_dialog() {
                    return Self::Message::AddToCollection(photos);
                }
                Self::Message::Noop
            })
        };

        let lock_screen_el_height = 180.0;
        let lock_screen_el_width = {
            let aspect_ratio = (self.primary_monitor_dimensions.0 as f64) / (self.primary_monitor_dimensions.1 as f64);
            lock_screen_el_height * aspect_ratio
        };
        let (cur_time, cur_date) = get_cur_time_and_date();
        let lock_screen_text = || { TextBlock::new().horizontal_alignment(HorizontalAlignment::Center).foreground(Color::rgb(255, 255, 255)).font_weight(FontWeight::SEMI_BOLD) };
        let lock_screen_el = Border::new().border_brush(Color::rgb(0, 0, 0)).width(lock_screen_el_width).height(lock_screen_el_height).border_thickness(8.0).corner_radius(8.0);
        let lock_screen_el: View = if let (Some(data), Some(path)) = (&self.lock_screen_image, &self.lock_screen_path) {
            Grid::new().rows([GridLength::Auto]).columns([GridLength::Auto, GridLength::Auto]).column_spacing(16.0)
                .children((
                    lock_screen_el.grid_row(0).grid_column(0)
                        .content(Image::new().stretch(Stretch::UniformToFill).width(lock_screen_el_width - 16.0).height(lock_screen_el_height - 16.0).source_data(EncodedImage::new(Arc::clone(data)))),
                    Border::new().grid_row(0).grid_column(0).width(lock_screen_el_width - 16.0).height(lock_screen_el_height - 16.0).background(Color::rgb(0, 0, 0)).opacity(0.2).corner_radius(8.0),
                    StackPanel::new().grid_row(0).grid_column(0).horizontal_alignment(HorizontalAlignment::Center).vertical_alignment(VerticalAlignment::Top).margin(12.0).children((
                        lock_screen_text().text(cur_time.to_string_lossy()).font_size(32.0),
                        lock_screen_text().text(cur_date.to_string_lossy()).font_size(12.0),
                    )),
                    StackPanel::new().grid_row(0).grid_column(1).orientation(Orientation::Horizontal).vertical_alignment(VerticalAlignment::Bottom).spacing(8.0).children((
                        TextBlock::new().vertical_alignment(VerticalAlignment::Center).text(path),
                        Button::new().vertical_alignment(VerticalAlignment::Center).on_click(cx.message(Self::Message::RefreshPreview)).content("Refresh")
                    )),
                ))
        } else {
            lock_screen_el.background(ThemeBrush::SolidBackground).into()
        };

        let photos_el: View = if self.collection.is_empty() {
            TextBlock::new().text("No photos chosen.").margin(12.0).into()
        } else {
            ListView::new()
                .selected_index(self.selected_photo)
                .can_drag_items(true)
                .can_reorder_items(true)
                .allow_drop(true)
                .on_selection_changed(cx.callback(Self::Message::Select))
                .on_reordered(cx.callback(Self::Message::Collection))
                .collection_slot(
                    ListViewSlot::Items,
                    self.collection.iter().enumerate().map(|(idx, s)| {
                        let border_view = Border::new()
                            .content(item_row().column_spacing(12.0).margin(Thickness::xy(0.0, 4.0)).children((
                                label!(TextBlock::new().text(s).margin(4.0)),
                                action!(Button::new().margin(4.0).on_click(cx.callback(move |_| Self::Message::RemoveFromCollection(idx))))
                                    .content(TextBlock::new().text("Remove").font_size(12.0))),
                            ));
                        KeyedView::new(s.clone(), ListViewItem::new().tag(s).content(border_view).tooltip_with(image_tooltip(s)))
                    }))
        };

        let mut lock_trigger_card_children = vec![
            KeyedView::new("trigger_label", label!("Lock screen change trigger").grid_row(0)),
            KeyedView::new(
                "trigger_action",
                drop_down_menu(&["Never", "Timer", "Session lock"], self.trigger, Self::Message::Trigger, cx),
            ),
        ];
        if self.trigger == CarouselTrigger::Interval {
            lock_trigger_card_children.extend([
                KeyedView::new("line", Border::new().height(1.0).background(ThemeBrush::CardStroke).grid_row(1).grid_column_span(2)),
                KeyedView::new("interval_label", label!("Change lock screen every...").grid_row(2)),
                KeyedView::new(
                    "interval_action",
                    action!(StackPanel::new().orientation(Orientation::Horizontal)).grid_row(2).children((
                        NumberBox::new()
                            .value(self.interval as f64)
                            .minimum(5.0).maximum(10080.0)
                            .on_value_changed(cx.callback(|x| if let Some(x) = x { Self::Message::Interval(x as u32) } else { Self::Message::Noop })),
                        TextBlock::new().text(" minutes").vertical_alignment(VerticalAlignment::Center),
                    ))
                ),
            ]);
        };

        let main_body = [
            lock_screen_el,
            TextBlock::new().text("Settings").font_weight(FontWeight::SEMI_BOLD).font_size(20.0).into(),
            card(item_row().children((
                label!("Choose next background by..."),
                drop_down_menu(&["Next in list", "Random"], self.choose_type, Self::Message::Type, cx),
            ))),
            card(item_row()
                .rows(vec![GridLength::Auto; if self.trigger == CarouselTrigger::Interval { 3 } else { 1 }])
                .row_spacing(12.0)
                .keyed_children(lock_trigger_card_children)
            ),
            item_row().children((
                label!(TextBlock::new().text("Photos").font_weight(FontWeight::SEMI_BOLD).font_size(20.0)),
                action!(Button::new().style(ButtonStyle::Accent).on_click(choose_photos)).content("Add photos"),
            )),
            Border::new()
                .background(ThemeBrush::CardBackground).border_brush(ThemeBrush::CardStroke).border_thickness(1.0).corner_radius(8.0)
                .content(ScrollViewer::new()
                    .max_height(400.0)
                    .horizontal_scroll_bar_visibility(ScrollBarVisibility::Auto)
                    .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
                    .content(photos_el)
                ),
            "".into(),
        ];

        cx.window_title("Lock Screen Carousel");
        cx.window_visuals(
            WindowVisuals::new()
                .backdrop(WindowBackdrop::Mica)
                .client_size(1200.0, 800.0)
                .constraints(WindowConstraints {
                    min_width: Some(800.0),
                    min_height: Some(600.0),
                    max_width: None,
                    max_height: None,
                })
        );
        cx.on_window_size(cx.callback(Self::Message::Resize));

        StackPanel::new()
            .horizontal_alignment(HorizontalAlignment::Center)
            .children((
                TextBlock::new().text("Lock Screen Carousel").font_weight(FontWeight::SEMI_BOLD).font_size(28.0).margin(Thickness::xy(16.0, 16.0))
                    .width(self.window_size.width - 40.0)
                    .max_width(800.0),
                ScrollViewer::new()
                    .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
                    .width(self.window_size.width)
                    .max_height(self.window_size.height - 70.0)
                    .content(
                        StackPanel::new()
                            .width(self.window_size.width - 40.0).min_width(640.0).max_width(800.0).spacing(8.0).margin(Thickness::xy(16.0, 0.0))
                            .horizontal_alignment(HorizontalAlignment::Center)
                            .children(main_body)
                    ),
            ))
    }
}

fn main() {
    App::run_component::<AppComponent>(()).unwrap();
}
