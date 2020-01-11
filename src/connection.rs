use tokio::net::TcpStream;
use tokio::io::{self, BufReader, AsyncReadExt, AsyncWriteExt};
use serde::{Serialize, Deserialize};

pub struct Connection {
    stream_in: BufReader<tokio::io::ReadHalf<TcpStream>>,
    stream_out: tokio::io::WriteHalf<TcpStream>,
    buffer: Vec<u8>,
}

impl Connection {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        let (a, b) = tokio::io::split(stream);
        Ok(Connection {
            stream_in: BufReader::new(a),
            stream_out: b,
            buffer: Vec::with_capacity(1024),
        })
    }
    pub async fn send<Msg: Serialize>(&mut self, msg: &Msg) -> io::Result<()> {
        self.buffer.clear();
        self.buffer.resize(4usize, 0u8);
        bincode::serialize_into(&mut self.buffer, msg).unwrap();

        let len = (self.buffer.len() - 4) as u32;
        (&mut self.buffer[0..4]).copy_from_slice(&len.to_be_bytes());

        self.stream_out.write_all(&self.buffer).await
    }
    pub async fn recv<'de, Msg: Deserialize<'de>>(&'de mut self) -> io::Result<Msg> {
        let mut len = [0,0,0,0];
        self.stream_in.read_exact(&mut len).await?;

        let len = u32::from_be_bytes(len) as usize;

        self.buffer.clear();
        self.buffer.resize(len, 0u8);
        self.stream_in.read_exact(&mut self.buffer).await?;

        Ok(bincode::deserialize(&self.buffer).unwrap())
    }
}

