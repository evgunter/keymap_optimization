//! Results files: one JSON record per line, appended as the records are made.

use std::io::{self, BufRead, Write};

use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ResultsError {
    #[error("failed to read results: {0}")]
    Io(#[from] io::Error),
    #[error("malformed record in results: {0}")]
    Json(#[from] serde_json::Error),
}

/// Reads the records written by `append_record`, oldest first.
pub fn read_records<T: DeserializeOwned>(results: impl BufRead) -> Result<Vec<T>, ResultsError> {
    results.lines().map(|line| Ok(serde_json::from_str(&line?)?)).collect()
}

/// Appends a record as one line of JSON.
pub fn append_record<T: Serialize>(results: &mut impl Write, record: &T) -> io::Result<()> {
    // written in one piece, so that being killed mid-write is unlikely to leave part of a record behind
    let mut line = serde_json::to_string(record)?;
    line.push('\n');
    results.write_all(line.as_bytes())?;
    results.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_round_trip_in_order() {
        let mut results = Vec::new();
        for record in [3u32, 1, 2] {
            append_record(&mut results, &record).unwrap();
        }
        assert_eq!(read_records::<u32>(results.as_slice()).unwrap(), [3, 1, 2]);
    }

    #[test]
    fn malformed_records_are_an_error() {
        assert!(matches!(read_records::<u32>("not a record\n".as_bytes()), Err(ResultsError::Json(_))));
    }
}
