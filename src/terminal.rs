use termion;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::color::AnsiValue;
use std::io::{stdout, stdin, Write};
use std::error::Error;
use std::thread;
//use std::sync::mpsc;
use crossbeam::channel;
use std::collections::VecDeque;

#[derive(Copy, Clone)]
struct ColoredChar {
    ch: char,
    foreground: AnsiValue,
    background: AnsiValue,
}

pub struct Scene([[ColoredChar;SCREEN_H as usize];SCREEN_W as usize]);

impl Default for Scene {
    fn default() -> Scene {
        let c = ColoredChar {
            ch: ' ',
            foreground: AnsiValue::rgb(5, 5, 5),
            background: AnsiValue::rgb(0, 0, 0),
        };
        Scene([[c; SCREEN_H as usize]; SCREEN_W as usize])
    }
}
impl Scene {
    pub fn set_point(&mut self, sx: i32, sy: i32, ch: char, foreground: AnsiValue, background: Option<AnsiValue>) {
        if !(sx < 0 || sy < 0 || sx >= SCREEN_W as i32 || sy >= SCREEN_H as i32) {
            let sx = sx as usize; let sy = sy as usize;
            let background = background.unwrap_or(self.0[sx][sy].background);
            self.0[sx][sy] = ColoredChar { ch, foreground, background }
        }
    }
    pub fn write(&mut self, text: String, sx: i32, sy: i32) {
        for (ix, ch) in text.char_indices() {
            self.set_point(sx + ix as i32, sy, ch, AnsiValue::rgb(5, 5, 5), Some(AnsiValue::rgb(0, 0, 0)));
        }
    }
}

enum TerminalCommand {
    Println(String),
    SetQuery(Option<String>),
    AddReplyChar(char),
    Backspace,
    FinishReply(channel::Sender<String>),
    DrawScene(Box<Scene>),
}
enum InputCommand {
    Query(String, channel::Sender<String>),
    GetEvent(channel::Sender<termion::event::Event>),
}

#[derive(Clone)]
pub struct Terminal {
    control: channel::Sender<TerminalCommand>,
    input: channel::Sender<InputCommand>,
}
impl Terminal {
    pub fn new() -> Self {
        let (ttx, trx) = channel::unbounded();
        let (itx, irx) = channel::unbounded();
        let ttx2 = ttx.clone();
        let term = Terminal {
            control : ttx,
            input : itx,
        };
        thread::spawn(move || {
            terminal_thread(trx)
        });
        thread::spawn(move || {
            input_thread(irx, ttx2)
        });
        term
    }
    pub fn println<S: ToString>(&self, line: S) -> Result<(), Box<dyn Error>> {
        self.control.send(TerminalCommand::Println(line.to_string()))?;
        Ok(())
    }
    pub fn readln<S: ToString>(&self, query: S) -> Result<String, Box<dyn Error>> {
        let (rtx, rrx) = channel::unbounded();
        self.input.send(InputCommand::Query(query.to_string(), rtx))?;
        Ok(rrx.recv()?)
    }
    pub fn get_ev(&self) -> Result<termion::event::Event, Box<dyn Error>> {
        let (rtx, rrx) = channel::unbounded();
        self.input.send(InputCommand::GetEvent(rtx))?;
        Ok(rrx.recv()?)
    }
    pub fn draw_scene(&self, scene: Box<Scene>) -> Result<(), Box<dyn Error>> {
        self.control.send(TerminalCommand::DrawScene(scene))?;
        Ok(())
    }
}

pub const SCREEN_W: u16 = 61;
pub const HEIGHT: u16 = 30;
pub const TERM_H: u16 = HEIGHT - 3;
pub const SCREEN_H: u16 = HEIGHT - 3;
pub const TERM_W: u16 = 30;

struct TerminalState {
    console_out: VecDeque<String>,
    query: Option<String>,
    reply: String,
    scene: Scene,
}
impl TerminalState {
    fn render(&mut self) {
        print!("{}{}", AnsiValue::rgb(5,5,5).fg_string(), AnsiValue::rgb(0, 0, 0).bg_string());
        print!("{}╔", termion::cursor::Goto(1, 1));
        for i in 0 .. SCREEN_W+1+TERM_W {
            print!("{}", if i != SCREEN_W { "═" } else { "╤" });
        }
        print!("╗");
        for i in 0 .. HEIGHT {
            print!("{}", termion::cursor::Goto(1, 1+i+1));
            print!("{}", if i != HEIGHT - 3 { "║" } else { "╟" });
            print!("{}", termion::cursor::Goto(1+1+SCREEN_W+1+TERM_W, 1+i+1));
            print!("{}", if i != HEIGHT - 3 { "║" } else { "╢" });
            if i < HEIGHT - 3 {
                print!("{}│", termion::cursor::Goto(1+1+SCREEN_W, 1+i+1));
            }
            else if i == HEIGHT - 3 {
                print!("{}", termion::cursor::Goto(1+1, 1+i+1));
                for j in 0 .. SCREEN_W+1+TERM_W {
                    print!("{}", if j != SCREEN_W { "─" } else { "┴" });
                }
            }
        }
        print!("{}╚", termion::cursor::Goto(1, 2+HEIGHT));
        for _ in 0 .. SCREEN_W+1+TERM_W {
            print!("═");
        }
        print!("╝");
        self.finish_render();
        self.render_console();
        self.render_query();
        self.render_scene();
    }
    fn render_console(&mut self) {
        print!("{}{}", AnsiValue::rgb(5,5,5).fg_string(), AnsiValue::rgb(0, 0, 0).bg_string());
        for i in 0 .. TERM_H {
            print!("{}", termion::cursor::Goto(1+1+SCREEN_W+1, 1+1+i));
            let mut rem = TERM_W;
            if self.console_out.len() >= (TERM_H - i) as usize {
                let line = &self.console_out[i as usize - (TERM_H as usize - self.console_out.len())];
                print!("{}", line);
                rem -= line.len() as u16;
            }
            for _ in 0 .. rem {
                print!(" ")
            }
        }
        self.finish_render();
    }
    fn render_query(&mut self) {
        print!("{}{}", AnsiValue::rgb(5,5,5).fg_string(), AnsiValue::rgb(0, 0, 0).bg_string());
        print!("{}", termion::cursor::Goto(1+1, 1+1+SCREEN_H+1));
        let mut rem = (SCREEN_W+1+TERM_W) as usize;
        match &self.query {
            None => {}
            Some(query) => {
                print!("{}", query);
                rem -= query.len();
            }
        }
        for _ in 0 .. rem {
            print!(" ");
        }
        print!("{}", termion::cursor::Goto(1+1, 1+1+SCREEN_H+2));
        rem = (SCREEN_W+1+TERM_W) as usize;
        if self.query.is_some() {
            rem -= 2;
            print!("> ");
            print!("{}", self.reply);
            rem -= self.reply.len();
        }
        for _ in 0 .. rem {
            print!(" ");
        }
        self.finish_render();
    }
    fn render_scene(&mut self) {
        let Scene(charmap) = self.scene;
        for y in 0 .. SCREEN_H as usize {
            print!("{}", termion::cursor::Goto(1+1, 1+1+y as u16));
            for x in 0 .. SCREEN_W as usize {
                print!("{}{}{}", charmap[x][y].foreground.fg_string(), charmap[x][y].background.bg_string(), charmap[x][y].ch);
            }
        }
        print!("{}{}", AnsiValue::rgb(5,5,5).fg_string(), AnsiValue::rgb(0, 0, 0).bg_string());
        self.finish_render();
    }
    fn finish_render(&mut self) {
        if self.query.is_some() {
            print!("{}{}", termion::cursor::Show, termion::cursor::Goto(1+1+2+self.reply.len() as u16, 1+1+SCREEN_H+2));
        }
        else {
            print!("{}", termion::cursor::Hide);
        }
        stdout().flush().unwrap();
    }
    fn println(&mut self, line: String) {
        let iter = line.char_indices().step_by(TERM_W as usize).map(|(ix,_)| ix);
        let iter2 = iter.clone();
        for (ix1, ix2) in iter.zip(iter2.skip(1).chain(std::iter::once(line.len()))) {
            self.console_out.push_back(line[ix1..ix2].to_string());
        }
        while self.console_out.len() > TERM_H as usize {
            self.console_out.pop_front();
        }
        self.render_console();
    }
    fn set_query(&mut self, query: Option<String>) {
        self.query = query;
        self.render_query();
    }
    fn add_reply_char(&mut self, ch: char) {
        if (self.reply.len() as u16) < SCREEN_W+1+TERM_W-3 {
            self.reply.push(ch);
        }
        self.render_query();
    }
    fn backspace(&mut self) {
        self.reply.pop();
        self.render_query();
    }
    fn finish_reply(&mut self, resp: channel::Sender<String>) {
        resp.send(std::mem::replace(&mut self.reply, String::new())).unwrap();
        self.query = None;
        self.render_query();
    }
    fn draw_scene(&mut self, scene: Box<Scene>) {
        self.scene = *scene;
        self.render_scene();
    }
}

fn terminal_thread(rx: channel::Receiver<TerminalCommand>) {
    let mut state = TerminalState {
        console_out : VecDeque::new(),
        query : None,
        reply : "".to_string(),
        scene : Scene::default(),
    };
    state.render();
    while let Ok(message) = rx.recv() {
        use crate::terminal::TerminalCommand::*;
        match message {
            Println(line) => state.println(line),
            SetQuery(query) => state.set_query(query),
            AddReplyChar(ch) => state.add_reply_char(ch),
            Backspace => state.backspace(),
            FinishReply(resp) => state.finish_reply(resp),
            DrawScene(scene) => state.draw_scene(scene),
        }
    }
}


fn input_thread(rx: channel::Receiver<InputCommand>, tx: channel::Sender<TerminalCommand>) {
    while let Ok(command) = rx.recv() {
        match command {
            InputCommand::Query(query, result) => {
                tx.send(TerminalCommand::SetQuery(Some(query))).unwrap();
                let _raw_mode = stdout().into_raw_mode();
                for ev in stdin().events() {
                    use termion::event::*;
                    match ev.unwrap() {
                        Event::Key(Key::Ctrl('c')) => {
                            drop(_raw_mode);
                            std::process::abort()
                        }
                        Event::Key(Key::Char('\n')) => {
                            tx.send(TerminalCommand::FinishReply(result)).unwrap();
                            break;
                        }
                        Event::Key(Key::Char(c)) =>
                            tx.send(TerminalCommand::AddReplyChar(c)).unwrap(),
                        Event::Key(Key::Backspace) =>
                            tx.send(TerminalCommand::Backspace).unwrap(),
                        Event::Key(_) => {}
                        _ => {}
                    }
                }
                tx.send(TerminalCommand::SetQuery(None)).unwrap();
            }
            InputCommand::GetEvent(result) => {
                let _raw_mode = stdout().into_raw_mode();
                use termion::event::*;
                match stdin().events().next().unwrap().unwrap() {
                    Event::Key(Key::Ctrl('c')) => {
                        drop(_raw_mode);
                        std::process::abort()
                    }
                    ev => result.send(ev).unwrap(),
                }
            }
        }
    }
}
