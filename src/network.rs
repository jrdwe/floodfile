use pnet::datalink::Channel::Ethernet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::Packet;
use pnet::{
    datalink::{DataLinkReceiver, DataLinkSender, NetworkInterface},
    util::MacAddr,
};
use std::fs;
use std::time::Duration;

const ETHERNET_HEADER_SIZE: usize = 14;
const MSG_PREAMBLE: &[u8] = b"file";

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
}

impl Channel {
    pub fn new(interface: NetworkInterface) -> Self {
        let mut config = pnet::datalink::Config::default();
        config.read_timeout = Some(Duration::from_millis(750));

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
        }
    }

    pub fn interface_name(&self) -> String {
        self.interface.name.clone()
    }

    pub fn set_path(&mut self, path: &String) {
        self.local_path = path.clone();
    }

    pub fn path(&self) -> String {
        self.local_path.clone()
    }

    pub fn send(&mut self, file_name: String) {
        // TODO: chunk out and send the file via send_chunk
        let file_content: Vec<u8> = fs::read(&file_name).unwrap();
        self.send_chunk(&file_content);
    }

    pub fn send_chunk(&mut self, file: &Vec<u8>) {
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

        eprintln!("{:?}", arp_packet);

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

    pub fn recv(&mut self) {
        // TODO: parse packets received as valid and assemble file
        if let Ok(packet) = self.rx.next() {
            let packet = EthernetPacket::new(packet).unwrap();
            if packet.get_ethertype() != EtherTypes::Arp || packet.payload()[7] != 1 {
                eprintln!("not taking this non arp");
                return;
            }

            eprintln!("{:?}", &packet.payload()[14..18]);
            if !(&packet.payload()[14..18] != MSG_PREAMBLE) {
                return;
            }

            let chunk_len: u8 = packet.payload()[5];
            // let chunk: [u8] = packet.payload()[14..(chunk_len as usize)];

            // throw bytes into file on disk?
        }

        // probably extract file here? somewhere?
    }
}
