use std::{
    collections::HashMap, fmt::Display, fs::OpenOptions, io::{ self, stdout, Stdout, Write }, path::PathBuf, rc::Rc, sync::{ atomic::AtomicBool, Arc, Mutex }, time::Duration
};
use std::cmp::{max,min};
use a2s::info::Info;
use anyhow::{anyhow, Error};
use base64::display;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle, TermLike};
use log::{error, warn};
use ratatui::{
    layout::{ Alignment, Constraint, Flex, Layout, Position, Rect },
    prelude::CrosstermBackend,
    style::{ Color, Style, Stylize },
    symbols,
    text::{ Line, Text, ToLine, ToSpan, ToText },
    widgets::{ Block, Padding, Paragraph, Row, Table, TableState, Tabs, Widget },
    Terminal,
};
use crossterm::{
    cursor::SetCursorStyle,
    event::{ self, read, Event, KeyCode, KeyEvent, KeyEventKind, MouseEventKind },
    execute,
    terminal::{disable_raw_mode, Clear},
    ExecutableCommand,
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use std::cell::{ Cell, RefCell };

use crate::{configs::{CACConfig, CACContent, Links}, msgraph, servers::{ self, Server }, unzip, ClientCtx, CONFIG_FILE, PROGRESS_STYLE_DOWNLOAD, TMP_FOLDER};

pub const LOGO: &str =
r###"
_____          _____ _                            _               
/ ____|   /\   / ____| |  discord.gg/dNGcyEYK8F   | |              
| |       /  \ | |    | |     __ _ _   _ _ __   ___| |__   ___ _ __ 
| |      / /\ \| |    | |    / _` | | | | '_ \ / __| '_ \ / _ \ '__|
| |____ / ____ \ |____| |___| (_| | |_| | | | | (__| | | |  __/ |   
\_____/_/    \_\_____|______\__,_|\__,_|_| |_|\___|_| |_|\___|_|   
    Made by UnladenCoconut 

"###;

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal]).flex(Flex::Center).areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}

/// helper struct for indicatif's `ProgressBar`, so can run this asynchronously of a UI widget.
/// take a ref to `buffer` before passing this struct to`ProgressBar::render_target`.
#[derive(Debug,Clone)]
pub struct ProgressBarBuffer {
    pub width: Arc<Mutex<Cell<u16>>>,
    pub buffer: Arc<Mutex<RefCell<String>>>,
}
impl ProgressBarBuffer {
    //TODO: width - can change if term gets resized so just let grow to whatever
    pub fn new() -> Self {
        ProgressBarBuffer {
            width: Arc::new(Mutex::new(Cell::new(500))),
            buffer: Arc::new(Mutex::new(RefCell::new(String::with_capacity(500 as usize)))),
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

struct ServerMenu {
    servers: Vec<(String,Server)>,
    status: Vec<(String, Option<Info>)>,
    select: TableState,
}

impl ServerMenu {

    /// constructs the widget to render
    fn make<'a>(&self) -> Table<'a> {
        let ret = Table::new(
            self.status
                .iter()
                .map(|(k, v)| {
                    Row::new(
                        vec![k.clone(), match v {
                            Some(v) => { format!("[{}/{}]",v.players, v.max_players) }
                            None => { "[Offline]".into() }
                        }]
                    )
                })
                .collect::<Vec<Row>>(),
            [Constraint::Length(self.status.iter().fold(13, |acc,x| std::cmp::max(acc,x.0.len())) as u16),Constraint::Fill(1)]
        ).row_highlight_style(Style::default().fg(Color::Black).bg(Color::Rgb(66, 149, 0xff))).header(Row::new(["(launch: \u{2191}/\u{2193})"]).style(Style::new().fg(Color::LightYellow).bold()));
        ret
    }

    /// returns false if should quit i.e. if launched arma 
    fn key_handler(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Up{
            self.select.select_previous();
        }else if key.code == KeyCode::Down {
            self.select.select_next();
        }else if key.code == KeyCode::Enter {
            self.servers.get(self.select.selected().unwrap()).unwrap().1.launch();
            return false;
        }
        true
    }
}

struct UpdateModsMenu {

}

impl UpdateModsMenu {
    fn make() {
        let titles = vec![
            "Update all mods",
            "Update mods for server",
            "Redownload mods",

        ];
    }
}

struct OptionalModsMenu {
    titles: Vec<(String,OptionalModsStatus)>,
    select: TableState
}

enum OptionalModsStatus {
    enabled,
    disabled,
    not_found
}

impl Display for OptionalModsStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            Self::enabled => {f.write_str("enabled")},
            Self::disabled => {f.write_str("disabled")},
            Self::not_found => {f.write_str("not found")}
        }
    }
}


impl OptionalModsMenu {
    fn new() -> Result<Self,Error>{
        let content = CACContent::read()?;
        
        let mut titles: Vec<(String,OptionalModsStatus)> = content.optionals.iter().map(|x| (x.0.clone(),OptionalModsStatus::disabled)).collect();
        Ok(Self {titles: titles, select: TableState::new().with_selected(0)})
        }

    pub fn make(&mut self) -> Result<Table,Error> {
        let content = CACContent::read()?;
        let config = CACConfig::read()?;
        config.enabled_optionals.iter().for_each(|x| {
            if !content.optionals.contains_key(x){
                warn!("Enabled optional mod '{}' is not present in the content manifest",x);
            }else{
                self.titles.iter_mut().find(|y| y.0 == *x).unwrap().1 = OptionalModsStatus::enabled;
            }
        });

        self.titles.iter_mut().map(|x| -> Result<_,Error>{
             if !config.absolute_mod_dir()?.join(&x.0).is_dir() {
                x.1 = OptionalModsStatus::not_found;
            } Ok(())
        }).collect::<Result<_,Error>>()?; //collect and propogate error if file_exists fails
        
        Ok(Table::new(self.titles.iter().map(|x| Row::new(vec![x.0.clone(),x.1.to_string()])),
        [Constraint::Length(self.titles.iter().fold(13, |acc,x| std::cmp::max(acc,x.0.len())) as u16),Constraint::Fill(1)]

        ).header(Row::new(vec!["Select","(\u{2191}/\u{2193},Enter: enable/disable, download if not found)"]).style(Style::new().fg(Color::LightYellow).bold()))
        .row_highlight_style(Style::default().fg(Color::Black).bg(Color::Rgb(66, 149, 0xff)))
        )
    }

    pub async fn key_handler(&mut self, ui: &mut TUI, key: KeyEvent) -> Result<(),Error>{

        /// TODO: enable/disable all mods

        if key.code == KeyCode::Up {
            self.select.select_previous();
        }else if key.code ==KeyCode::Down && self.select.selected().unwrap() < self.titles.len()-1 {
            self.select.select_next();
        }else if key.code == KeyCode::Enter {
            let entry = self.titles.get_mut(self.select.selected().unwrap()).unwrap(); 
            let mut config = CACConfig::read()?;
            match entry.1{
                OptionalModsStatus::not_found => {

                    //TODO one point of reference?
                    let content = CACContent::read()?;
                    let content_map =  content.content_map();
                    let links  =content_map.get(&entry.0).unwrap();
                    
                    let join = ui.popup_download((*links).clone()).await;
                    //TODO check indicates download fail

                    if join.is_ok() { //if download mod

                    }

                    match join {
                        Ok(b) => {
                            if(b){
                                config.enabled_optionals.insert(entry.0.clone());
                                entry.1=OptionalModsStatus::enabled;
                            }else{
                                ;//was cancelled by user
                            }
                        }
                        Err(e) => {
                            ui.popup_blocking_prompt(Line::from(vec!["download failed:".light_red(),e.to_span()]).to_text());
                            error!("download of {} failed: {}",entry.0,e);
                        }
                    }
                },
                OptionalModsStatus::enabled => { 
                    if!(config.enabled_optionals.remove(&entry.0)){
                        return Err(anyhow!("failed to disable mod {} from the config",&entry.0));
                    }
                    entry.1 = OptionalModsStatus::disabled; 
                },
                OptionalModsStatus::disabled => {
                    config.enabled_optionals.insert(entry.0.clone());
                    entry.1=OptionalModsStatus::enabled;
                }

            }
            let f = OpenOptions::new().write(true).truncate(true).open(CONFIG_FILE.as_path())?;
            serde_json::to_writer_pretty(f, &config);
        }
        Ok(())
    }
}

struct LauncherSettingsMenu {

}

impl LauncherSettingsMenu {
    
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
        let lines: Vec<_> = message.split('\n').collect();
        let text  = Text::from(lines.iter().map(|x| x.to_line()).collect::<Vec<_>>());
        let panel = Paragraph::new(text).block(block).centered();
        self.term.clear();

        self.term.draw(|x| {
            let rect = center(
                x.area(),
                Constraint::Length(
                    max((message.len() + 2) as u16, max(50, (panel.line_width() + 2) as u16))
                ),
                Constraint::Length(lines.len() as u16 + 2) //TODO line wraps
            );
            panel.clone().render(rect, x.buffer_mut());
        });
    }

    //TODO: popups dont have bind to exit
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
        self.term.clear();
    }

    ///this function blocks until recieves finish signal or user request to cancel.
    /// intended for handling downloads
    /// # Returns:
    /// false if task was cancelled.
    pub fn popup_progress(
        &mut self,
        pbuf: Arc<Mutex<RefCell<String>>>,
        finish: CancellationToken
    ) -> bool {
        let mut prev_len = 0;
        loop {
            if finish.is_cancelled() {
                return true;
            }
        
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
                log::info!("detected ProgressBarBuffer len==0 (bug to fix)"); //TODO why is this happening...
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
        }
        self.term.clear();
    }

    /// popup to wrap download + unzip mod
    /// # Return:
    /// indicates download failed by returning Error
    pub async fn popup_download(&mut self, links: Links) -> Result<bool,Error> {
        warn!("UI: entered popup_download");

        let term_size = self.term.size()?;
        let mut progressBuf = ProgressBarBuffer::new(); 
        let mut pbuf = progressBuf.buffer.clone();
        let mut progress = ProgressBar::new((term_size.width /2) as u64).with_style(ProgressStyle::with_template("{spinner} {msg:.green.bold}")?);
        progress.set_length(1); progress.set_position(0);
        progress.set_draw_target(ProgressDrawTarget::term_like(Box::new(progressBuf)));
        
        let _finish = CancellationToken::new();
        let finish = _finish.clone();

        let join: JoinHandle<Result<(),Error>>  =tokio::spawn(async move {

            progress.set_message(" Fetching info... ");

            let config = CACConfig::read()?;
            let client_ctx = ClientCtx::build()?; //TODO initialise elsewhere
            let token = msgraph::login(&client_ctx.client).await?;

            let mut archive0: Option<PathBuf> = None;
            for link in links.into_iter() {
                warn!("popup_download: downloading item: {}",link);
                let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(),link.to_string() ).await?;
                let file = msgraph::download_item(client_ctx.client.clone(), token.clone(),item, TMP_FOLDER.to_str().unwrap().to_string(), &mut progress, finish.clone()).await?;
                if(archive0.is_none()){
                    archive0 = Some(file);
                }
            }
            unzip(archive0.unwrap().to_str().unwrap(),config.absolute_mod_dir()?.to_str().unwrap(),Some(&mut progress))?;
            finish.cancel();
            warn!("popup_download: async move finished ok");
            Ok(())
        });

        let ret = if !self.popup_progress(pbuf, _finish.clone()){
            _finish.cancel();
            false
        }else {
            true
        };

        join.await?;
        warn!("UI:popup_download ok");
        self.term.clear();
        Ok(ret)
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
        self.term.clear();
    }

    pub fn warn_unknown_mod_state(&mut self) {
        let mut txt =
            "current mod state is unknown, assuming all mods are up to date.\n\
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

        //ratatui is an immediate mode gui. you should be constructing widget objects each loop so that
        //widgets update wth changes e.g. new screen size
        let mut _status = servers::status(&servers).await?; //TODO do this in a loop, arc/rwlock as wont update server status whilst running
        //TODO put offline servers at the bottom
        _status.sort_by_key(|(k, _)| k.clone());

        let mut server_menu = ServerMenu {
            servers: servers,
            status: _status,
            select: TableState::new().with_selected(0)
        };

        //let mut update_mods_menu = UpdateModsMenu::new();
        let mut optional_mods_menu  = OptionalModsMenu::new()?;
        //let mut launcher_settings_menu = LauncherSettingsMenu::new();
        
        loop {
            let tabs = self.main_menu(&titles);
            let tabs = tabs.select(tab_select);
            let term_size = self.term.size().unwrap();

            self.term.draw(|x| {
                tabs.render(
                    Rect::new(0, 0, x.area().width, min(term_size.height, 3)),
                    x.buffer_mut()
                );
                //render tab menus 
                match titles[tab_select] {
                    "Connect" => {
                        x.render_stateful_widget(server_menu.make(), Rect::new(0,3,term_size.width,term_size.height.saturating_sub(3)), &mut server_menu.select);
                    }
                    "Update Mods" => {
                        
                    }
                    "Optional Mods"  => {
                        let mut s = optional_mods_menu.select.clone(); 
                        x.render_stateful_widget(optional_mods_menu.make().unwrap(), Rect::new(0,3,term_size.width,term_size.height.saturating_sub(3)), &mut s);
                    }
                    "Change User Profile"  => {}
                    "Launcher Settings"  => {}
                    _ => {}
                }
            });

            let event = read().unwrap();

            if event.is_key_press() {
                let key = event.as_key_event().unwrap();
                if key.code == KeyCode::Left {
                    tab_select = tab_select.saturating_sub(1);
                } else if key.code == KeyCode::Right && tab_select < titles.len() - 1 {
                    tab_select += 1;
                }else if key.code == KeyCode::Esc {
                    return Ok(());
                } 

                match titles[tab_select] {
                    "Connect" => {
                        if !server_menu.key_handler(key) {return Ok(());}
                    }
                    "Update Mods" => {

                    }
                    "Optional Mods" => {
                        optional_mods_menu.key_handler(self,key).await;
                    }
                    "Change User Profile" => {
                        if key.code == KeyCode::Up {

                        }else if key.code == KeyCode::Down {

                        }
                    }
                    "Launcher Settings" => {
                        if key.code == KeyCode::Up {

                        }else if key.code == KeyCode::Down {

                        }
                    }
                    _ => {}
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
