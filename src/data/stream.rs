use crate::stream::InputStream;
use crate::errors::{CrushResult, error};
use crate::data::Row;

#[derive(Debug, Clone)]
pub struct Stream {
    pub stream: InputStream,
}

impl Stream {
    pub fn get(&self, idx: i128) -> CrushResult<Row> {
        let mut i = 0i128;
        loop {
            match self.stream.recv() {
                Ok(row) => {
                    if i == idx {
                        return Ok(row);
                    }
                    i += 1;
                },
                Err(_) => return error("Index out of bounds"),
            }
        }
    }

    pub fn reader(self) -> InputStream {
        self.stream
    }
}
