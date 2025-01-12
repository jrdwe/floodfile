use lz4_flex::block::{compress_prepend_size, decompress_size_prepended};
use pnet::datalink::Channel::Ethernet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::Packet;
use pnet::{
    datalink::{DataLinkReceiver, DataLinkSender, NetworkInterface},
    util::MacAddr,
};
use rand::prelude::*;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

const ETHERNET_HEADER_SIZE: usize = 14;
// preamble + [opcode(1), total (2), offset (2)] + [key (8)]
const FLOODFILE_HEADER_SIZE: usize = MSG_PREAMBLE.len() + 5 + 8;
const MSG_PREAMBLE: &[u8] = b"file";
const CHUNK_SIZE: usize = u8::MAX as usize - FLOODFILE_HEADER_SIZE;

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
        match self {
            Payload::File(filehash, data) => {
                let data = compress_prepend_size(data);
                eprintln!("sending len: {0}", data.len());
                [&filehash[..], &data[..]].concat()
            }
            Payload::Advertise(path) => path.as_bytes().to_vec(),
            Payload::Download(filehash) => filehash.to_vec(),
        }
    }

    fn deserialize(opcode: u8, data: &[u8]) -> Option<Payload> {
        match opcode {
            0 => {
                let hash: FileHash = data[0..16].try_into().unwrap();
                let file_compressed = data[16..].try_into().unwrap();
                let file = decompress_size_prepended(file_compressed).unwrap();
                Some(Payload::File(hash, file))
            }
            1 => {
                let path: String = std::str::from_utf8(&data[..]).unwrap().to_string();
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
    packets: HashMap<Key, Vec<Vec<u8>>>,
}

impl Channel {
    pub fn new(interface: NetworkInterface) -> Self {
        let mut config = pnet::datalink::Config::default();
        config.read_timeout = Some(Duration::from_millis(1000));

        // TODO: remove panic statements
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
            packets: HashMap::new(),
        }
    }

    pub fn interface_name(&self) -> String {
        self.interface.name.clone()
    }

    pub fn set_path(&mut self, path: &String) {
        self.local_path = path.clone();
    }

    pub fn get_path(&self) -> String {
        self.local_path.clone()
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

        assert!(total <= u16::MAX as usize);

        // 4. send chunks over wire!
        for (offset, chunk) in chunks.iter().enumerate() {
            self.send_chunk(opcode, offset as u16, total as u16, key, chunk);
        }
    }

    pub fn send_chunk(&mut self, op: u8, offset: u16, total: u16, key: Key, data: &[u8]) {
        // TODO: fixes issue with large file transfers. unideal solution.
        thread::sleep(Duration::from_micros(1_000));

        let data = [
            MSG_PREAMBLE,
            &[op],
            &offset.to_le_bytes()[..],
            &total.to_le_bytes()[..],
            &key[..],
            data,
        ]
        .concat();

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

        let data_len = packet.payload()[5] as usize - MSG_PREAMBLE.len();
        let data = &packet.payload()[18..(18 + data_len)];

        let opcode = data[0];
        let offset = u16::from_le_bytes([data[1], data[2]]) as usize;
        let total = u16::from_le_bytes([data[3], data[4]]) as usize;
        let key: Key = data[5..13].try_into().unwrap();

        // 1. allocate vec with total size
        let packet = self.packets.entry(key).or_insert(vec![vec![]; total]);

        // 2. store portion if we haven't already
        if packet[offset].is_empty() {
            packet[offset] = data[13..].to_vec();
        }

        // 3. check if we've a full payload
        let collected: bool = packet.iter().all(|x| !x.is_empty());
        if !collected {
            return None;
        }

        // 4. deserialize
        let packet = Payload::deserialize(opcode, &packet[..].concat());
        self.packets.remove(&key);

        packet
    }
}
