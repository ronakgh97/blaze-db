use colored::Colorize;

pub fn log(level: &str, msg: &str) {
    let now = chrono::Local::now();

    let level = match level {
        "INFO" => "INFO".green().bold(),
        "WARN" => "WARN".yellow().bold(),
        "ERROR" => "ERROR".red().bold(),
        _ => level.normal(),
    };

    println!("[{}][{}] {}", now.format("%H:%M:%S"), level, msg);
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        log("INFO", &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        log("WARN", &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        log("ERROR", &format!($($arg)*));
    };
}
