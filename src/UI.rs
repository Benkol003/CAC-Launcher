use std::{collections::HashMap, io::{ self, stdout, Stdout, Write }, rc::Rc, sync::{atomic::AtomicBool, Arc, Mutex}, time::Duration};

use a2s::info::Info;
use anyhow::Error;
use indicatif::TermLike;
use ratatui::{
    layout::{ Alignment, Constraint, Flex, Layout, Rect }, prelude::CrosstermBackend, style::{Style, Stylize}, symbols, text::{ Text, ToLine, ToText }, widgets::{Block, Padding, Paragraph, Row, Table, Tabs, Widget}, Terminal
};
use crossterm::{
    event::{ self, read, Event, KeyCode, KeyEventKind, MouseEventKind },
    ExecutableCommand,
};

use std::cell::{Cell,RefCell};

use crate::servers::{self, Server};

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal]).flex(Flex::Center).areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}

/// helper struct for indicatif's ProgressBar, so 
/// 
#[derive(Debug)]
pub struct ProgressBarBuffer {
        pub width: Mutex<Cell<u16>>,
        pub buffer: Arc<Mutex<RefCell<String>>>,
}
impl ProgressBarBuffer {
    pub fn new(width: u16) -> Self {
        ProgressBarBuffer {
            width: Mutex::new(Cell::new(100)),
            buffer: Arc::new(Mutex::new(RefCell::new(String::with_capacity(width as usize))))
        }
    }
    pub fn set_width(&self, width: u16) -> () {
        let lock = self.width.lock().unwrap();
        lock.set(width);
    }
}
impl TermLike for ProgressBarBuffer {
    /// if the width isnt large enough then ProgressBar will simply refuse to print
    fn width(&self) -> u16 {
        let lock = self.width.lock().unwrap();
        lock.get()
    }
    fn height(&self) -> u16 {
        1
    }
    fn move_cursor_up(&self, n: usize) -> io::Result<()> {
        Ok(())
    }
    fn move_cursor_down(&self, n: usize) -> io::Result<()>{
        Ok(())
    }
    fn move_cursor_right(&self, n: usize) -> io::Result<()>{
        Ok(())
    }
    fn move_cursor_left(&self, n: usize) -> io::Result<()>{
        Ok(())
    }
    fn write_line(&self, s: &str) -> io::Result<()>{
        self.write_str(s);
        Ok(())
    }
    fn write_str(&self, s: &str) -> io::Result<()>{ 
        //if ProgressBar tries to clear current line with empty line then just skip
        if(s.len()==0 || !s.contains(|i| i!=' ')) {
            //TODO add own clear logic, e.g. message length is reduced then clear is needed
            return Ok(());
        }
        let buf_lock = self.buffer.lock().unwrap();
        buf_lock.replace(s.to_string());
        Ok(())
    }
    fn clear_line(&self) -> io::Result<()>{
        let lock = self.buffer.lock().unwrap();
        let s = lock.borrow_mut().clear();
        Ok(())
    }

    //dont actually flush here - ProgressBar prints a blank clear line then flushes so you won't print anything
    fn flush(&self) -> io::Result<()>{
        Ok(())
    }
}

pub struct TUI {
    pub term: Terminal<CrosstermBackend<Stdout>>,
}

impl TUI {
    pub fn new() -> Self {
        std::io::stdout().execute(crossterm::event::EnableMouseCapture).unwrap();
        let mut term = Terminal::new(CrosstermBackend::new(stdout())).unwrap();
        term.clear();
        TUI {
            term: term,
        }
    }

    /// blocking. TODO REMOVE
    pub fn popup_progress(&mut self, pbuf: Arc<Mutex<RefCell<String>>>,finish: Arc<AtomicBool>) {
        let mut prev_len =0;
        loop {
            let panel: Paragraph;
            let v: String;
            {
                let lock = pbuf.lock().unwrap();
                v = lock.borrow().clone();
            }
            let block = Block::bordered();
            if v.len()==0 {
                    log::warn!("detected ProgressBarBuffer len==0 (bug to fix)"); //TODO why is this happening...
                    continue;
            }
            if v.len()!=prev_len {
                self.term.clear();
            }
            prev_len = v.len();

                panel = Paragraph::new(v).block(block).centered();
            self.term.draw(|x| {
                let width = panel.line_width() as u16;
                panel
                    .clone()
                    .render(
                        center(
                            x.area(),
                            Constraint::Length(width + 2),
                            Constraint::Length((3) as u16)
                        ),
                        x.buffer_mut()
                    );
            });

            if crossterm::event::poll(Duration::from_millis(0)).unwrap(){
                let e = crossterm::event::read().unwrap();
                if e.is_key_press() {
                return;
                }
            }
            if finish.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
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

impl Drop for TUI {
    fn drop(&mut self) {
        self.term.clear();
        self.term.show_cursor();
        self.term.set_cursor_position((0, 0));
    }
}
