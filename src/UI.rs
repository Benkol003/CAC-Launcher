use std::{
    collections::HashMap,
    io::{ self, stdout, Stdout, Write },
    rc::Rc,
    sync::{ atomic::AtomicBool, Arc, Mutex },
    time::Duration,
};
use std::cmp::max;
use a2s::info::Info;
use anyhow::Error;
use indicatif::TermLike;
use ratatui::{
    layout::{ Alignment, Constraint, Flex, Layout, Position, Rect },
    prelude::CrosstermBackend,
    style::{ Style, Stylize },
    symbols,
    text::{ Text, ToLine, ToText },
    widgets::{ Block, Padding, Paragraph, Row, Table, TableState, Tabs, Widget },
    Terminal,
};
use crossterm::{
    cursor::SetCursorStyle,
    event::{ self, read, Event, KeyCode, KeyEvent, KeyEventKind, MouseEventKind },
    execute,
    terminal::disable_raw_mode,
    ExecutableCommand,
};

use std::cell::{ Cell, RefCell };

use crate::servers::{ self, Server };

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal]).flex(Flex::Center).areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}

/// helper struct for indicatif's `ProgressBar`, so can run this asynchronously of a UI widget.
/// take a ref to `buffer` before passing this struct to`ProgressBar::render_target`.
#[derive(Debug)]
pub struct ProgressBarBuffer {
    pub width: Mutex<Cell<u16>>,
    pub buffer: Arc<Mutex<RefCell<String>>>,
}
impl ProgressBarBuffer {
    pub fn new(width: u16) -> Self {
        ProgressBarBuffer {
            width: Mutex::new(Cell::new(100)),
            buffer: Arc::new(Mutex::new(RefCell::new(String::with_capacity(width as usize)))),
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
    fn move_cursor_down(&self, n: usize) -> io::Result<()> {
        Ok(())
    }
    fn move_cursor_right(&self, n: usize) -> io::Result<()> {
        Ok(())
    }
    fn move_cursor_left(&self, n: usize) -> io::Result<()> {
        Ok(())
    }
    fn write_line(&self, s: &str) -> io::Result<()> {
        self.write_str(s);
        Ok(())
    }
    fn write_str(&self, s: &str) -> io::Result<()> {
        //if ProgressBar tries to clear current line with empty line then just skip
        if s.len() == 0 || !s.contains(|i| i != ' ') {
            //TODO add own clear logic, e.g. message length is reduced then clear is needed
            return Ok(());
        }
        let buf_lock = self.buffer.lock().unwrap();
        buf_lock.replace(s.to_string());
        Ok(())
    }
    fn clear_line(&self) -> io::Result<()> {
        let lock = self.buffer.lock().unwrap();
        let s = lock.borrow_mut().clear();
        Ok(())
    }

    //dont actually flush here - ProgressBar prints a blank clear line then flushes so you won't print anything
    fn flush(&self) -> io::Result<()> {
        Ok(())
    }
}

pub struct TUI {
    pub term: Terminal<CrosstermBackend<Stdout>>,
}

/// UI elements that are aysn
impl TUI {
    pub fn new() -> Self {
        std::io::stdout().execute(crossterm::event::EnableMouseCapture).unwrap();
        let mut term = Terminal::new(CrosstermBackend::new(stdout())).unwrap();
        term.clear();
        TUI {
            term: term,
        }
    }

    pub fn popup_message(&mut self, message: &str) {
        let block = Block::bordered();
        let panel = Paragraph::new(message.clone()).block(block).centered();
        self.term.clear();

        self.term.draw(|x| {
            let rect = center(
                x.area(),
                Constraint::Length(
                    max((message.len() + 2) as u16, max(50, (panel.line_width() + 2) as u16))
                ),
                Constraint::Length(3 as u16) //TODO line wraps
            );
            panel.clone().render(rect, x.buffer_mut());
        });
    }

    pub fn popup_text_entry(&mut self, message: &str) -> String {
        let block = Block::bordered();

        // txt.push_line("Press C to cancel".to_line().white());
        // let panel = Paragraph::new(txt.clone()).block(block).centered();

        let mut cur = 0;
        let mut buf = String::new();
        self.term.clear();
        loop {
            let block = Block::bordered()
                .title_top(message.clone())
                .title_bottom("Press Enter to Submit")
                .title_alignment(Alignment::Center);
            let panel = Paragraph::new(buf.clone()).block(block);

            self.term.draw(|x| {
                let rect = center(
                    x.area(),
                    Constraint::Length(
                        max((message.len() + 2) as u16, max(50, (panel.line_width() + 2) as u16))
                    ),
                    Constraint::Length(3 as u16) //TODO line wraps
                );

                panel.clone().render(rect, x.buffer_mut());
                execute!(io::stdout(), SetCursorStyle::BlinkingBar);
                x.set_cursor_position(
                    Position::new(rect.left() + 1 + (cur as u16), (rect.top() + 1) as u16)
                );
            });

            let event = read().unwrap();
            if event.is_key_press() {
                let event = event.as_key_press_event().unwrap();
                match event.code {
                    KeyCode::Char(c) => {
                        buf.insert(cur, c);
                        cur += 1;
                    }
                    KeyCode::Enter => {
                        return buf;
                    }
                    KeyCode::Left => {
                        if cur > 0 {
                            cur -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if cur < buf.len() {
                            cur += 1;
                        }
                    }
                    KeyCode::Backspace => {
                        if cur > 0 {
                            buf.remove(cur - 1);
                            cur -= 1;
                        }
                    }
                    KeyCode::Delete => {
                        if cur >= 0 && cur < buf.len() {
                            buf.remove(cur);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    ///this function blocks until recieves finish signal or user requesta to cancel.
    /// # Returns:
    /// false if task was cancelled.
    pub fn popup_progress(
        &mut self,
        pbuf: Arc<Mutex<RefCell<String>>>,
        finish: Arc<AtomicBool>
    ) -> bool {
        let mut prev_len = 0;
        loop {
            let panel: Paragraph;
            let v: String;
            {
                let lock = pbuf.lock().unwrap();
                v = lock.borrow().clone();
            }
            let block = Block::bordered()
                .title_bottom("Press C to cancel")
                .title_alignment(Alignment::Center);
            if v.len() == 0 {
                log::warn!("detected ProgressBarBuffer len==0 (bug to fix)"); //TODO why is this happening...
                continue;
            }
            if v.len() != prev_len {
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
                            Constraint::Length(3 as u16)
                        ),
                        x.buffer_mut()
                    );
            });

            if crossterm::event::poll(Duration::from_millis(0)).unwrap() {
                let e = crossterm::event::read().unwrap();
                if e.is_key_press() {
                    let e = e.as_key_press_event().unwrap();
                    if e.code == KeyCode::Char('c') {
                        return false;
                    }
                }
            }
            if finish.load(std::sync::atomic::Ordering::Relaxed) {
                return true;
            }
        }
    }

    /// this function will block until user enters any key input to the popup prompt.
    /// border shrinks to fit lines of text. there is no limit on the maximum text line size.
    pub fn popup_blocking_prompt(&mut self, mut txt: Text) {
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
        self.popup_blocking_prompt(txt);
    }

    pub fn main_menu<'a>(&self, titles: &'a Vec<&str>) -> Tabs<'a> {
        let titles2: Vec<_> = titles
            .iter()
            .map(|s| s.to_line().green().bold())
            .collect();
        let titles_width =
            titles2.iter().fold(0, |a, x| a + x.width()) + //length of titles strings
            (titles.len() - 2) * 3 + //chars used by delimiters
            6; //chars used by borders
        let padding = self.term
            .size()
            .unwrap()
            .width.saturating_sub(titles_width as u16);
        let padding = Padding::new(padding / 2, padding / 2, 0, 0);
        let mut tabs = Tabs::new(titles2)
            .block(
                Block::bordered()
                    .border_style(Style::new().green())
                    .title(" CAC Launcher ")
                    .title_bottom("(keys: \u{2190}/\u{2192}), Esc to quit")
                    .title_alignment(Alignment::Center)
                    .padding(padding)
            )
            .highlight_style(Style::default().light_yellow())
            .select(0)
            .divider("|");
        tabs
    }

    fn server_status_table<'a>(&self, servers_status: Vec<(String, Option<Info>)>) -> Table<'a> {
        let ret = Table::new(
            servers_status
                .iter()
                .map(|(k, v)| {
                    Row::new(
                        vec![k.clone(), match v {
                            Some(v) => { format!("[{}/{}]", v.players, v.max_players) }
                            None => { "Offline".to_string() }
                        }]
                    )
                })
                .collect::<Vec<Row>>(),
            [Constraint::Length(20), Constraint::Length(20)]
        ).row_highlight_style(Style::default().fg(ratatui::style::Color::Yellow));
        ret
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        let servers = servers::read_config()?;

        let titles: Vec<_> = vec![
            "Connect",
            "Update Mods",
            "Optional Mods",
            "Change User Profile",
            "Launcher Settings"
        ];
        let mut tab_select: usize = 0;
        let mut srv_select = TableState::new().with_selected(0);

        //ratatui is an immediate mode gui. you should be constructing widget objects each loop so that
        //widgets update wth changes e.g. new screen size
        let mut status = servers::status(&servers).await?; //TODO do this in a loop, arc/rwlock
        status.sort_by_key(|(k, _)| k.clone());

        loop {
            let tabs = self.main_menu(&titles);
            let tabs = tabs.select(tab_select);
            let term_size = self.term.size().unwrap();

            let status_table = self.server_status_table(status.clone());

            self.term.draw(|x| {
                tabs.render(
                    Rect::new(0, 0, x.area().width, std::cmp::min(term_size.height, 3)),
                    x.buffer_mut()
                );
                x.render_stateful_widget(
                    status_table,
                    Rect::new(0, 3, x.area().width, x.area().height),
                    &mut srv_select
                );
            });

            let event = read().unwrap();
            if event.is_key_press() {
                let key = event.as_key_event().unwrap();
                if key.code == KeyCode::Left {
                    tab_select = tab_select.saturating_sub(1);
                } else if key.code == KeyCode::Right && tab_select < titles.len() - 1 {
                    tab_select += 1;
                } else if key.code == KeyCode::Up {
                    srv_select.scroll_up_by(1);
                } else if key.code == KeyCode::Down && srv_select.offset() < servers.len() {
                    srv_select.scroll_down_by(1);
                }else if key.code == KeyCode::Esc {
                    return Ok(());
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
        disable_raw_mode();
    }
}
