use pnet::datalink::Channel::Ethernet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::Packet;
use pnet::{
    datalink::{DataLinkReceiver, DataLinkSender, NetworkInterface},
    util::MacAddr,
};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::time::Duration;

const ETHERNET_HEADER_SIZE: usize = 14;
const MSG_PREAMBLE: &[u8] = b"file";

type Key = [u8; 8];
type FileHash = [u8; 8];

pub fn compute_filehash(name: String) -> FileHash {
    let digest = md5::compute(name.into_bytes());
    digest[..8].try_into().unwrap()
}

pub enum Payload {
    Data(Key, FileHash, Vec<u8>),
    Advertise(Key, String),
    Download(Key, FileHash),
}

impl Payload {
    fn opcode(&self) -> u8 {
        match self {
            Payload::Data(_, _, _) => 0,
            Payload::Advertise(_, _) => 1,
            Payload::Download(_, _) => 2,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        // turn into bytes for over the wire
        match self {
            Payload::Data(key, filehash, data) => [&key[..], &filehash[..], data].concat(),
            Payload::Advertise(key, path) => [key, path.as_bytes()].concat(),
            Payload::Download(key, filehash) => [&key[..], &filehash[..]].concat(),
        }
    }

    fn deserialize(opcode: u8, data: &[u8]) -> Option<Payload> {
        match opcode {
            0 => {
                let key: Key = data[..8].try_into().unwrap();
                let hash: FileHash = data[8..16].try_into().unwrap();
                let info = data[16..].try_into().unwrap();
                Some(Payload::Data(key, hash, info))
            }
            1 => {
                let key: Key = data[..8].try_into().unwrap();
                let path: String = std::str::from_utf8(&data[8..]).unwrap().to_string();
                Some(Payload::Advertise(key, path))
            }
            2 => {
                let key: Key = data[..8].try_into().unwrap();
                let hash: FileHash = data[8..16].try_into().unwrap();
                Some(Payload::Download(key, hash))
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
    sharing: HashMap<Key, String>,
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
        }
    }

    pub fn interface_name(&self) -> String {
        self.interface.name.clone()
    }

    pub fn set_path(&mut self, path: &String) {
        self.local_path = path.clone();
    }

    pub fn send(&mut self, packet: Payload) {
        // TODO: chunk out and send the file via send_chunk

        // 1. serialise packet
        // 2. chunk packet into reasonable size
        // > [[opcode], [id; 8], [total], [offset], [data]]
        // 3. send chunks over wire!

        // a random id for each payload?
        // let file_content: Vec<u8> = fs::read(&file_name).unwrap();
        // self.send_chunk(&file_content);
    }

    pub fn send_chunk(&mut self, file: &Vec<u8>) {
        // TODO: include opcode, sequencenumber, totalchunks
        let data = [MSG_PREAMBLE, file].concat();
        assert!(data.len() as u8 <= u8::MAX);

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

        eprintln!("sending packet");
        eprintln!("file: {:?}", file);
        self.tx
            .send_to(ethernet_packet.packet(), None)
            .unwrap()
            .unwrap();
    }

    pub fn recv(&mut self) {
        let packet = match self.rx.next() {
            Ok(packet) => packet,
            Err(_) => return,
        };

        let packet = EthernetPacket::new(packet).unwrap();
        if packet.get_ethertype() != EtherTypes::Arp || packet.payload()[7] != 1 {
            return;
        }

        if &packet.payload()[14..18] != MSG_PREAMBLE {
            return;
        }

        let chunk_len = packet.payload()[5] as usize;

        // TODO: store file name somewhere and save using that
        // NOTE: Payload::Data(_, _) will now hold entire file in memory?
        let mut file = File::create(self.local_path.clone() + "output_floodfile").unwrap();
        file.write_all(&packet.payload()[18..(18 + chunk_len)])
            .unwrap();
    }
}
