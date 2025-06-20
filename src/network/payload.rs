use lz4_flex::block::{compress_prepend_size, decompress_size_prepended};

use crate::network::FileHash;

#[derive(Debug)]
pub enum Payload {
    File(FileHash, Vec<u8>),
    Advertise(String),
    DownloadRequest(FileHash),
}

impl Payload {
    pub fn opcode(&self) -> u8 {
        match self {
            Payload::File(_, _) => 0,
            Payload::Advertise(_) => 1,
            Payload::DownloadRequest(_) => 2,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        match self {
            Payload::File(filehash, data) => {
                let data = compress_prepend_size(data);
                [&filehash[..], &data[..]].concat()
            }
            Payload::Advertise(path) => path.as_bytes().to_vec(),
            Payload::DownloadRequest(filehash) => filehash.to_vec(),
        }
    }

    pub fn deserialize(opcode: u8, data: &[u8]) -> Option<Payload> {
        match opcode {
            0 => {
                // need at least 16 bytes for hash
                if data.len() < 16 {
                    return None;
                }

                // extract file hash from first 16 bytes
                let hash: FileHash = data[0..16].try_into().ok()?;

                // decompress remaining data
                let file_compressed = &data[16..];
                let file = decompress_size_prepended(file_compressed).ok()?;

                Some(Payload::File(hash, file))
            }
            1 => {
                let path = std::str::from_utf8(data).ok()?.to_string();
                Some(Payload::Advertise(path))
            }
            2 => {
                // need exactly 16 bytes for hash
                if data.len() < 16 {
                    return None;
                }

                let hash: FileHash = data[0..16].try_into().ok()?;
                Some(Payload::DownloadRequest(hash))
            }
            _ => None,
        }
    }
}
