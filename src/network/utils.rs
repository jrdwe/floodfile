use pnet::datalink::NetworkInterface;

use crate::errors::FloodFileError;
use crate::network::FileHash;

pub fn compute_filehash(name: &String) -> Result<FileHash, FloodFileError> {
    let digest = md5::compute(name.clone().into_bytes());
    match digest.try_into() {
        Ok(hash) => Ok(hash),
        _ => Err(FloodFileError::UnableToGenerateHash),
    }
}

pub fn usable_interfaces() -> Vec<NetworkInterface> {
    let mut interfaces = pnet::datalink::interfaces()
        .into_iter()
        .filter(|i| i.mac.is_some() && !i.ips.is_empty())
        .collect::<Vec<NetworkInterface>>();

    interfaces.sort_by_key(|i| i.ips.len());
    interfaces.reverse();

    interfaces
}
