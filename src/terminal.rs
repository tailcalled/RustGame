use termion;
use termion::input::{TermRead};
use termion::raw::{IntoRawMode};
use std::io::{Stdout, stdout, Stdin, stdin, Write};
use std::error::{Error};
use std::thread;
use std::sync::mpsc;
use std::collections::VecDeque;

enum TerminalCommand {
    Println(String),
    SetQuery(Option<String>),
    AddReplyChar(char),
    Backspace,
    FinishReply(mpsc::Sender<String>),
}
enum InputCommand {
    Query(String, mpsc::Sender<String>),
}

pub struct Terminal {
    control: mpsc::Sender<TerminalCommand>,
    input: mpsc::Sender<InputCommand>,
}
impl Terminal {
    pub fn new() -> Self {
        let (ttx, trx) = mpsc::channel();
        let (itx, irx) = mpsc::channel();
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
        let (rtx, rrx) = mpsc::channel();
        self.input.send(InputCommand::Query(query.to_string(), rtx))?;
        Ok(rrx.recv()?)
    }
}

const SCREEN_W: u16 = 60;
const HEIGHT: u16 = 30;
const TERM_H: u16 = HEIGHT - 3;
const SCREEN_H: u16 = HEIGHT - 3;
const TERM_W: u16 = 30;

struct TerminalState {
    console_out: VecDeque<String>,
    query: Option<String>,
    reply: String,
}
impl TerminalState {
    fn render(&mut self) {
        print!("{}╔", termion::cursor::Goto(1, 1));
        for i in 0 .. SCREEN_W+1+TERM_W {
            print!("═");
        }
        print!("╗");
        for i in 0 .. HEIGHT {
            print!("{}║", termion::cursor::Goto(1, 1+i+1));
            print!("{}║", termion::cursor::Goto(1+1+SCREEN_W+1+TERM_W, 1+i+1));
            if i < HEIGHT - 3 {
                print!("{}│", termion::cursor::Goto(1+1+SCREEN_W, 1+i+1));
            }
            else if i == HEIGHT - 3 {
                print!("{}", termion::cursor::Goto(1+1, 1+i+1));
                for i in 0 .. SCREEN_W+1+TERM_W {
                    print!("─");
                }
            }
        }
        print!("{}╚", termion::cursor::Goto(1, 2+HEIGHT));
        for i in 0 .. SCREEN_W+1+TERM_W {
            print!("═");
        }
        print!("╝");
        self.finish_render();
        self.render_console();
        self.render_query()
    }
    fn render_console(&mut self) {
        for i in 0 .. TERM_H {
            print!("{}", termion::cursor::Goto(1+1+SCREEN_W+1, 1+1+i));
            let mut rem = TERM_W;
            if (self.console_out.len() >= (TERM_H - i) as usize) {
                let line = &self.console_out[i as usize - (TERM_H as usize - self.console_out.len())];
                print!("{}", line);
                rem -= line.len() as u16;
            }
            for i in 0 .. rem {
                print!(" ")
            }
        }
        self.finish_render();
    }
    fn render_query(&mut self) {
        print!("{}", termion::cursor::Goto(1+1, 1+1+SCREEN_H+1));
        let mut rem = (SCREEN_W+1+TERM_W) as usize;
        match &self.query {
            None => {}
            Some(query) => {
                print!("{}", query);
                rem -= query.len();
            }
        }
        for i in 0 .. rem {
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
        for i in 0 .. rem {
            print!(" ");
        }
        self.finish_render();
    }
    fn finish_render(&mut self) {
        if self.query.is_some() {
            print!("{}{}", termion::cursor::Show, termion::cursor::Goto(1+1+2+self.reply.len() as u16, 1+1+SCREEN_H+2));
        }
        else {
            print!("{}", termion::cursor::Hide);
        }
        stdout().flush();
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
        if self.reply.len() as (u16) < SCREEN_W+1+TERM_W-3 {
            self.reply.push(ch);
        }
        self.render_query();
    }
    fn backspace(&mut self) {
        self.reply.pop();
        self.render_query();
    }
    fn finish_reply(&mut self, resp: mpsc::Sender<String>) {
        resp.send(std::mem::replace(&mut self.reply, String::new()));
        self.query = None;
        self.render_query();
    }
}

fn terminal_thread(rx: mpsc::Receiver<TerminalCommand>) {
    let mut state = TerminalState {
        console_out : VecDeque::new(),
        query : None,
        reply : "".to_string(),
    };
    state.render();
    let mut iter = 0;
    while let Ok(message) = rx.recv() {
        use crate::terminal::TerminalCommand::*;
        match message {
            Println(line) => state.println(line),
            SetQuery(query) => state.set_query(query),
            AddReplyChar(ch) => state.add_reply_char(ch),
            Backspace => state.backspace(),
            FinishReply(resp) => state.finish_reply(resp),
        }
        iter += 1;
    }
}


fn input_thread(rx: mpsc::Receiver<InputCommand>, tx: mpsc::Sender<TerminalCommand>) {
    while let Ok(InputCommand::Query(query, result)) = rx.recv() {
        tx.send(TerminalCommand::SetQuery(Some(query)));
        let raw_mode = stdout().into_raw_mode();
        for ev in stdin().events() {
            use termion::event::*;
            match ev.unwrap() {
                Event::Key(Key::Ctrl('c')) =>
                    std::process::abort(),
                Event::Key(Key::Char('\n')) => {
                    tx.send(TerminalCommand::FinishReply(result));
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
        tx.send(TerminalCommand::SetQuery(None));
    }
}