mod time_manager;

use std::fs::{create_dir, read_dir, remove_file, DirEntry, OpenOptions};
use std::io::{Error, Write};
use std::path::Path;
use std::sync::{mpsc, Mutex};
use std::thread;
use log::{Metadata, Record, Level, SetLoggerError, LevelFilter};

/// Struct holding sender to dedicated logging thread
pub struct LoggingService {
    sender: Mutex<mpsc::Sender<LoggingCommands>>,
}

// struct holding the body of a message to log
struct MessageBody {
    level: String,
    content: String,
}

enum LoggingCommands {
    Kill,
    Message(MessageBody),
}

/// Configuration struct for Logging service
pub struct LoggingConfig {
    /// Path from executable to directory to be used for log files
    pub logging_directory: &'static Path,
    /// The maximum size of the logging directory in bytes
    pub max_dir_size: usize,
}

impl LoggingService {

    /// Create a new LoggingService instance holding the sender to the dedicated logging thread.
    ///
    /// # Arguments
    ///
    /// * `options` - LoggingConfig struct containing the options for the new logging service instance.
    ///
    pub fn new(options: LoggingConfig) -> Result<(), SetLoggerError> {
        let (sender, receiver) = mpsc::channel();

        // kick off logging thread
        thread::spawn(move || loop {
            match receiver.recv().unwrap() {
                LoggingCommands::Message(message) => {
                    log(message, &options)
                        .expect("Logging service failed when receiving message.");
                }
                LoggingCommands::Kill => break,
            }
        });

        // box logger
        let logger = Box::new(LoggingService { sender: Mutex::new(sender) });

        // set global logger
        log::set_boxed_logger(logger)
            .map(|()| log::set_max_level(LevelFilter::Info))?;

        Ok(())

    }

}

impl Drop for LoggingService {
    fn drop(&mut self) {
        self.sender.lock().unwrap()
            .send(LoggingCommands::Kill)
            .expect("Failed to kill logging service on drop.");
    }
}

impl log::Log for LoggingService {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {

        // convert level to string
        let level = match record.level() {
            Level::Error => String::from(" ERROR "),
            Level::Debug => String::from(" DEBUG "),
            Level::Info => String::from(" INFO  "),
            Level::Trace => String::from(" TRACE "),
            Level::Warn => String::from(" WARN  "),
        };

        self.sender.lock().unwrap()
            .send(LoggingCommands::Message(MessageBody { content: record.args().to_string(), level}))
            .expect("Failed to send message to logging service.");
    }

    fn flush(&self) {
        unimplemented!()
    }
}

// write a message to a log file
// writes the given message to a log file for the current date in the logging directory
// a file will be created in the logging directory specified by the logging config containing the message
// the file will be titled with the current unix date in the format "YYYY_MM_DD.log"
// the message will be preceded with a unix timestamp in the format "[YYYY-MM-DD HH:MM:SS]"
fn log(message_body: MessageBody, options: &LoggingConfig) -> Result<(), Error> {
    // create logging dir if needed
    if !options.logging_directory.exists() {
        create_dir(&options.logging_directory)?;
    } else {
        check_size(options)?;
    }

    // path to file
    let log_file_path = options
        .logging_directory
        .join(format!("{}.log", time_manager::curr_datestamp()));

    // create or open
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(log_file_path)?;

    // write message
    file.write_all((time_manager::curr_timestamp() + message_body.level.as_str() + message_body.content.as_str() + "\n").as_bytes())
}

// checks the size of the directory, deleting oldest files if too big
fn check_size(options: &LoggingConfig) -> Result<(), Error> {
    // get sorted Vec of DirEntries
    let files = get_sorted_files_from_dir(options.logging_directory)?;

    // check size of each file
    let mut total_size: usize = 0;
    let mut start_index: usize = 0;
    for i in 0..files.len() {
        // add file size to total
        total_size += files.get(i).unwrap().metadata()?.len() as usize;

        // delete oldest files until size is small enough
        while total_size > options.max_dir_size && start_index <= i {
            total_size -= files.get(start_index).unwrap().metadata()?.len() as usize;
            remove_file(files.get(start_index).unwrap().path())?;
            start_index += 1;
        }
    }

    Ok(())
}

// gets a sorted list (old to new) of logging files from logging dir
fn get_sorted_files_from_dir(logging_directory: &Path) -> Result<Vec<DirEntry>, Error> {
    // files to be sorted
    let mut files: Vec<DirEntry> = Vec::new();

    // get all files in dir
    for file in read_dir(logging_directory)? {
        let file = file?;

        // check file type and name
        if file.file_type()?.is_file() {
            // get file name
            let filename = match file.file_name().into_string() {
                Ok(filename) => filename,
                Err(_) => continue,
            };

            // check filename
            if filename.ends_with(".log") && time_manager::check_date(&filename[0..10]) {
                files.push(file);
            }
        }
    }

    // sort files by date
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{remove_dir_all, File};
    use std::thread;
    use std::time;
    use std::error::Error;
    use log::*;

    #[test]
    fn test_log() -> Result<(), Box<dyn Error>> {
        let logging_directory = Path::new("./test_logs/");
        LoggingService::new(LoggingConfig {
            logging_directory,
            max_dir_size: 10000,
        })?;
        let current_date = time_manager::curr_datestamp();
        info!("test message");
        thread::sleep(time::Duration::from_millis(10));
        assert_eq!(
            true,
            logging_directory
                .join(format!("{}.log", current_date))
                .as_path()
                .exists()
        );
        remove_dir_all(logging_directory)?;
        assert_eq!(false, logging_directory.exists());
        Ok(())
    }

    #[test]
    fn test_sorted_files() -> std::io::Result<()> {
        let curr_date = format!("{}.log", time_manager::curr_datestamp());
        let files = ["2020_01_12.log", "2020_03_14.log", curr_date.as_str()];
        let logging_directory = Path::new("./test_logs2/");
        create_dir(logging_directory)?;
        for filename in files.iter() {
            File::create(logging_directory.join(filename).as_path())?;
        }
        let sorted = get_sorted_files_from_dir(logging_directory)?;
        for i in 0..files.len() {
            assert_eq!(files[i], sorted[i].file_name().to_str().unwrap());
        }
        remove_dir_all(logging_directory)?;
        assert_eq!(false, logging_directory.exists());
        Ok(())
    }
}
