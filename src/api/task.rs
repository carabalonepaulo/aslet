use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering::SeqCst},
};

use godot::prelude::*;
use sharded_slab::Slab;

pub fn create(slab: &Slab<Gd<AsletTask>>) -> (TaskContext, Gd<AsletTask>) {
    let entry = slab.vacant_entry().unwrap();
    let id = entry.key();
    let task_ctx = TaskContext::new(id);
    let task = AsletTask::new(task_ctx.clone());

    entry.insert(task.clone());
    (task_ctx, task)
}

/// Represents an asynchronous operation in progress.
///
/// An `AsletTask` can be canceled before it completes, and will emit
/// the [`done`] signal once finished.
#[derive(GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct AsletTask {
    ctx: TaskContext,
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
    pub fn new(ctx: TaskContext) -> Gd<Self> {
        Gd::from_object(Self { ctx })
    }

    /// Attempts to cancel the task if it is still waiting.
    ///
    /// Returns:
    /// - `OK` if the task was successfully canceled.
    /// - `FAILED` if the task was already running or finished.
    #[func]
    pub fn cancel(&self) -> godot::global::Error {
        if self.ctx.cancel() {
            godot::global::Error::OK
        } else {
            godot::global::Error::FAILED
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskContext(Arc<(usize, AtomicU8)>);

impl TaskContext {
    const WAITING: u8 = 0;
    const CANCELED: u8 = 1;
    const DONE: u8 = 2;

    pub fn new(id: usize) -> Self {
        Self(Arc::new((id, AtomicU8::new(Self::WAITING))))
    }

    fn cancel(&self) -> bool {
        self.0
            .1
            .compare_exchange(Self::WAITING, Self::CANCELED, SeqCst, SeqCst)
            .is_ok()
    }

    pub fn id(&self) -> usize {
        self.0.0
    }

    pub fn is_canceled(&self) -> bool {
        self.0.1.load(SeqCst) == Self::CANCELED
    }

    pub fn done(&self) {
        self.0.1.store(Self::DONE, SeqCst);
    }
}
