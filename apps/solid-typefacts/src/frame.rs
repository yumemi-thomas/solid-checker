//! Length-prefixed frames: a u32 little-endian size, then that many payload
//! bytes. Behaviour and error text follow main.go's `readFrame`/`writeFrame`.

use crate::gocompat::io_message;
use std::fmt;
use std::io::{ErrorKind, Read, Write};

pub use typefacts::MAX_MESSAGE_BYTES;

#[derive(Debug)]
pub enum FrameError {
    /// Go's `io.EOF`: the stream ended before the first byte of a read.
    Eof,
    /// Go's `io.ErrUnexpectedEOF`: the stream ended inside a frame.
    UnexpectedEof,
    TooLarge {
        size: u64,
        limit: usize,
    },
    Io(std::io::Error),
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eof => formatter.write_str("EOF"),
            Self::UnexpectedEof => formatter.write_str("unexpected EOF"),
            Self::TooLarge { size, limit } => {
                write!(formatter, "request is {size} bytes, limit is {limit}")
            }
            Self::Io(error) => formatter.write_str(&io_message(error)),
        }
    }
}

impl std::error::Error for FrameError {}

/// Go's `io.ReadFull`: EOF only if no byte was read.
fn read_full(reader: &mut impl Read, buffer: &mut [u8]) -> Result<(), FrameError> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => {
                return Err(if filled == 0 {
                    FrameError::Eof
                } else {
                    FrameError::UnexpectedEof
                });
            }
            Ok(count) => filled += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(FrameError::Io(error)),
        }
    }
    Ok(())
}

pub fn read_frame(reader: &mut impl Read) -> Result<Vec<u8>, FrameError> {
    let mut prefix = [0_u8; 4];
    read_full(reader, &mut prefix)?;
    let size = u32::from_le_bytes(prefix);
    if u64::from(size) > MAX_MESSAGE_BYTES as u64 {
        return Err(FrameError::TooLarge {
            size: u64::from(size),
            limit: MAX_MESSAGE_BYTES,
        });
    }
    let mut payload = vec![0_u8; size as usize];
    read_full(reader, &mut payload)?;
    Ok(payload)
}

/// Writes the frame prefix, refusing a payload over the message limit with
/// Go's wording.
pub fn write_frame_header(writer: &mut impl Write, size: usize) -> Result<(), String> {
    if size > MAX_MESSAGE_BYTES {
        return Err(format!(
            "response is {size} bytes, limit is {MAX_MESSAGE_BYTES}"
        ));
    }
    let prefix = u32::try_from(size)
        .map_err(|error| error.to_string())?
        .to_le_bytes();
    writer
        .write_all(&prefix)
        .map_err(|error| io_message(&error))
}

pub fn write_frame(writer: &mut impl Write, payload: &[u8]) -> Result<(), String> {
    write_frame_header(writer, payload.len())?;
    writer
        .write_all(payload)
        .map_err(|error| io_message(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_eof_kinds() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, b"abc").unwrap();
        assert_eq!(bytes, [3, 0, 0, 0, b'a', b'b', b'c']);
        let mut reader = bytes.as_slice();
        assert_eq!(read_frame(&mut reader).unwrap(), b"abc");
        assert!(matches!(read_frame(&mut reader), Err(FrameError::Eof)));
        // A truncated prefix is unexpected; a payload cut at its first byte
        // is a clean EOF, exactly as io.ReadFull reports it.
        assert!(matches!(
            read_frame(&mut [1_u8, 0].as_slice()),
            Err(FrameError::UnexpectedEof)
        ));
        assert!(matches!(
            read_frame(&mut [3_u8, 0, 0, 0].as_slice()),
            Err(FrameError::Eof)
        ));
        assert!(matches!(
            read_frame(&mut [3_u8, 0, 0, 0, b'a'].as_slice()),
            Err(FrameError::UnexpectedEof)
        ));
    }

    #[test]
    fn the_limit_has_go_wording() {
        let prefix = (MAX_MESSAGE_BYTES as u32 + 1).to_le_bytes();
        let error = read_frame(&mut prefix.as_slice()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "request is 67108865 bytes, limit is 67108864"
        );
        let mut sink = Vec::new();
        assert_eq!(
            write_frame_header(&mut sink, MAX_MESSAGE_BYTES + 1).unwrap_err(),
            "response is 67108865 bytes, limit is 67108864"
        );
        assert!(sink.is_empty());
    }
}
