use pnet::datalink::Channel::Ethernet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::Packet;
use pnet::{
    datalink::{DataLinkReceiver, DataLinkSender, NetworkInterface},
    util::MacAddr,
};
use rand::prelude::*;
use std::collections::HashMap;
// use std::fs;
// use std::fs::File;
// use std::io::prelude::*;
use std::time::Duration;

const ETHERNET_HEADER_SIZE: usize = 14;
const MSG_PREAMBLE: &[u8] = b"file";
const CHUNK_SIZE: usize = u8::MAX as usize - (MSG_PREAMBLE.len() + 3 + 8);

pub type Key = [u8; 8];
pub type FileHash = [u8; 16];

pub fn compute_filehash(name: &String) -> FileHash {
    let digest = md5::compute(name.clone().into_bytes());
    digest.try_into().unwrap()
}

#[derive(Debug)]
pub enum Payload {
    File(FileHash, Vec<u8>),
    Advertise(String),
    Download(FileHash),
}

impl Payload {
    fn opcode(&self) -> u8 {
        match self {
            Payload::File(_, _) => 0,
            Payload::Advertise(_) => 1,
            Payload::Download(_) => 2,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        // turn into bytes for over the wire
        match self {
            Payload::File(filehash, data) => [&filehash[..], data].concat(),
            Payload::Advertise(path) => path.as_bytes().to_vec(),
            Payload::Download(filehash) => filehash.to_vec(),
        }
    }

    fn deserialize(opcode: u8, data: &[u8]) -> Option<Payload> {
        match opcode {
            0 => {
                let hash: FileHash = data[0..16].try_into().unwrap();
                let file = data[16..].try_into().unwrap();
                Some(Payload::File(hash, file))
            }
            1 => {
                // let key: Key = data[..8].try_into().unwrap();
                let path: String = std::str::from_utf8(&data[8..]).unwrap().to_string();
                Some(Payload::Advertise(path))
            }
            2 => {
                let hash: FileHash = data[..16].try_into().unwrap();
                Some(Payload::Download(hash))
            }
            _ => None,
        }
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

pub struct Channel {
    src_mac_addr: MacAddr,
    local_path: String,
    interface: NetworkInterface,
    tx: Box<dyn DataLinkSender>,
    rx: Box<dyn DataLinkReceiver>,
    sharing: HashMap<FileHash, String>,
    packets: HashMap<Key, Vec<Vec<u8>>>,
}

impl Channel {
    pub fn new(interface: NetworkInterface) -> Self {
        let mut config = pnet::datalink::Config::default();
        config.read_timeout = Some(Duration::from_millis(1000));

        let (tx, rx) = match pnet::datalink::channel(&interface, config) {
            Ok(Ethernet(tx, rx)) => (tx, rx),
            Ok(_) => panic!("Unhandled channel type"),
            Err(e) => panic!("Error occurred fetching channel: {}", e),
        };

        Self {
            src_mac_addr: interface.mac.unwrap(),
            local_path: String::from("/tmp/"),
            interface,
            tx,
            rx,
            sharing: HashMap::new(),
            packets: HashMap::new(),
        }
    }

    pub fn interface_name(&self) -> String {
        self.interface.name.clone()
    }

    pub fn set_path(&mut self, path: &String) {
        self.local_path = path.clone();
    }

    pub fn send(&mut self, packet: Payload) {
        // 1. serialise packet
        let data = packet.serialize();

        // 2. chunk packet into reasonable size
        let chunks: Vec<&[u8]> = data.chunks(CHUNK_SIZE).collect();

        // 3. [[opcode], [total], [offset], [payload specific data]]
        let opcode = packet.opcode();
        let total = chunks.len();
        let key: Key = rand::thread_rng().gen();

        // 4. store advertising filename
        if let Payload::Advertise(filename) = packet {
            let hash = compute_filehash(&filename);
            self.sharing.insert(hash, filename.clone());
        }

        // 5. send chunks over wire!
        for (offset, chunk) in chunks.iter().enumerate() {
            self.send_chunk(opcode, offset as u8, total as u8, key, chunk);
        }
    }

    pub fn send_chunk(&mut self, op: u8, offset: u8, total: u8, key: Key, data: &[u8]) {
        let data = [MSG_PREAMBLE, &[op, offset, total], &key[..], data].concat();
        assert!(data.len() <= u8::MAX as usize);

        let arp_packet = [
            &[0, 1],                     // hardware type
            &[8, 0],                     // protocol type
            &[6][..],                    // hardware size
            &[data.len() as u8],         // payload length
            &[0, 1],                     // opcode - req
            &self.src_mac_addr.octets(), // sender mac
            &data,                       // payload!
            &[0; 6],                     // target mac
            &data,                       // payload!
        ]
        .concat();

        let mut ethernet_buffer = vec![0; ETHERNET_HEADER_SIZE + arp_packet.len()];
        let mut ethernet_packet = MutableEthernetPacket::new(&mut ethernet_buffer).unwrap();
        ethernet_packet.set_source(self.src_mac_addr);
        ethernet_packet.set_destination(MacAddr::broadcast());
        ethernet_packet.set_ethertype(EtherTypes::Arp);
        ethernet_packet.set_payload(&arp_packet);

        self.tx
            .send_to(ethernet_packet.packet(), None)
            .unwrap()
            .unwrap();
    }

    pub fn recv(&mut self) -> Option<Payload> {
        let packet = match self.rx.next() {
            Ok(packet) => packet,
            Err(_) => return None,
        };

        let packet = EthernetPacket::new(packet).unwrap();

        // check for early exit conditions
        if packet.get_ethertype() != EtherTypes::Arp || packet.payload()[7] != 1 {
            return None;
        }
        if &packet.payload()[14..18] != MSG_PREAMBLE {
            return None;
        }

        let data_len = packet.payload()[5] as usize;
        let data = &packet.payload()[18..(18 + data_len)];

        let opcode = data[0]; // why not include this into the payload?
        let offset = data[1] as usize;
        let total = data[2] as usize;
        let key: Key = data[3..11].try_into().unwrap();

        // 1. check if we've seen the id - allocate vec with total size
        let packet = self.packets.entry(key).or_insert(vec![vec![]; total]);

        // 2. store portion if we haven't already
        if packet[offset].is_empty() {
            packet[offset] = data[11..].to_vec();
        }

        // 3. check if we've a full payload
        let collected: bool = packet.iter().all(|x| !x.is_empty());
        if !collected {
            return None;
        }

        // 4. deserialize
        let packet = Payload::deserialize(opcode, &packet[..].concat());
        self.packets.remove(&key);

        // TODO: maybe check if request for file we advertise right here..

        packet
    }
}
