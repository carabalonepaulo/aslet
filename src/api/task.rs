use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering::SeqCst},
};

use godot::prelude::*;
use sharded_slab::Slab;

pub fn create(slab: &Slab<Gd<AsletTask>>) -> (TaskContext, Gd<AsletTask>) {
    let entry = slab.vacant_entry().unwrap();
    let state = Arc::new(AtomicU8::new(WAITING));
    let id = entry.key();
    let task = AsletTask::new(state.clone());
    let task_ctx = TaskContext::new(id, state);

    entry.insert(task.clone());
    (task_ctx, task)
}

pub const WAITING: u8 = 0;
pub const CANCELED: u8 = 1;
pub const DONE: u8 = 2;

/// Represents an asynchronous operation in progress.
///
/// An `AsletTask` can be canceled before it completes, and will emit
/// the [`done`] signal once finished.
#[derive(GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct AsletTask {
    state: Arc<AtomicU8>,
}

#[godot_api]
impl AsletTask {
    /// Emitted when the task finishes execution.
    /// The `result` is an array where:
    /// - the first element is `OK` or `FAILED`,
    /// - if the first element is `OK`, the second element contains the operation's data.
    #[signal]
    fn done(result: Variant);

    /// Creates a new [`AsletTask`] with the given internal state.
    pub fn new(state: Arc<AtomicU8>) -> Gd<Self> {
        Gd::from_object(Self { state })
    }

    /// Attempts to cancel the task if it is still waiting.
    ///
    /// Returns:
    /// - `OK` if the task was successfully canceled.
    /// - `FAILED` if the task was already running or finished.
    #[func]
    pub fn cancel(&self) -> godot::global::Error {
        match self
            .state
            .compare_exchange(WAITING, CANCELED, SeqCst, SeqCst)
        {
            Ok(_) => godot::global::Error::OK,
            Err(_) => godot::global::Error::FAILED,
        }
    }
}

#[derive(Debug)]
pub struct TaskContext(usize, Arc<AtomicU8>);

impl TaskContext {
    pub fn new(id: usize, state: Arc<AtomicU8>) -> Self {
        Self(id, state)
    }

    pub fn id(&self) -> usize {
        self.0
    }

    pub fn is_canceled(&self) -> bool {
        self.1.load(SeqCst) == CANCELED
    }

    pub fn done(&self) {
        self.1.store(DONE, SeqCst);
    }
}
