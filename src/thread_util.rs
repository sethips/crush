use std::thread::JoinHandle;
use crate::errors::JobResult;
use std::thread;
use crate::commands::JobJoinHandle;

pub fn build(name: String) -> thread::Builder {
    thread::Builder::new().name(name)
}

pub fn handle(h: Result<JoinHandle<JobResult<()>>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}