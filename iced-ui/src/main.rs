use std::error::Error;
use iced::{Border, Color, Element, Font, Length::{self, Fill}, Shadow, Theme, Vector, alignment, border::Radius, color, font, widget::{Button, image, Row, Rule, Space, Text, button::{self, Style}, column, container, keyed::column, row, rule, text}};
use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString, IntoStaticStr};
use iced_gif::{Frames, widget::gif};
use src_backend;

/*
- View() is only called on state change, not every render frame.
*/

lazy_static!{ 
    static ref LOGO: Frames = Frames::from_bytes(include_bytes!("logo.gif").to_vec()).unwrap();
}

const LOGO_ASCII: &'static str = include_str!("logo_ascii.txt");

#[derive(Debug,Clone)]
enum Message {
    MainMenu(u64),
}

#[derive(Default)]
struct State {
    selected_menu: u64,
}

const fn font() -> Font {
    Font {
        family: font::Family::Name("JetBrains Mono"),
        weight: font::Weight::Normal,
        stretch: font::Stretch::Normal,
        style: font::Style::Normal
    }
}

fn main() -> Result<(),Box<dyn Error>>{
    iced::application("CAC Launcher", update, view)
    .default_font(font())
    .run()?;
    Ok(())
}

fn update(state: &mut State, message: Message ) -> (){
    match message {
        Message::MainMenu(i) => {
            state.selected_menu = i;
        }
    }
}

#[derive(EnumIter,Display)]
enum MENUS {
    #[strum(to_string = "Main Menu")]
    MainMenu,
    #[strum(to_string = "Update Mods")]
    UpdateMods,
    #[strum(to_string = "Optional Mods")]
    OptionalMods,
    #[strum(to_string = "Launcher Settings")]
    LauncherSettings
}

fn menu_select(select_id: u64) -> Message {
    Message::MainMenu(select_id)
}

fn view(state: &State) -> Element<Message> {
    column!(
    Space::with_height(32),
    row!(
        Space::with_width(Length::FillPortion(1)),
        container(gif(&LOGO)).width(Length::FillPortion(1)),
        //container(image::Image::new(image::Handle::from_bytes(LOGO)).height(96).width(96)).align_x(alignment::Horizontal::Center).width(Length::Fill)
        container(text(LOGO_ASCII).align_x(alignment::Horizontal::Center).align_y(alignment::Vertical::Center).font(AppStyle::title_font()).size(12).style(AppStyle::title)).width(Length::FillPortion(3)),
        //Space::with_width(Length::FillPortion(1)),
    ).width(Length::Fill),
    Space::with_height(32),
    
    MENUS::iter().into_iter().enumerate().fold(Row::new(),
    |menu, (i,name)| menu.push(container(Row::new()
        .push(Space::with_width(Length::FillPortion(1)))
        .push(Button::new(Text::new(name.to_string())
            .align_x(alignment::Horizontal::Center))
            .width(Length::FillPortion(6))
            .on_press(Message::MainMenu(i as u64))
            .style(AppStyle::menu_button(state.selected_menu==(i as u64))))
        .push(Space::with_width(Length::FillPortion(1))))
    .style(AppStyle::menu)))
    ).into()
}

struct AppStyle {
}

impl AppStyle {

    fn menu_button(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
        move |_: &Theme, _: button::Status| -> button::Style {
            let green1 = color!(0,255,0);
            let green2 = color!(0,216,0);
            let green = match selected {
                true => {
                    green1
                }   
                false => {
                    green2
                }
            };
            
            button::Style {
                text_color: Color::BLACK, 
                background: Some(iced::Background::Color(green)), 
                border: Border {color: green,width: 6.0, radius: Radius::new(6.0)}, 
                shadow: Shadow {color: Color::BLACK, offset: Vector::ZERO, blur_radius: 0.0} 
            }
        }
    }

    fn title_font() -> Font {
        let mut font = Font::MONOSPACE; font.weight = font::Weight::Bold;font
    }

    fn title(_: &Theme) -> text::Style {
        text::Style {
            color: Some(color!(0,255,0))
        }
    }

    fn menu( _: &Theme) -> container::Style {
        container::Style { text_color: Some(Color::BLACK), 
            background: None, 
            border: Border {color: Color::BLACK,width: 0.0, radius: Radius::new(0)}, 
            shadow: Shadow {color: Color::BLACK, offset: Vector::ZERO, blur_radius: 0.0} }
    }
}