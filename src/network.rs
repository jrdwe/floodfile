use pnet::datalink::NetworkInterface;

pub fn usable_interfaces() -> Vec<NetworkInterface> {
    let mut interfaces = pnet::datalink::interfaces()
        .into_iter()
        .filter(|i| i.mac.is_some() && !i.ips.is_empty())
        .collect::<Vec<NetworkInterface>>();

    interfaces.sort_unstable_by_key(|i| i.ips.len());
    interfaces.reverse();
    interfaces
}
