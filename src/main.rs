mod platform;

use std::ffi::OsString;

fn main() {
    // Не склеиваем аргументы в одну строку: кавычки и пробелы
    // в аргументах команды нужно сохранить как есть.
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if let Err(e) = platform::run(args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
