use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

use super::run_service;

pub fn run(command: Vec<OsString>) -> Result<(), Box<dyn std::error::Error>> {
    // stdout → journald. Под systemd (NOTIFY_SOCKET установлен) — INFO как у службы;
    // ручной запуск — отладочный DEBUG. Уровень задаётся переменной WOL_LOG.
    let default_level = if std::env::var_os("NOTIFY_SOCKET").is_some() {
        LevelFilter::INFO
    } else {
        LevelFilter::DEBUG
    };
    let filter = EnvFilter::builder()
        .with_env_var("WOL_LOG")
        .with_default_directive(default_level.into())
        .from_env_lossy();
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        info!("Получен сигнал завершения, останавливаемся...");
        r.store(false, Ordering::SeqCst);
    }) {
        // Без обработчика systemd остановит процесс по SIGTERM принудительно.
        error!(error = %e, "Не удалось установить обработчик сигналов");
    }

    sd_notify::notify(true, &[sd_notify::NotifyState::Ready])
        .unwrap_or_else(|e| error!("Не удалось отправить READY=1: {}", e));

    info!("WOL сервис запущен");

    let mut usec: u64 = 0;
    let watchdog_enabled = sd_notify::watchdog_enabled(true, &mut usec);
    let heartbeat_interval = if watchdog_enabled {
        Some(Duration::from_micros(usec) / 2)
    } else {
        None
    };

    if heartbeat_interval.is_some() {
        info!("Systemd watchdog активен");
    }

    let running_hb = running.clone();
    if let Some(interval) = heartbeat_interval {
        std::thread::spawn(move || {
            while running_hb.load(Ordering::SeqCst) {
                std::thread::sleep(interval);
                if running_hb.load(Ordering::SeqCst) {
                    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Watchdog]);
                }
            }
        });
    }

    run_service(running, &command)?;
    // Ошибка (например, порт занят) уйдёт в main как код выхода 1 —
    // journald зафиксирует её, а не «тихий» успешный останов.

    info!("WOL сервис остановлен");
    Ok(())
}
