pub mod network_status {
    use std::net::Ipv4Addr;

    /// Структура для хранения полной информации о сетевом адаптере.
    /// Использует кроссплатформенный крейт `netdev`.
    #[derive(Debug, Clone)]
    pub struct AdapterInfo {
        pub name: String,
        pub mac_address: String,
        pub oper_status: String,
        pub ipv4_addresses: Vec<Ipv4Addr>,
        pub connected: bool,
    }

    /// Получает список всех сетевых адаптеров в системе, используя `netdev`.
    pub fn get_adapters() -> Vec<AdapterInfo> {
        let interfaces = netdev::get_interfaces();
        let mut adapter_infos = Vec::new();

        for iface in interfaces {
            let ipv4_addresses: Vec<Ipv4Addr> = iface.ipv4.iter().map(|net| net.addr()).collect();

            let is_up = iface.is_up() || iface.is_running();
            let oper_status = if is_up { "UP" } else { "DOWN" }.to_string();

            let mac_address = iface
                .mac_addr
                .map(|m| m.to_string().to_uppercase())
                .unwrap_or_else(|| "N/A".to_string());

            let name = iface
                .friendly_name
                .clone()
                .unwrap_or_else(|| iface.name.clone());

            let info = AdapterInfo {
                name,
                mac_address,
                oper_status,
                ipv4_addresses,
                connected: is_up,
            };
            adapter_infos.push(info);
        }

        adapter_infos
    }
}
