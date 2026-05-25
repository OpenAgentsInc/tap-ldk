use std::{error::Error, fmt};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TlvRecord {
    pub type_id: u64,
    pub value: Vec<u8>,
}

impl TlvRecord {
    pub fn new(type_id: u64, value: impl Into<Vec<u8>>) -> Self {
        Self {
            type_id,
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TlvError {
    TruncatedBigSize,
    NonCanonicalBigSize {
        value: u64,
        minimum: u64,
    },
    TruncatedRecord {
        type_id: u64,
        expected_len: usize,
        remaining_len: usize,
    },
    DuplicateType(u64),
    OutOfOrder {
        previous: u64,
        next: u64,
    },
    UnknownRequiredType(u64),
}

impl fmt::Display for TlvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedBigSize => write!(f, "truncated BigSize integer"),
            Self::NonCanonicalBigSize { value, minimum } => {
                write!(
                    f,
                    "non-canonical BigSize integer {value}; minimum for prefix is {minimum}"
                )
            }
            Self::TruncatedRecord {
                type_id,
                expected_len,
                remaining_len,
            } => {
                write!(
                    f,
                    "truncated TLV record {type_id}; expected {expected_len} bytes, got {remaining_len}"
                )
            }
            Self::DuplicateType(type_id) => {
                write!(f, "duplicate TLV record type {type_id}")
            }
            Self::OutOfOrder { previous, next } => {
                write!(
                    f,
                    "TLV record type {next} appears after {previous}; records must be sorted"
                )
            }
            Self::UnknownRequiredType(type_id) => {
                write!(f, "unknown required TLV record type {type_id}")
            }
        }
    }
}

impl Error for TlvError {}

pub fn encode_stream(records: &[TlvRecord]) -> Result<Vec<u8>, TlvError> {
    ensure_sorted_unique(records)?;

    let mut encoded = Vec::new();
    for record in records {
        encode_big_size(record.type_id, &mut encoded);
        encode_big_size(record.value.len() as u64, &mut encoded);
        encoded.extend_from_slice(&record.value);
    }

    Ok(encoded)
}

pub fn decode_stream(bytes: &[u8]) -> Result<Vec<TlvRecord>, TlvError> {
    let mut cursor = bytes;
    let mut records = Vec::new();

    while !cursor.is_empty() {
        let type_id = decode_big_size(&mut cursor)?;
        let len = decode_big_size(&mut cursor)? as usize;
        if cursor.len() < len {
            return Err(TlvError::TruncatedRecord {
                type_id,
                expected_len: len,
                remaining_len: cursor.len(),
            });
        }

        let value = cursor[..len].to_vec();
        cursor = &cursor[len..];
        records.push(TlvRecord { type_id, value });
    }

    ensure_sorted_unique(&records)?;

    Ok(records)
}

pub fn reject_unknown_required(records: &[TlvRecord], known_types: &[u64]) -> Result<(), TlvError> {
    for record in records {
        let is_known = known_types.binary_search(&record.type_id).is_ok();
        if !is_known && record.type_id % 2 == 0 {
            return Err(TlvError::UnknownRequiredType(record.type_id));
        }
    }

    Ok(())
}

pub fn encode_big_size(value: u64, out: &mut Vec<u8>) {
    match value {
        0..=0xfc => out.push(value as u8),
        0xfd..=0xffff => {
            out.push(0xfd);
            out.extend_from_slice(&(value as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(0xfe);
            out.extend_from_slice(&(value as u32).to_be_bytes());
        }
        _ => {
            out.push(0xff);
            out.extend_from_slice(&value.to_be_bytes());
        }
    }
}

pub fn decode_big_size(bytes: &mut &[u8]) -> Result<u64, TlvError> {
    let prefix = take_byte(bytes).ok_or(TlvError::TruncatedBigSize)?;
    match prefix {
        0x00..=0xfc => Ok(prefix as u64),
        0xfd => {
            let value = take_array::<2>(bytes).map(u16::from_be_bytes)? as u64;
            require_canonical(value, 0xfd)
        }
        0xfe => {
            let value = take_array::<4>(bytes).map(u32::from_be_bytes)? as u64;
            require_canonical(value, 0x1_0000)
        }
        0xff => {
            let value = take_array::<8>(bytes).map(u64::from_be_bytes)?;
            require_canonical(value, 0x1_0000_0000)
        }
    }
}

fn ensure_sorted_unique(records: &[TlvRecord]) -> Result<(), TlvError> {
    let mut previous = None;
    for record in records {
        if let Some(previous) = previous {
            if record.type_id == previous {
                return Err(TlvError::DuplicateType(record.type_id));
            }

            if record.type_id < previous {
                return Err(TlvError::OutOfOrder {
                    previous,
                    next: record.type_id,
                });
            }
        }

        previous = Some(record.type_id);
    }

    Ok(())
}

fn require_canonical(value: u64, minimum: u64) -> Result<u64, TlvError> {
    if value < minimum {
        return Err(TlvError::NonCanonicalBigSize { value, minimum });
    }

    Ok(value)
}

fn take_array<const N: usize>(bytes: &mut &[u8]) -> Result<[u8; N], TlvError> {
    if bytes.len() < N {
        return Err(TlvError::TruncatedBigSize);
    }

    let (head, tail) = bytes.split_at(N);
    *bytes = tail;
    Ok(head.try_into().expect("slice length is checked"))
}

fn take_byte(bytes: &mut &[u8]) -> Option<u8> {
    let (byte, tail) = bytes.split_first()?;
    *bytes = tail;
    Some(*byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_sorted_unique_records() {
        let records = vec![
            TlvRecord::new(1, [0x01, 0x02]),
            TlvRecord::new(2, [0x03]),
            TlvRecord::new(253, [0xaa; 4]),
        ];

        let encoded = encode_stream(&records).expect("records encode");
        assert_eq!(decode_stream(&encoded).expect("records decode"), records);
    }

    #[test]
    fn rejects_non_canonical_big_size() {
        let mut input = &[0xfd, 0x00, 0xfc][..];
        assert_eq!(
            decode_big_size(&mut input),
            Err(TlvError::NonCanonicalBigSize {
                value: 0xfc,
                minimum: 0xfd
            })
        );
    }

    #[test]
    fn rejects_unsorted_and_duplicate_records() {
        assert_eq!(
            encode_stream(&[TlvRecord::new(2, []), TlvRecord::new(1, [])]),
            Err(TlvError::OutOfOrder {
                previous: 2,
                next: 1
            })
        );
        assert_eq!(
            encode_stream(&[TlvRecord::new(1, []), TlvRecord::new(1, [])]),
            Err(TlvError::DuplicateType(1))
        );
    }

    #[test]
    fn rejects_unknown_even_required_types() {
        let records = vec![TlvRecord::new(1, []), TlvRecord::new(4, [])];
        assert_eq!(
            reject_unknown_required(&records, &[1]),
            Err(TlvError::UnknownRequiredType(4))
        );
    }
}
