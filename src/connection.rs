use std::io::{self, Read, Write, BufReader};
use std::net::TcpStream;
use serde::{Serialize, Deserialize};

pub struct Connection {
    stream_in: BufReader<TcpStream>,
    stream_out: TcpStream,
    buffer: Vec<u8>,
}

impl Connection {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        let stream2 = stream.try_clone()?;
        Ok(Connection {
            stream_in: BufReader::new(stream),
            stream_out: stream2,
            buffer: Vec::with_capacity(1024),
        })
    }
    pub fn send<Msg: Serialize>(&mut self, msg: &Msg) -> io::Result<()> {
        self.buffer.clear();
        self.buffer.resize(4usize, 0u8);
        bincode::serialize_into(&mut self.buffer, msg).unwrap();

        let len = (self.buffer.len() - 4) as u32;
        (&mut self.buffer[0..4]).copy_from_slice(&len.to_be_bytes());

        self.stream_out.write_all(&self.buffer)
    }
    pub fn recv<'de, Msg: Deserialize<'de>>(&'de mut self) -> io::Result<Msg> {
        let mut len = [0,0,0,0];
        self.stream_in.read_exact(&mut len)?;

        let len = u32::from_be_bytes(len) as usize;

        self.buffer.clear();
        self.buffer.resize(len, 0u8);
        self.stream_in.read_exact(&mut self.buffer)?;

        Ok(bincode::deserialize(&self.buffer).unwrap())
    }
}

