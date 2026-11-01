use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::run;

#[cfg(not(windows))]
mod linux;
#[cfg(not(windows))]
pub use linux::run;

/// Общая логика WOL-сервиса.
use hex::FromHex;

pub fn run_service(running: Arc<AtomicBool>, command: &[OsString]) -> std::io::Result<()> {
    use std::io;
    use std::net::UdpSocket;
    use std::time::Instant;

    let socket = match UdpSocket::bind("0.0.0.0:9") {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "Не удалось занять UDP порт 9. Убедитесь, что он не занят другой программой.");
            return Err(e);
        }
    };
    // SO_BROADCAST нужен только для отправки пакетов, а сервис их не шлёт —
    // поэтому при отказе не прерываем работу.
    if let Err(e) = socket.set_broadcast(true) {
        warn!(error = %e, "Не удалось установить опцию SO_BROADCAST");
    }
    // Без таймаута чтения цикл не узнает о запросе остановки сервиса.
    if let Err(e) = socket.set_read_timeout(Some(Duration::from_secs(1))) {
        error!(error = %e, "Не удалось установить таймаут чтения для сокета");
        return Err(e);
    }

    let mut valid_macs = get_valid_mac_addresses();
    let cmd_info = if command.is_empty() {
        "не задана".to_string()
    } else {
        format!("{command:?}")
    };
    if valid_macs.is_empty() {
        warn!(macs = ?macs_to_strings(&valid_macs), cmd = %cmd_info, "Сервис запущен. Нет интерфейсов. Ожидание сети...");
    } else {
        info!(macs = ?macs_to_strings(&valid_macs), cmd = %cmd_info, "Сервис запущен и слушает пакеты");
    }

    let mut buf = [0; 1024];
    let debounce_interval = Duration::from_secs(10);
    let mac_refresh_interval = Duration::from_secs(60);
    let mut last_trigger_time = Instant::now()
        .checked_sub(debounce_interval)
        .unwrap_or_else(Instant::now);
    let mut last_mac_refresh = Instant::now();

    while running.load(Ordering::SeqCst) {
        if last_mac_refresh.elapsed() >= mac_refresh_interval {
            if let Some(new_macs) = refresh_macs(&valid_macs) {
                valid_macs = new_macs;
                info!(macs = ?macs_to_strings(&valid_macs), "Сетевые интерфейсы обновлены");
            }
            last_mac_refresh = Instant::now();
        }

        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                debug!(len = len, src = %src, "Получен пакет");
                if is_magic_packet(&buf[..len], &valid_macs)
                    && last_trigger_time.elapsed() >= debounce_interval
                {
                    if command.is_empty() {
                        info!("Обнаружен валидный магический пакет.");
                    } else {
                        info!(
                            command = ?command,
                            "Обнаружен валидный магический пакет! Выполнение команды"
                        );
                        execute_command(command);
                    }
                    last_trigger_time = Instant::now();
                }
            }
            Err(e)
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                // Сигнал (Ctrl+C / SIGTERM) прервал блокирующее чтение —
                // это штатная остановка, а не сбой: обработчик сигнала уже
                // выставил флаг остановки и цикл завершится сам. Настоящие
                // ошибки чтения логируются в следующей ветке как раньше.
                continue;
            }
            Err(e) => {
                error!(error = %e, "Ошибка при получении данных из сокета");
            }
        }
    }

    Ok(())
}

/// Форматирует MAC-адрес для логов: верхний регистр, пары байтов через
/// двоеточие, например "00:11:22:AA:BB:CC".
fn macs_to_strings(macs: &[[u8; 6]]) -> Vec<String> {
    macs.iter()
        .map(|mac| {
            mac.iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(":")
        })
        .collect()
}

fn refresh_macs(current: &[[u8; 6]]) -> Option<Vec<[u8; 6]>> {
    let mut fresh = get_valid_mac_addresses();
    fresh.sort_unstable();

    let mut prev = current.to_vec();
    prev.sort_unstable();

    if fresh == prev {
        None
    } else {
        Some(fresh)
    }
}

fn is_magic_packet(packet: &[u8], valid_macs: &[[u8; 6]]) -> bool {
    if packet.len() < 102 || !packet.starts_with(&[0xFF; 6]) {
        return false;
    }
    let target_mac: [u8; 6] = match packet[6..12].try_into() {
        Ok(mac) => mac,
        Err(_) => return false,
    };
    if !valid_macs.contains(&target_mac) {
        return false;
    }
    for i in 1..16 {
        let start = 6 + i * 6;
        let end = start + 6;
        if packet.get(start..end) != Some(&target_mac) {
            return false;
        }
    }
    true
}

fn get_valid_mac_addresses() -> Vec<[u8; 6]> {
    use std::collections::HashSet;

    let mut macs = HashSet::new();

    for adapter in wol_service::network_status::get_adapters() {
        if !adapter.connected {
            continue;
        }

        let name = adapter.name.to_lowercase();
        if name == "lo"
            || name == "lo0"
            || name == "lo1"
            || name.contains("loopback")
            || name.contains("vmware")
            || name.contains("virtual")
            || name.contains("vethernet")
            || name.contains("tap")
            || name.starts_with("docker")
            || name.starts_with("br-")
            || name.starts_with("veth")
        {
            continue;
        }

        if adapter.mac_address == "N/A" {
            continue;
        }

        let has_valid_ip = adapter
            .ipv4_addresses
            .iter()
            .any(|ip| !ip.is_loopback() && !ip.is_link_local());

        if !has_valid_ip {
            continue;
        }

        if let Ok(mac_bytes) = <[u8; 6]>::from_hex(adapter.mac_address.replace(':', "")) {
            macs.insert(mac_bytes);
        }
    }

    macs.into_iter().collect()
}

fn execute_command(command: &[OsString]) {
    let Some((cmd, args)) = command.split_first() else {
        return; // Команда не задана
    };

    match std::process::Command::new(cmd).args(args).spawn() {
        Ok(mut child) => {
            // Ждём завершения в отдельном потоке, иначе на Unix завершившийся
            // дочерний процесс останется зомби до остановки сервиса.
            let cmd_name = cmd.clone();
            std::thread::spawn(move || match child.wait() {
                Ok(status) => debug!(status = %status, cmd = ?cmd_name, "Команда выполнена"),
                Err(e) => {
                    error!(error = %e, cmd = ?cmd_name, "Ошибка при ожидании завершения команды")
                }
            });
        }
        Err(e) => {
            error!(error = %e, cmd = ?cmd, "Не удалось выполнить команду");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_magic_packet;

    /// Произвольные тестовые MAC-адреса.
    const MAC_A: [u8; 6] = [0xA8, 0x5E, 0x45, 0xD0, 0x3F, 0x45];
    const MAC_B: [u8; 6] = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];

    /// Валидный пакет: 6×0xFF, 16 повторений `mac`, затем `trailing`.
    fn magic_packet(mac: [u8; 6], trailing: &[u8]) -> Vec<u8> {
        let mut p = vec![0xFFu8; 6];
        for _ in 0..16 {
            p.extend_from_slice(&mac);
        }
        p.extend_from_slice(trailing);
        p
    }

    #[test]
    fn accepts_valid_packet() {
        assert!(is_magic_packet(&magic_packet(MAC_A, &[]), &[MAC_A]));
    }

    #[test]
    fn accepts_packet_longer_than_102() {
        // «Минимум 102 байта»: хвост датаграммы допускается и игнорируется.
        assert!(is_magic_packet(
            &magic_packet(MAC_A, &[0x00, 0x01, 0x02]),
            &[MAC_A]
        ));
    }

    #[test]
    fn rejects_short_packet() {
        let p = magic_packet(MAC_A, &[]);
        assert!(!is_magic_packet(&p[..101], &[MAC_A]));
    }

    #[test]
    fn rejects_packet_without_sync_prefix() {
        let mut p = magic_packet(MAC_A, &[]);
        p[3] = 0xFE; // один байт синхропрефикса не 0xFF
        assert!(!is_magic_packet(&p, &[MAC_A]));
    }

    #[test]
    fn rejects_unknown_target_mac() {
        // Самосогласованный пакет на чужой MAC: цель не в списке.
        assert!(!is_magic_packet(&magic_packet(MAC_A, &[]), &[MAC_B]));
        // Цель есть в списке из нескольких адресов.
        assert!(is_magic_packet(&magic_packet(MAC_B, &[]), &[MAC_A, MAC_B]));
    }

    #[test]
    fn rejects_corrupted_repeat() {
        let mut p = magic_packet(MAC_A, &[]);
        let offset = 6 + 10 * 6; // испорчено 11-е повторение MAC
        p[offset] ^= 0xFF;
        assert!(!is_magic_packet(&p, &[MAC_A]));
    }

    #[test]
    fn rejects_when_no_macs_known() {
        // Сеть не отдала ни одного валидного MAC — пакет не признаётся.
        assert!(!is_magic_packet(&magic_packet(MAC_A, &[]), &[]));
    }
}
