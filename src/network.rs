use crossbeam::channel::{unbounded, Receiver, Sender};
use payload::Payload;
use pnet::datalink::Channel::Ethernet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::Packet;
use pnet::{
    datalink::{DataLinkReceiver, DataLinkSender, NetworkInterface},
    util::MacAddr,
};
use rand::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::thread;

use crate::errors::FloodFileError;

pub mod payload;
pub mod utils;

const ETHERNET_HEADER_SIZE: usize = 14;
const ETHERNET_PACKET_SIZE: usize = 1518;
// bytes: [preamble (4)] + [opcode(1), total (2), offset (2)] + [key (8)]
const FLOODFILE_HEADER_SIZE: usize = MSG_PREAMBLE.len() + 5 + 8;
const MSG_PREAMBLE: &[u8] = b"file";
const CHUNK_SIZE: usize = u8::MAX as usize - FLOODFILE_HEADER_SIZE;

pub type Key = [u8; 8];
pub type FileHash = [u8; 16];

fn listener_thread(
    mut channel_rx: Box<dyn DataLinkReceiver>,
    buffer_tx: Sender<[u8; ETHERNET_PACKET_SIZE]>,
) {
    let mut buffer = [0u8; 1518];
    loop {
        let data = match channel_rx.next() {
            Ok(packet) => packet,
            _ => continue,
        };

        let len = data.len().min(1518);
        buffer[..len].copy_from_slice(&data[..len]);

        buffer_tx.send(buffer).ok();
    }
}

pub struct Channel {
    src_mac_addr: MacAddr,
    local_path: PathBuf,
    interface: NetworkInterface,
    tx: Box<dyn DataLinkSender>,
    buffer_rx: Receiver<[u8; ETHERNET_PACKET_SIZE]>,
    packets: HashMap<Key, Vec<Vec<u8>>>,
}

impl Channel {
    pub fn new(interface: NetworkInterface) -> Result<Self, FloodFileError> {
        let config = pnet::datalink::Config::default();
        let (tx, rx) = match pnet::datalink::channel(&interface, config) {
            Ok(Ethernet(tx, rx)) => (tx, rx),
            Ok(_) => return Err(FloodFileError::InvalidChannelType),
            Err(e) => return Err(FloodFileError::ChannelError(e)),
        };

        let (buffer_tx, buffer_rx) = unbounded::<[u8; ETHERNET_PACKET_SIZE]>();
        thread::spawn(move || listener_thread(rx, buffer_tx)); // detached

        Ok(Self {
            src_mac_addr: interface.mac.unwrap(),
            local_path: std::env::temp_dir(),
            interface,
            tx,
            buffer_rx,
            packets: HashMap::new(),
        })
    }

    pub fn send(&mut self, packet: Payload) -> Result<(), FloodFileError> {
        let data = packet.serialize();

        // chunk packet into maximum possible size
        let chunks: Vec<&[u8]> = data.chunks(CHUNK_SIZE).collect();

        // structure: [[opcode], [total], [offset], [payload specific data]]
        let opcode = packet.opcode();
        let total = chunks.len();
        let key: Key = rand::thread_rng().gen();

        if total >= u16::MAX as usize {
            return Err(FloodFileError::FileTooLarge);
        }

        // send chunks over wire!
        for (offset, chunk) in chunks.iter().enumerate() {
            self.send_chunk(opcode, offset as u16, total as u16, key, chunk)?;
        }

        Ok(())
    }

    pub fn send_chunk(
        &mut self,
        op: u8,
        offset: u16,
        total: u16,
        key: Key,
        data: &[u8],
    ) -> Result<(), FloodFileError> {
        let data = [
            MSG_PREAMBLE,
            &[op],
            &offset.to_le_bytes()[..],
            &total.to_le_bytes()[..],
            &key[..],
            data,
        ]
        .concat();

        if data.len() > u8::MAX as usize {
            return Err(FloodFileError::PacketTooLarge);
        }

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
        let mut ethernet_packet = MutableEthernetPacket::new(&mut ethernet_buffer)
            .ok_or(FloodFileError::FailedToSerializeArp)?;
        ethernet_packet.set_source(self.src_mac_addr);
        ethernet_packet.set_destination(MacAddr::broadcast());
        ethernet_packet.set_ethertype(EtherTypes::Arp);
        ethernet_packet.set_payload(&arp_packet);

        match self.tx.send_to(ethernet_packet.packet(), None) {
            Some(Ok(())) => Ok(()),
            _ => Err(FloodFileError::FailedToSendArp),
        }
    }

    pub fn recv(&mut self) -> Result<Option<Payload>, FloodFileError> {
        let data = match self.buffer_rx.try_recv() {
            Ok(data) => data,
            _ => return Ok(None),
        };

        let packet = match EthernetPacket::new(&data) {
            Some(packet) => packet,
            _ => return Ok(None),
        };

        if packet.get_ethertype() != EtherTypes::Arp || packet.payload()[7] != 1 {
            return Ok(None);
        }
        if &packet.payload()[14..18] != MSG_PREAMBLE {
            return Ok(None);
        }

        let data_len = packet.payload()[5] as usize - MSG_PREAMBLE.len();
        let data = &packet.payload()[18..(18 + data_len)];

        let opcode = data[0];
        let offset = u16::from_le_bytes([data[1], data[2]]) as usize;
        let total = u16::from_le_bytes([data[3], data[4]]) as usize;
        let key: Key = match data[5..13].try_into() {
            Ok(key) => key,
            _ => return Ok(None),
        };

        // allocate vec with total size
        let packet = self.packets.entry(key).or_insert(vec![vec![]; total]);

        // store portion if we haven't already
        if packet[offset].is_empty() {
            packet[offset] = data[13..].to_vec();
        }

        let collected: bool = packet.iter().all(|x| !x.is_empty());
        if !collected {
            return Ok(None);
        }

        let packet = Payload::deserialize(opcode, &packet[..].concat())
            .ok_or(FloodFileError::FailedToDeserializeArp)?;
        self.packets.remove(&key);

        Ok(Some(packet))
    }

    pub fn interface_name(&self) -> String {
        self.interface.name.clone()
    }

    pub fn set_path(&mut self, path: &String) -> Result<(), FloodFileError> {
        self.local_path = match PathBuf::from_str(path) {
            Ok(path) => path,
            Err(_) => return Err(FloodFileError::InvalidDestinationPath),
        };

        Ok(())
    }

    pub fn get_path(&self) -> PathBuf {
        self.local_path.clone()
    }
}
