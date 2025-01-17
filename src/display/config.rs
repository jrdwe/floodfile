use pnet::datalink::NetworkInterface;
use std::collections::{HashMap, HashSet};

use crate::network::utils::usable_interfaces;
use crate::network::{Channel, FileHash};

pub struct Config {
    pub channel: Channel,
    pub shared: HashMap<FileHash, String>,
    pub sharing: HashMap<FileHash, String>,
    pub requested: HashSet<FileHash>,
}

impl Config {
    pub fn new() -> Self {
        let interfaces = usable_interfaces();

        Config {
            channel: Channel::new(interfaces[0].clone()).unwrap(),
            shared: HashMap::new(),
            sharing: HashMap::new(),
            requested: HashSet::new(),
        }
    }

    pub fn from(interface: NetworkInterface) -> Self {
        Config {
            channel: Channel::new(interface).unwrap(),
            shared: HashMap::new(),
            sharing: HashMap::new(),
            requested: HashSet::new(),
        }
    }
}
