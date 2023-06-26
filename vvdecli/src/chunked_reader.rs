use std::io::{BufReader, Read};
use thiserror::Error;

#[derive(Debug)]
pub struct ChunkedReader<R: Read> {
    reader: BufReader<R>,
    buffer: Vec<u8>,
    next_start: usize,
    end: usize,
    page_size: usize,
    max_buffer_size: usize,
}

#[derive(Debug, Error)]
pub enum ChunkedError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("next next doesn't start with an Annex-B start code")]
    NoAnnexBStartCode,
    #[error("max buffer size reached of {0} bytes")]
    MaxBufferSize(usize),
}

const DEFAULT_PAGE_SIZE: usize = 16 * 1024;
const DEFAULT_MAX_BUFFER_SIZE: usize = 16 * 1024 * 1024;

impl<R: Read> ChunkedReader<R> {
    pub fn new(reader: R) -> Self {
        Self::custom(reader, DEFAULT_PAGE_SIZE, DEFAULT_MAX_BUFFER_SIZE)
    }

    // TODO: properly implement chunking here
    pub fn next_chunk(&mut self) -> Result<Option<&[u8]>, ChunkedError> {
        if self.next_start > 0 {
            self.buffer.copy_within(self.next_start..self.end, 0);
            self.end -= self.next_start;
            self.next_start = 0;
        }

        let num_read = self.reader.read(&mut self.buffer[self.end..])?;
        self.end += num_read;

        if self.buffer[..self.end].is_empty() {
            return Ok(None);
        }

        let Some(0) = find_next_start_code(&self.buffer) else {
            return Err(ChunkedError::NoAnnexBStartCode);
        };

        const OFFSET: usize = 3;
        let Some(next_start) = find_next_start_code(&self.buffer[OFFSET..self.end]) else {
            if num_read == 0 {
                self.next_start = self.end;
                return Ok(Some(&self.buffer[..self.end]));
            } else {
                self.increase_buffer_size()?;
                return self.next_chunk();
            }
        };

        self.next_start = next_start + OFFSET;
        Ok(Some(&self.buffer[..self.next_start]))
    }

    // Only for testing
    fn custom(reader: R, page_size: usize, max_buffer_size: usize) -> Self {
        Self {
            reader: BufReader::new(reader),
            buffer: vec![0; page_size],
            next_start: 0,
            end: 0,
            page_size,
            max_buffer_size,
        }
    }

    fn increase_buffer_size(&mut self) -> Result<(), ChunkedError> {
        if self.buffer.len() >= self.max_buffer_size {
            return Err(ChunkedError::MaxBufferSize(self.max_buffer_size));
        }
        self.buffer.resize(self.buffer.len() + self.page_size, 0);
        Ok(())
    }
}

fn find_next_start_code(buffer: &[u8]) -> Option<usize> {
    const ANNEX_B_START_CODE_3: &[u8] = &[0, 0, 1];
    buffer
        .windows(3)
        .enumerate()
        .find(|(_, slice)| *slice == ANNEX_B_START_CODE_3)
        .map(|(i, _)| {
            // Start codes may be 0x000001 or 0x00000001
            if i > 0 && buffer[i - 1] == 0 {
                i - 1
            } else {
                i
            }
        })
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;

    #[test]
    fn test_find_next_start_code() {
        assert_eq!(find_next_start_code(&[0, 0, 0, 1]), Some(0));
        assert_eq!(find_next_start_code(&[0, 0, 1]), Some(0));
        assert_eq!(find_next_start_code(&[1, 0, 0, 0, 1]), Some(1));
        assert_eq!(find_next_start_code(&[1, 0, 0, 1]), Some(1));
        assert_eq!(find_next_start_code(&[0, 0, 2]), None);
        assert_eq!(find_next_start_code(&[0, 0, 0, 2]), None);
    }

    #[test]
    fn basic() {
        const INPUT_BUFFER: &[u8] = &[0, 0, 0, 1, 1, 2, 3, 4, 0, 0, 0, 1, 5, 6, 7, 8, 0, 0, 1];
        let mut chunked_reader = ChunkedReader::custom(INPUT_BUFFER, 16, 32);

        assert_eq!(
            chunked_reader.next_chunk().unwrap().unwrap(),
            &[0, 0, 0, 1, 1, 2, 3, 4]
        );
        assert_eq!(
            chunked_reader.next_chunk().unwrap().unwrap(),
            &[0, 0, 0, 1, 5, 6, 7, 8]
        );
        assert_eq!(chunked_reader.next_chunk().unwrap().unwrap(), &[0, 0, 1]);
        assert_eq!(chunked_reader.next_chunk().unwrap(), None);
    }

    #[test]
    fn from_file() -> anyhow::Result<()> {
        let reader = File::open("../tests/short.vvc")?;
        let mut chunked_reader = ChunkedReader::new(reader);
        assert_eq!(chunked_reader.next_chunk()?.unwrap().len(), 249);
        assert_eq!(chunked_reader.next_chunk()?.unwrap().len(), 17);
        assert_eq!(chunked_reader.next_chunk()?.unwrap().len(), 23);
        assert_eq!(chunked_reader.next_chunk()?.unwrap().len(), 1374);
        assert_eq!(chunked_reader.next_chunk()?.unwrap().len(), 66);
        assert_eq!(chunked_reader.next_chunk()?.unwrap().len(), 25);
        assert_eq!(chunked_reader.next_chunk()?, None);
        Ok(())
    }
}
