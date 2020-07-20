use std::sync::Arc;
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;

/// A basic thread pool with a constant number of threads.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

/// A job for a thread pool. The job may run on any thread, and will only be run once.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// A message to a thread.
enum Message {
    /// A new job to run.
    NewJob(Job),
    /// Message to tell the thread to return.
    Terminate,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for _ in 0..size {
            workers.push(new_worker(Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender }
    }

    /// Executes the given closure on a thread.
    pub fn execute<F>(&self, f: F)
        where
            F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.send(Message::NewJob(job)).unwrap();
    }
}

impl Drop for ThreadPool {
    /// Sends the termination message to all threads in the thread pool and waits for them to return.
    fn drop(&mut self) {
        for _ in &self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }

        for worker in &mut self.workers {
            if let Some(thread) = worker.take() {
                thread.join().unwrap();
            }
        }
    }
}

/// A worker, represented by a join handler if the thread is still running, or None.
type Worker = Option<thread::JoinHandle<()>>;

/// Creates a new worker with the given receiver end of an mpsc channel.
/// The worker will run until a Terminate message is sent to it through the channel.
fn new_worker(receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
    let thread = thread::spawn(move || loop {
        let message = receiver.lock().unwrap().recv().unwrap();

        match message {
            Message::NewJob(job) => job(),
            Message::Terminate => break
        }
    });

    Some(thread)
}

