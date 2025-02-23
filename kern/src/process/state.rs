use core::fmt;

use alloc::boxed::Box;

use crate::process::Process;

/// Type of a function used to determine if a process is ready to be scheduled
/// again. The scheduler calls this function when it is the process's turn to
/// execute. If the function returns `true`, the process is scheduled. If it
/// returns `false`, the process is not scheduled, and this function will be
/// called on the next time slice.
pub type EventPollFn = Box<dyn FnMut(&mut Process) -> bool + Send>;

pub enum State {
    Ready,
    Waiting(Option<EventPollFn>), // Wrap it in an Option
    Running,
    Dead,
}

impl Clone for State {
    fn clone(&self) -> Self {
        match self {
            State::Ready => State::Ready,
            State::Running => State::Running,
            State::Waiting(_) => State::Waiting(None), // Drop function when cloning
            State::Dead => State::Dead,
        }
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            State::Ready => write!(f, "Ready"),
            State::Running => write!(f, "Running"),
            State::Waiting(_) => write!(f, "Waiting"),
            State::Dead => write!(f, "Dead"),
        }
    }
}