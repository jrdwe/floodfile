use pnet::datalink::Channel::Ethernet;
use pnet::{
    datalink::{DataLinkReceiver, DataLinkSender, NetworkInterface},
    util::MacAddr,
};

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
    interface: NetworkInterface,
    tx: Box<dyn DataLinkSender>,
    rx: Box<dyn DataLinkReceiver>,
}

impl Channel {
    pub fn new(interface: NetworkInterface) -> Self {
        let (tx, rx) = match pnet::datalink::channel(&interface, Default::default()) {
            Ok(Ethernet(tx, rx)) => (tx, rx),
            Ok(_) => panic!("Unhandled channel type"),
            Err(e) => panic!("Error occurred fetching channel: {}", e),
        };

        Self {
            src_mac_addr: interface.mac.unwrap(),
            interface,
            tx,
            rx,
        }
    }

    pub fn interface_name(&self) -> String {
        self.interface.name.clone()
    }

    pub fn send() {}
    pub fn recv() {}

    // send to network func
    // fetch from network func
}
