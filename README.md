# WOL Service

Сервис для приёма Wake-on-LAN магических пакетов и выполнения заданной команды.

- Кроссплатформенность — Windows (служба) и Linux (systemd).
- Автоопределение MAC-адресов — реагирует только на пакеты, адресованные этой машине.
- Любая команда ОС передаётся через аргументы.
- Код написан при участии ИИ-ассистента.

# Сборка

## Windows:
```bat
cargo build --release --bin wol_service
```

## Linux (кросс-компиляция с Windows):
Требуется [Zig](https://ziglang.org/download/) в PATH.

```shell
rustup target add x86_64-unknown-linux-gnu
cargo install cargo-zigbuild
cargo zigbuild --release --target x86_64-unknown-linux-gnu --bin wol_service
```

Или всё одной командой: build.bat — fmt --check, clippy, сборка всех бинарей
под Windows и Linux (для Linux нужен zig в PATH). Артефакты складываются в dist\:
wol_service.exe и его вариант без расширения для Linux.

# Запуск

## Linux (systemd):

1. Скопировать бинарник и unit-файл:

   ```bash
   sudo cp wol_service /usr/local/bin/
   sudo cp wol-service.service /etc/systemd/system/
   ```

2. В wol-service.service задать ExecStart= — путь к бинарнику и команду.

3. Активировать:

   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable --now wol-service
   ```

## Windows:

*Служба:
1. Скопировать wol_service.exe в постоянную папку.
2. В install-service-windows.bat указать EXE_PATH и COMMAND.
3. Запустить .bat от имени администратора.

Удаление — uninstall-service-windows.bat от администратора.

## Изменение команды службы (без пересоздания):
```bat
sc config WOL-LISTENER binPath= "\"<путь>\wol_service.exe\" <новая команда>"
sc stop WOL-LISTENER
sc start WOL-LISTENER
```

Команда всегда берётся из binPath: диспетчер задач (колонка «Командная
строка») показывает ровно ту, что будет исполнена. Аргументы, переданные
`sc start`, сознательно игнорируются — фактическая команда не расходится
с видимой в списке процессов.

*Консольный режим (отладка; логи по умолчанию на уровне DEBUG — см. # Логирование):
```bat
wol_service.exe shutdown /s /t 0 /f
```

- Запуск без аргументов — только логирование.
- Всё, что после имени программы — команда для выполнения. Кавычки и пробелы в аргументах сохраняются.

```bash
./wol_service shutdown -h now
./wol_service /usr/local/bin/my-script.sh "arg with spaces"
./wol_service
```

```bat
wol_service.exe shutdown /s /t 0
wol_service.exe
```

# Логирование

Уровень по умолчанию:
- служба: INFO (журнал событий Windows / journald);
- консольный режим: DEBUG — отладочный, виден каждый пакет, ничего настраивать не нужно.

Переопределяется переменной WOL_LOG (синтаксис EnvFilter):
- WOL_LOG=info              # консоль без DEBUG — как у службы
- WOL_LOG=wol_service=debug # точечный уровень для крейта wol_service

Примеры: set WOL_LOG=info (Windows cmd) / WOL_LOG=info ./wol_service (Linux).

# Магический пакет

Сервис слушает UDP-порт 9 (0.0.0.0:9).

Валидный пакет:
- минимум 102 байта
- первые 6 байт: FF FF FF FF FF FF
- далее 16 повторений целевого MAC-адреса
- MAC должен быть в списке локальных интерфейсов

Debounce: одно срабатывание не чаще 10 секунд.

Обновление MAC-адресов — раз в минуту.

Фильтрация интерфейсов: исключаются loopback, виртуальные, Docker, VMware и прочие; учитываются только UP-интерфейсы с валидным MAC и IPv4.


# Лицензия
MIT

---

# WOL Service (English version)

Service for receiving Wake-on-LAN magic packets and executing a configured command.

- Cross-platform — Windows (as a service) and Linux (systemd).
- Automatic MAC address detection — reacts only to packets addressed to this machine.
- Any OS command is passed via arguments.
- The code was written with the assistance of an AI agent.

# Building

## Windows:
```bat
cargo build --release --bin wol_service
```

## Linux (cross-compilation from Windows):
Requires [Zig](https://ziglang.org/download/) in PATH.

```shell
rustup target add x86_64-unknown-linux-gnu
cargo install cargo-zigbuild
cargo zigbuild --release --target x86_64-unknown-linux-gnu --bin wol_service
```

Or everything with one command: build.bat — fmt --check, clippy, builds all
binaries for Windows and Linux (zig in PATH is required for Linux). Artifacts
are placed into dist\: wol_service.exe and its extensionless variant for Linux.

# Running

## Linux (systemd):

1. Copy the binary and the unit file:

   ```bash
   sudo cp wol_service /usr/local/bin/
   sudo cp wol-service.service /etc/systemd/system/
   ```

2. In wol-service.service set ExecStart= — the binary path and the command.

3. Activate:

   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable --now wol-service
   ```

## Windows:

*Service:
1. Copy wol_service.exe to a permanent folder.
2. In install-service-windows.bat set EXE_PATH and COMMAND.
3. Run the .bat as administrator.

Removal — uninstall-service-windows.bat as administrator.

## Changing the service command (without recreating the service):
```bat
sc config WOL-LISTENER binPath= "\"<path>\wol_service.exe\" <new command>"
sc stop WOL-LISTENER
sc start WOL-LISTENER
```

The command always comes from binPath: Task Manager (the "Command line"
column) shows exactly the command that will be executed. Arguments passed
to `sc start` are deliberately ignored — the effective command never
diverges from what is visible in the process list.

*Console mode (debugging; logging defaults to DEBUG — see # Logging):
```bat
wol_service.exe shutdown /s /t 0 /f
```

- Running without arguments — logging only.
- Everything after the program name — the command to execute. Quotes and spaces in arguments are preserved.

```bash
./wol_service shutdown -h now
./wol_service /usr/local/bin/my-script.sh "arg with spaces"
./wol_service
```

```bat
wol_service.exe shutdown /s /t 0
wol_service.exe
```

# Logging

Default level:
- service: INFO (Windows Event Log / journald);
- console mode: DEBUG — a debug mode, every packet is visible, nothing to configure.

Overridden by the WOL_LOG variable (EnvFilter syntax):
- WOL_LOG=info              # console without DEBUG — same as the service
- WOL_LOG=wol_service=debug # targeted level for the wol_service crate

Examples: set WOL_LOG=info (Windows cmd) / WOL_LOG=info ./wol_service (Linux).

# Magic packet

The service listens on UDP port 9 (0.0.0.0:9).

A valid packet:
- at least 102 bytes
- first 6 bytes: FF FF FF FF FF FF
- followed by 16 repetitions of the target MAC address
- the MAC must be in the list of local interfaces

Debounce: a trigger fires no more often than once per 10 seconds.

MAC addresses are refreshed once a minute.

Interfaces are filtered: loopback, virtual, Docker, VMware and others are excluded; only UP interfaces with a valid MAC and IPv4 are considered.

# License
MIT
