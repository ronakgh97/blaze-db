use colored::{ColoredString, Colorize};

#[inline]
pub fn log(level: &str, msg: ColoredString) {
    let now = chrono::Local::now();

    let level = match level {
        "TRACE" => "TRACE".dimmed(),
        "DEBUG" => "DEBUG".blue().bold(),
        "INFO" => "INFO".bright_green().bold(),
        "WARN" => "WARN".yellow().bold(),
        "ERROR" => "ERROR".red().bold(),
        _ => level.normal(),
    };

    println!("[{}][{}] {}", now.format("%H:%M:%S"), level, msg);
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        {
            use colored::Colorize;
            $crate::utils::log::log("TRACE", format!($($arg)*).dimmed())
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        {
            use colored::Colorize;
            $crate::utils::log::log("DEBUG", format!($($arg)*).blue())
        }
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        {
            use colored::Colorize;
            $crate::utils::log::log("INFO", format!($($arg)*).bright_green())
        }
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        {
            use colored::Colorize;
            $crate::utils::log::log("WARN", format!($($arg)*).bright_yellow())
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        {
            use colored::Colorize;
            $crate::utils::log::log("ERROR", format!($($arg)*).bright_red())
        }
    };
}
