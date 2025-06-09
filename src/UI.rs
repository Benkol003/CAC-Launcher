use std::{collections::HashMap, io::{ stdout, Stdout }};

use a2s::info::Info;
use anyhow::Error;
use ratatui::{
    layout::{ Constraint, Flex, Layout, Rect }, prelude::CrosstermBackend, 
    style::Stylize, symbols, text::{ Text, ToLine, ToText }, widgets::*, Terminal
};
use crossterm::{
    event::{ self, read, Event, KeyCode, KeyEventKind, MouseEventKind },
    ExecutableCommand,
};

use ratatui::{ prelude::*, widgets::* };

use crate::servers::{self, Server};

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal]).flex(Flex::Center).areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}

pub struct UI {
    pub term: Terminal<CrosstermBackend<Stdout>>,
}

impl UI {
    pub fn new() -> Self {
        std::io::stdout().execute(crossterm::event::EnableMouseCapture).unwrap();
        UI {
            term: Terminal::new(CrosstermBackend::new(stdout())).unwrap(),
        }
    }

    /// this function will block until user enters any key input to the popup prompt.
    /// border shrinks to fit lines of text. there is no limit on the maximum text line size.
    fn popup_blocking_prompt(&mut self, txt: &mut Text) {
        let block = Block::bordered();
        txt.push_line("press any key to continue...".to_line().white());
        let panel = Paragraph::new(txt.clone()).block(block).centered();
        self.term.clear();
        loop {
            self.term.draw(|x| {
                let width = panel.line_width() as u16;
                panel
                    .clone()
                    .render(
                        center(
                            x.area(),
                            Constraint::Length(width + 2),
                            Constraint::Length((txt.height() + 2) as u16)
                        ),
                        x.buffer_mut()
                    );
            });

            let event = read().unwrap();
            if event.is_key_press() {
                return;
            }
            if event.is_mouse() {
                let event = event.as_mouse_event().unwrap();
                if let MouseEventKind::Down(e) = event.kind {
                    return;
                }
            }
        }
    }

    pub fn warn_unkown_mod_state(&mut self) {
        let mut txt =
            "current mod state is unkown, assuming all mods are up to date.\n\
       If any mods are outdated, please redownload them later from the menu."
                .to_text()
                .light_yellow();
        self.popup_blocking_prompt(&mut txt);
    }

    pub fn main_menu<'a>(&self, titles: &'a Vec<&str>) -> Tabs<'a> {
        let titles2: Vec<_> = titles.iter().map(|s| s.to_line().green().bold()).collect();
        let titles_width = titles2.iter().fold(0, |a,x| a+x.width()) //length of titles strings 
            + ((titles.len()-2)*3) //chars used by delimiters
            +6; //chars used by borders
        let padding = self.term.size().unwrap().width.saturating_sub(titles_width as u16);
        let padding = Padding::new(padding/2, padding/2,0,0);
        let mut tabs = Tabs::new(titles2)
            .block(Block::bordered().border_style(Style::new().green()).title(" CAC Launcher ").title_bottom("(keys: \u{2190}/\u{2192})")
            .title_alignment(Alignment::Center)
            .padding(padding)
            )
            .highlight_style(Style::default().light_yellow())
            .select(0)
            .divider("|");
        tabs
    }

    fn server_status_table<'a>(&self,servers_status: Vec<(String,Option<Info>)>) -> Table<'a> {
        let ret = Table::new(
            servers_status.iter().map(|(k,v)| {
                Row::new(vec![k.clone(),
                match v {
                    Some(v) => {
                        format!("[{}/{}]",v.players,v.max_players)
                    }
                    None => {
                        "Offline".to_string()
                    }
                }
                ])
            }).collect::<Vec<Row>>(),
            [Constraint::Length(20),Constraint::Length(20)]
        );
        ret
    }


    pub async fn run(&mut self) -> Result<(),Error> {

        let servers = servers::read_config()?;

        let titles: Vec<_> = vec!(
            "Connect",
            "Update Mods",
            "Optional Mods",
            "Change User Profile",
            "Launcher Settings"
        );
        let mut select: usize = 0;

        //ratatui is an immediate mode gui. you should be constructing widget objects each loop so that 
        //widgets update wth changes e.g. new screen size 
        let mut status = servers::status(&servers).await?; //TODO do this in a loop, arc/rwlock
        status.sort_by_key(|(k,_)| k.clone());

        loop {
            let tabs = self.main_menu(&titles);
            let tabs = tabs.select(select);
            let term_size = self.term.size().unwrap();
            let status_table = self.server_status_table(status.clone());
            self.term.draw(|x| {
                tabs.render(Rect::new(0,0,x.area().width,std::cmp::min(term_size.height,3)), x.buffer_mut());
                x.render_widget(status_table, Rect::new(0,3,x.area().width,x.area().height));
            });

            

            let event = read().unwrap();
            if event.is_key_press() {
                let key = event.as_key_event().unwrap();
                if key.code == KeyCode::Left {
                    select = select.saturating_sub(1);
                } else if key.code == KeyCode::Right && select < titles.len() - 1 {
                    select += 1;
                }
            }
            if event.is_mouse() {
                let event = event.as_mouse_event().unwrap();
                if let MouseEventKind::Down(e) = event.kind {
                    return Ok(()); //TODO RM
                }
            }
        }
    }

}

impl Drop for UI {
    fn drop(&mut self) {
        self.term.clear();
        self.term.show_cursor();
        self.term.set_cursor_position((0, 0));
    }
}
