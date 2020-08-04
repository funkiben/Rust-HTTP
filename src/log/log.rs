use std::fmt::Arguments;
use std::mem;
use std::sync::{Arc, Once};

#[derive(Clone)]
struct GlobalLogger {
    inner: Option<Arc<dyn Logger>>
}

impl Logger for GlobalLogger {
    fn log(&self, level: LogLevel, args: Arguments<'_>) {
        if let Some(ref logger) = self.inner {
            logger.log(level, args)
        }
    }
}

static mut GLOBAL_LOGGER: *const GlobalLogger = 0 as *const GlobalLogger;

pub fn set_global(logger: impl Logger + 'static) {
    static ONCE: Once = Once::new();

    unsafe {
        ONCE.call_once(|| {
            let logger = GlobalLogger {
                inner: Some(Arc::new(logger))
            };
            GLOBAL_LOGGER = mem::transmute(Box::new(logger));
        })
    }
}

pub fn global() -> impl Logger {
    unsafe {
        if GLOBAL_LOGGER.is_null() {
            GlobalLogger { inner: None }
        } else {
            (*GLOBAL_LOGGER).clone()
        }
    }
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "log-debug")]
        $crate::log::global().debug(format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        #[cfg(feature = "log-info")]
        $crate::log::global().info(format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "log-warn")]
        $crate::log::global().warn(format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        #[cfg(feature = "log-error")]
        $crate::log::global().error(format_args!($($arg)*))
    }
}

#[derive(std::fmt::Debug, Copy, Clone, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

pub trait Logger: Sync + Send {
    fn log(&self, level: LogLevel, args: Arguments<'_>);

    fn debug(&self, args: Arguments<'_>) {
        self.log(LogLevel::Debug, args)
    }

    fn info(&self, args: Arguments<'_>) {
        self.log(LogLevel::Info, args)
    }

    fn warn(&self, args: Arguments<'_>) {
        self.log(LogLevel::Warn, args)
    }

    fn error(&self, args: Arguments<'_>) {
        self.log(LogLevel::Error, args)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Arguments;
    use std::ops::Deref;
    use std::sync::{Arc, Mutex};
    use std::thread::spawn;

    use crate::log::{global, Logger, LogLevel, set_global};

    struct MockLogger {
        calls: Arc<Mutex<Vec<(LogLevel, String)>>>
    }

    impl Logger for MockLogger {
        fn log(&self, level: LogLevel, args: Arguments<'_>) {
            self.calls.lock().unwrap().push((level, format!("{}", args)))
        }
    }

    #[test]
    fn no_global_logger_set() {
        global().info(format_args!("hello"));
        global().warn(format_args!("hello 2"));
        global().error(format_args!("hello 3"));
        global().debug(format_args!("hello 4"));
    }

    #[test]
    fn global_logger() {
        let calls = Arc::new(Mutex::new(vec![]));
        set_global(MockLogger { calls: Arc::clone(&calls) });
        global().info(format_args!("hello"));
        global().warn(format_args!("hello 2"));
        global().error(format_args!("hello 3"));
        global().debug(format_args!("hello 4"));

        let mut handlers = vec![];
        for _ in 0..10 {
            handlers.push(spawn(|| {
                global().info(format_args!("hello"));
            }));
        }

        for handler in handlers {
            handler.join().unwrap();
        }

        assert_eq!(
            &vec![(LogLevel::Info, "hello".to_string()),
                  (LogLevel::Warn, "hello 2".to_string()),
                  (LogLevel::Error, "hello 3".to_string()),
                  (LogLevel::Debug, "hello 4".to_string()),
                  (LogLevel::Info, "hello".to_string()),
                  (LogLevel::Info, "hello".to_string()),
                  (LogLevel::Info, "hello".to_string()),
                  (LogLevel::Info, "hello".to_string()),
                  (LogLevel::Info, "hello".to_string()),
                  (LogLevel::Info, "hello".to_string()),
                  (LogLevel::Info, "hello".to_string()),
                  (LogLevel::Info, "hello".to_string()),
                  (LogLevel::Info, "hello".to_string()),
                  (LogLevel::Info, "hello".to_string())],
            calls.lock().unwrap().deref());

        calls.lock().unwrap().clear();
        debug!("hello {} {} {}", 1, 2, 3);
        info!("hello {} {} {}", 1, 2, 3);
        warn!("hello {} {} {}", 1, 2, 3);
        error!("hello {} {} {}", 1, 2, 3);
        assert_eq!(
            &vec![(LogLevel::Debug, "hello 1 2 3".to_string()),
                  (LogLevel::Info, "hello 1 2 3".to_string()),
                  (LogLevel::Warn, "hello 1 2 3".to_string()),
                  (LogLevel::Error, "hello 1 2 3".to_string())],
            calls.lock().unwrap().deref());

    }
}