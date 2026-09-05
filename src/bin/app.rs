use std::cell::Cell;
use windows::core::*;
use windows_reactor::*;
use windows::Win32::{System::Com::*, UI::Shell::*};

use lock_screen_carousel::{CarouselChooseType, CarouselTrigger, registry::*, task_scheduler::*};

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
                CoTaskMemFree(path.0.is_null().then(move || path.0 as _));
                Some(path.to_hstring().to_string_lossy())
            }).collect::<Vec<_>>();
        }

        Ok(paths)
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

#[derive(Clone)]
enum AppMessage {
    Trigger(CarouselTrigger),
    Interval(u32),
    Collection(Vec<String>),
    Select(Option<usize>),
    Type(CarouselChooseType),
    Resize(WindowSize),
    Noop,
}

struct AppComponent {
    trigger: CarouselTrigger,
    interval: u32,
    collection: Vec<String>,
    collection_dirty: Cell<bool>,
    selected_photo: Option<usize>,
    choose_type: CarouselChooseType,
    window_size: WindowSize,
}

impl Component for AppComponent {
    type Input = ();
    type Message = AppMessage;

    fn create(_input: &Self::Input, _cx: &ComponentContext<Self>) -> Self {
        Self {
            trigger: get_carousel_trigger().unwrap_or(CarouselTrigger::Never),
            interval: get_carousel_interval().unwrap_or(15),
            collection: get_carousel_collection().unwrap_or_default(),
            collection_dirty: Cell::new(false),
            selected_photo: None,
            choose_type: get_carousel_choose_type().unwrap_or(CarouselChooseType::Iterate),
            window_size: WindowSize { width: 1200.0, height: 800.0 },
        }
    }

    fn update(&mut self, message: Self::Message, _cx: &ComponentContext<Self>) {
        match message {
            Self::Message::Trigger(trigger) => self.trigger = trigger,
            Self::Message::Interval(interval) => self.interval = interval,
            Self::Message::Collection(collection) => { self.collection = collection; self.collection_dirty.set(true) },
            Self::Message::Select(idx) => self.selected_photo = idx,
            Self::Message::Type(typ) => self.choose_type = typ,
            Self::Message::Resize(window_size) => self.window_size = window_size,
            _ => (),
        };
    }

    fn view(&self, _input: &Self::Input, cx: &mut ViewContext<Self>) -> View {
        {
            let trigger = self.trigger;
            let interval = self.interval;
            cx.use_effect("trigger", self.trigger, move || {
                set_carousel_trigger(trigger).unwrap();
                set_carousel_interval(interval).unwrap();
                schedule_task(trigger, interval).unwrap();
                None
            });
            let trigger = self.trigger;
            let interval = self.interval;
            cx.use_effect("interval", self.interval, move || {
                set_carousel_interval(interval).unwrap();
                schedule_task(trigger, interval).unwrap();
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
            let collection = self.collection.clone();
            cx.callback(move |()| {
                if let Ok(photos) = open_file_dialog() {
                    return Self::Message::Collection([collection.clone(), photos].concat());
                }
                Self::Message::Noop
            })
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
                        let collection = self.collection.clone();
                        let remove_item = move |idx| {
                            collection.iter().enumerate().filter_map(|(i, s)| (i != idx).then(|| s.clone())).collect()
                        };
                        let border_view = Border::new()
                            .content(item_row().column_spacing(12.0).margin(Thickness::xy(0.0, 4.0)).children((
                                label!(TextBlock::new().text(s.clone()).margin(4.0)),
                                action!(Button::new().margin(4.0).on_click(cx.callback(move |()| Self::Message::Collection(remove_item(idx))))).content(TextBlock::new().text("Remove").font_size(12.0))),
                            ));
                        KeyedView::new(s.clone(), ListViewItem::new().tag(s).content(border_view))
                    }))
        };

        let trigger_texts = ["Never", "Timer", "Session lock"];
        let mut lock_trigger_card_children = vec![
            KeyedView::new("trigger_label", label!("Lock screen change trigger").grid_row(0)),
            KeyedView::new(
                "trigger_action",
                action!(DropDownButton::new()).grid_row(0)
                    .content(trigger_texts[self.trigger as usize])
                    .menu(Menu::new(
                        trigger_texts.iter().map(|t| MenuItem::item(*t, *t)),
                        cx.callback(move |item_text: String| {
                            for (i, text) in trigger_texts.iter().enumerate() {
                                if item_text == *text {
                                    return Self::Message::Trigger((i as u32).into());
                                }
                            }
                            Self::Message::Noop
                        })
                    ))
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
                        TextBlock::new().text(" minutes").vertical_alignment(VerticalAlignment::Center)
                    ))
                ),
            ]);
        };

        let choose_labels = ["Next in list", "Random"];
        let main_body = [
            item_row().children((
                label!(TextBlock::new().text("Photos").font_weight(FontWeight::SEMI_BOLD).font_size(20.0)),
                action!(Button::new().style(ButtonStyle::Accent).on_click(choose_photos)).content("Add photos")
            )),
            Border::new()
                .background(ThemeBrush::CardBackground).border_brush(ThemeBrush::CardStroke).border_thickness(1.0).corner_radius(8.0)
                .content(ScrollViewer::new()
                    .max_height(400.0)
                    .horizontal_scroll_bar_visibility(ScrollBarVisibility::Auto)
                    .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
                    .content(photos_el)
                ),
            TextBlock::new().text("Settings").font_weight(FontWeight::SEMI_BOLD).font_size(20.0).into(),
            card(item_row().children((
                label!("Choose next background by..."),
                action!(DropDownButton::new())
                    .content(choose_labels[self.choose_type as usize])
                    .menu(Menu::new(
                        choose_labels.iter().map(|t| MenuItem::item(*t, *t)),
                        cx.callback(move |item_text: String| {
                            for (i, text) in choose_labels.iter().enumerate() {
                                if item_text == *text {
                                    return Self::Message::Type((i as u32).into());
                                }
                            }
                            Self::Message::Noop
                        })
                    ))
            ))),
            card(item_row()
                .rows(vec![GridLength::Auto; if self.trigger == CarouselTrigger::Interval { 3 } else { 1 }])
                .row_spacing(12.0)
                .keyed_children(lock_trigger_card_children)
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
            .max_width(self.window_size.width - 40.0)
            .children((
                TextBlock::new().text("Lock Screen Carousel").font_weight(FontWeight::SEMI_BOLD).font_size(28.0).margin(Thickness::xy(16.0, 16.0)),
                ScrollViewer::new()
                    .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
                    .max_height(self.window_size.height - 70.0)
                    .content(StackPanel::new().min_width(640.0).spacing(8.0).margin(Thickness::xy(16.0, 0.0)).children(main_body))
            ))

    }
}

fn main() {
    App::run_component::<AppComponent>(()).unwrap();
}
