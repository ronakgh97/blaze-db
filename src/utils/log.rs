use colored::{ColoredString, Colorize};
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl Level {
    fn from_env() -> Self {
        dotenv::dotenv().ok();

        env::var("RUST_LOG")
            .ok()
            .and_then(|s| match s.to_lowercase().as_str() {
                "trace" => Some(Level::Trace),
                "debug" => Some(Level::Debug),
                "info" => Some(Level::Info),
                "warn" => Some(Level::Warn),
                "error" => Some(Level::Error),
                _ => None,
            })
            .unwrap_or(Level::Info)
    }
}

lazy_static::lazy_static! {
    static ref LOG_LEVEL: Level = Level::from_env();
}

#[inline]
fn should_log(level: Level) -> bool {
    level >= *LOG_LEVEL
}

#[inline]
pub fn log(level: Level, msg: ColoredString) {
    if !should_log(level) {
        return;
    }

    let now = chrono::Local::now();

    let level_str = match level {
        Level::Trace => "TRACE".dimmed(),
        Level::Debug => "DEBUG".blue().bold(),
        Level::Info => "INFO".green().bold(),
        Level::Warn => "WARN".yellow().bold(),
        Level::Error => "ERROR".red().bold(),
    };

    println!("[{}][{}] {}", now.format("%H:%M:%S"), level_str, msg);
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        {
            use colored::Colorize;
            $crate::utils::log::log($crate::utils::log::Level::Trace, format!($($arg)*).dimmed())
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        {
            use colored::Colorize;
            $crate::utils::log::log($crate::utils::log::Level::Debug, format!($($arg)*).blue())
        }
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        {
            use colored::Colorize;
            $crate::utils::log::log($crate::utils::log::Level::Info, format!($($arg)*).green())
        }
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        {
            use colored::Colorize;
            $crate::utils::log::log($crate::utils::log::Level::Warn, format!($($arg)*).yellow())
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        {
            use colored::Colorize;
            $crate::utils::log::log($crate::utils::log::Level::Error, format!($($arg)*).red())
        }
    };
}
