use godot::prelude::*;
use slab::Slab;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering::SeqCst},
};

use crate::api::{aslet::Aslet, task::AsletTask};

#[derive(Debug, Clone)]
pub struct Tasks(Arc<Mutex<Slab<Gd<AsletTask>>>>);

impl Tasks {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Slab::new())))
    }

    pub fn create(&self, aslet: Gd<Aslet>) -> (TaskContext, Gd<AsletTask>) {
        let mut tasks = self.0.lock().unwrap();
        let entry = tasks.vacant_entry();
        let id = entry.key();
        let task_ctx = TaskContext::new(id);
        let task = AsletTask::new(aslet, task_ctx.clone());

        entry.insert(task.clone());
        (task_ctx, task)
    }

    pub fn take(&self, key: usize) -> Option<Gd<AsletTask>> {
        let mut tasks = self.0.lock().unwrap();
        tasks.try_remove(key)
    }
}

#[derive(Debug, Clone)]
pub struct TaskContext(usize, Arc<AtomicU8>);

impl TaskContext {
    const WAITING: u8 = 0;
    const CANCELED: u8 = 1;
    const DONE: u8 = 2;

    pub fn new(id: usize) -> Self {
        Self(id, Arc::new(AtomicU8::new(Self::WAITING)))
    }

    pub fn cancel(&self) -> bool {
        self.1
            .compare_exchange(Self::WAITING, Self::CANCELED, SeqCst, SeqCst)
            .is_ok()
    }

    pub fn id(&self) -> usize {
        self.0
    }

    pub fn is_canceled(&self) -> bool {
        self.1.load(SeqCst) == Self::CANCELED
    }

    pub fn done(&self) {
        self.1.store(Self::DONE, SeqCst);
    }
}
