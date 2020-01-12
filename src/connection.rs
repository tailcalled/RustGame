use tokio::net::TcpStream;
use tokio::io::{self, BufReader, AsyncReadExt, AsyncWriteExt};
use serde::{Serialize, Deserialize};

pub fn split_stream(stream: TcpStream) -> (ConnectionIn, ConnectionOut) {
    let (a, b) = tokio::io::split(stream);
    (
        ConnectionIn {
            stream: BufReader::new(a),
            buffer: Vec::with_capacity(1024),
        },
        ConnectionOut {
            stream: b,
            buffer: Vec::with_capacity(1024),
        },
    )
}

pub struct ConnectionIn {
    stream: BufReader<tokio::io::ReadHalf<TcpStream>>,
    buffer: Vec<u8>,
}
pub struct ConnectionOut {
    stream: tokio::io::WriteHalf<TcpStream>,
    buffer: Vec<u8>,
}

impl ConnectionOut {
    pub async fn send<Msg: Serialize>(&mut self, msg: &Msg) -> io::Result<()> {
        self.buffer.clear();
        self.buffer.resize(4usize, 0u8);
        bincode::serialize_into(&mut self.buffer, msg).unwrap();

        let len = (self.buffer.len() - 4) as u32;
        (&mut self.buffer[0..4]).copy_from_slice(&len.to_be_bytes());

        self.stream.write_all(&self.buffer).await
    }
}
impl ConnectionIn {
    pub async fn recv<'de, Msg: Deserialize<'de>>(&'de mut self) -> io::Result<Msg> {
        let mut len = [0,0,0,0];
        self.stream.read_exact(&mut len).await?;

        let len = u32::from_be_bytes(len) as usize;

        self.buffer.clear();
        self.buffer.resize(len, 0u8);
        self.stream.read_exact(&mut self.buffer).await?;

        Ok(bincode::deserialize(&self.buffer).unwrap())
    }
}

