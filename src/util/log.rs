use std::fmt::Arguments;
use std::mem;
use std::sync::{Arc, Once};

use crate::util::log::LogStatus::{Debug, Error, Info, Warning};

#[derive(Clone)]
pub struct GlobalLogger {
    inner: Arc<dyn Logger>
}

impl Logger for GlobalLogger {
    fn log(&self, status: LogStatus, args: Arguments<'_>) {
        self.inner.log(status, args)
    }
}

static mut GLOBAL_LOGGER: *const GlobalLogger = 0 as *const GlobalLogger;

pub fn initialize_global(logger: impl Logger + 'static) {
    static ONCE: Once = Once::new();

    unsafe {
        ONCE.call_once(|| {
            let logger = GlobalLogger {
                inner: Arc::new(logger)
            };
            GLOBAL_LOGGER = mem::transmute(Box::new(logger));
        })
    }
}

fn global() -> GlobalLogger {
    unsafe {
        if GLOBAL_LOGGER.is_null() {
            panic!("Logger has not been initialized!");
        }
        (*GLOBAL_LOGGER).clone()
    }
}

#[derive(std::fmt::Debug, Copy, Clone, PartialEq, Eq)]
pub enum LogStatus {
    Debug,
    Info,
    Warning,
    Error,
}

pub trait Logger: Sync + Send {
    fn log(&self, status: LogStatus, args: Arguments<'_>);

    fn debug(&self, args: Arguments<'_>) {
        self.log(Debug, args)
    }

    fn info(&self, args: Arguments<'_>) {
        self.log(Info, args)
    }

    fn warn(&self, args: Arguments<'_>) {
        self.log(Warning, args)
    }

    fn error(&self, args: Arguments<'_>) {
        self.log(Error, args)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Arguments;
    use std::ops::Deref;
    use std::sync::{Arc, Mutex};
    use std::thread::spawn;

    use crate::util::log::{global, initialize_global, Logger, LogStatus};
    use crate::util::log::LogStatus::{Debug, Error, Info, Warning};

    struct MockLogger {
        calls: Arc<Mutex<Vec<(LogStatus, String)>>>
    }

    impl Logger for MockLogger {
        fn log(&self, status: LogStatus, args: Arguments<'_>) {
            self.calls.lock().unwrap().push((status, format!("{}", args)))
        }
    }

    #[test]
    fn global_logger() {
        let calls = Arc::new(Mutex::new(vec![]));
        initialize_global(MockLogger { calls: Arc::clone(&calls) });
        global().log(Info, format_args!("hello"));
        global().log(Warning, format_args!("hello 2"));
        global().log(Error, format_args!("hello 3"));
        global().log(Debug, format_args!("hello 4"));

        let mut handlers = vec![];
        for _ in 0..10 {
            handlers.push(spawn(|| {
                global().log(Info, format_args!("hello"));
            }));
        }

        for handler in handlers {
            handler.join().unwrap();
        }

        assert_eq!(
            &vec![(Info, "hello".to_string()),
                  (Warning, "hello 2".to_string()),
                  (Error, "hello 3".to_string()),
                  (Debug, "hello 4".to_string()),
                  (Info, "hello".to_string()),
                  (Info, "hello".to_string()),
                  (Info, "hello".to_string()),
                  (Info, "hello".to_string()),
                  (Info, "hello".to_string()),
                  (Info, "hello".to_string()),
                  (Info, "hello".to_string()),
                  (Info, "hello".to_string()),
                  (Info, "hello".to_string()),
                  (Info, "hello".to_string())],
            calls.lock().unwrap().deref());
    }
}