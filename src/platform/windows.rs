use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};
use tracing_subscriber::{filter::LevelFilter, layer::Layer as _, prelude::*, EnvFilter};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

use super::run_service;

const SERVICE_NAME: &str = "WOL-LISTENER";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

fn init_logging() {
    match tracing_layer_win_eventlog::EventLogLayer::new(SERVICE_NAME) {
        Ok(layer) => {
            // В журнал событий пишем только INFO и выше: на порт 9 прилетает
            // много постороннего трафика, не стоит засорять им журнал.
            tracing_subscriber::registry()
                .with(layer.with_filter(LevelFilter::INFO))
                .init();
        }
        Err(e) => {
            // Слой журнала событий недоступен — откатываемся на stdout.
            eprintln!("Не удалось создать слой журнала событий: {}", e);
            tracing_subscriber::fmt().init();
        }
    }
}
define_windows_service!(ffi_service_main, my_service_main);

fn my_service_main(_arguments: Vec<OsString>) {
    init_logging();

    // Единственный источник команды — командная строка процесса (binPath):
    // то, что диспетчер задач показывает, — то служба и исполнит.
    // ServiceMain argv (args канала StartService) преднамеренно игнорируется,
    // иначе фактическая команда расходилась бы с видимой в списке процессов.
    // Смена команды без пересоздания службы: sc config binPath= ... + restart.
    let command: Vec<OsString> = std::env::args_os().skip(1).collect();

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                running_clone.store(false, Ordering::SeqCst);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = match service_control_handler::register(SERVICE_NAME, event_handler) {
        Ok(handle) => handle,
        Err(e) => {
            error!(error = %e, "Не удалось зарегистрировать обработчик службы");
            return;
        }
    };

    // Пока инициализируемся (регистрация, занятие порта), сообщаем SCM о запуске.
    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(1),
        process_id: None,
    }) {
        error!(error = ?e, "Не удалось установить статус 'Запуск'");
        return;
    }

    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }) {
        error!(error = ?e, "Не удалось установить статус 'Запущен'");
        return;
    }

    let result = run_service(running, &command);
    if let Err(e) = &result {
        error!(
            error = %e,
            "Служба завершилась из-за ошибки; SCM применит политику восстановления"
        );
    }
    // Ненулевой код выхода — чтобы SCM считал остановку сбоем службы
    // и перезапустил её согласно `sc failure`.
    let exit_code = match &result {
        Ok(()) => ServiceExitCode::Win32(0),
        Err(_) => ServiceExitCode::Win32(1),
    };

    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }) {
        error!(error = ?e, "Не удалось установить статус 'Остановлен'");
    }
    info!("Сервис '{}' остановлен.", SERVICE_NAME);
}

fn run_console(command: Vec<OsString>) -> std::io::Result<()> {
    // stdout; консольный режим — отладочный: по умолчанию DEBUG, чтобы
    // видеть каждый пакет. Уровень задаётся переменной WOL_LOG (EnvFilter):
    // например, WOL_LOG=info отключит DEBUG-логи.
    let filter = EnvFilter::builder()
        .with_env_var("WOL_LOG")
        .with_default_directive(LevelFilter::DEBUG.into())
        .from_env_lossy();
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        info!("Получен сигнал завершения, останавливаемся...");
        r.store(false, Ordering::SeqCst);
    }) {
        error!(error = %e, "Не удалось установить обработчик Ctrl+C");
    }

    info!("WOL сервис запущен в консольном режиме");
    run_service(running, &command)?;
    info!("WOL сервис остановлен");
    Ok(())
}

pub fn run(command: Vec<OsString>) -> Result<(), Box<dyn std::error::Error>> {
    match service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Ошибка 1063 (ERROR_FAILED_SERVICE_CONTROLLER_CONNECT) — процесс
            // запущен не из SCM: работаем в консольном режиме.
            let is_console = match &e {
                windows_service::Error::Winapi(io_err) => io_err.raw_os_error() == Some(1063),
                _ => false,
            };

            if is_console {
                run_console(command)?;
                Ok(())
            } else {
                Err(Box::new(e))
            }
        }
    }
}
