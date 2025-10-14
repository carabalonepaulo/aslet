use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam::channel::{Receiver, RecvTimeoutError};
use godot::{global::printerr, prelude::*};
use sharded_slab::Slab;

use crate::{
    api::{
        conn::AsletConn,
        task::{AsletTask, TaskContext},
        transaction::AsletTransaction,
    },
    failed, ok,
    result::variant_from_result,
    worker::{
        Worker,
        messages::{InputMessage, OutputMessage},
    },
};

/// Main entry point for interacting with the database.
///
/// Provides methods to open connections, run queries, perform batch inserts,
/// manage transactions, and poll for async results.
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct Aslet {
    tasks: Arc<Slab<Gd<AsletTask>>>,
    base: Base<RefCounted>,
    worker: Worker,
    output_receiver: Receiver<OutputMessage>,
}

#[godot_api]
impl IRefCounted for Aslet {
    fn init(base: Base<RefCounted>) -> Self {
        let tasks = Arc::new(Slab::new());
        let (worker, output_receiver) = Worker::new();

        Self {
            base,
            tasks,
            worker,
            output_receiver,
        }
    }
}

#[godot_api]
impl Aslet {
    /// Opens a database file at the given path.
    ///
    /// Returns an [`AsletTask`] representing the asynchronous operation.  
    /// The task yields **once**, returning a [`VariantArray`] with one of the following forms:
    ///
    /// * `[OK, db]` — database opened successfully, `db` is the [`AsletConn`] instance.
    /// * `[FAILED, errmsg]` — failed to open the database, with `errmsg` containing the error message.
    ///
    /// # Parameters
    ///
    /// * `path` — Path to the database file to open.
    #[func]
    fn open(&self, path: String) -> Gd<AsletTask> {
        let (task_ctx, task) = super::task::create(&self.tasks);
        self.worker
            .send(InputMessage::Open(task_ctx, path.to_string()));
        task
    }

    /// Polls for completion of asynchronous tasks.
    ///
    /// Waits up to `timeout_ms` milliseconds for any pending task to complete.
    /// If a task completes during this time, its `done` signal will be emitted with the result.
    ///
    /// # Parameters
    ///
    /// * `timeout_ms` — Maximum time in milliseconds to wait for task completion.
    #[func]
    fn poll(&self, timeout_ms: u64) {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }

            let remaining = deadline - now;
            match self.output_receiver.recv_timeout(remaining) {
                Ok(msg) => self.handle_msg(msg),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn handle_msg(&self, msg: OutputMessage) {
        match msg {
            OutputMessage::Open(task_ctx, result) => match result {
                Ok((conn, path)) => {
                    let aslet_conn =
                        AsletConn::new(conn, path, self.worker.clone(), self.tasks.clone());
                    self.complete_task(task_ctx, ok!(aslet_conn));
                }
                Err(err) => self.complete_task(task_ctx, failed!(err)),
            },
            OutputMessage::Exec(task_ctx, result) => {
                self.complete_task(task_ctx, variant_from_result(result));
            }
            OutputMessage::Fetch(task_ctx, result) => match result {
                Ok((rows, columns)) => {
                    self.complete_task(task_ctx, ok!(rows, columns));
                }
                Err(err) => {
                    self.complete_task(task_ctx, failed!(err));
                }
            },
            OutputMessage::TransactionStarted(task_ctx, result) => match result {
                Ok(conn) => {
                    let transaction =
                        AsletTransaction::new(conn, self.worker.clone(), self.tasks.clone());
                    self.complete_task(task_ctx, ok!(transaction));
                }
                Err(err) => self.complete_task(task_ctx, failed!(err)),
            },
            OutputMessage::TransactionRolledBack(task_ctx, result)
            | OutputMessage::TransactionCommitted(task_ctx, result) => {
                self.complete_task(task_ctx, variant_from_result(result));
            }
            OutputMessage::Backup(task_ctx, result) => match result {
                Ok(backup_state) => {
                    let main = backup_state.main();
                    main.tick();
                    if main.is_done() {
                        self.complete_task(task_ctx, ok!());
                    } else {
                        self.worker
                            .send(InputMessage::BackupStep(task_ctx, backup_state));
                    }
                }
                Err(e) => {
                    self.complete_task(task_ctx, failed!(e));
                }
            },
            OutputMessage::Canceled(task_ctx) => {
                self.complete_task(task_ctx, failed!("task canceled"))
            }
        }
    }

    #[inline]
    fn complete_task(&self, task_ctx: TaskContext, result: Array<Variant>) {
        let task_id = task_ctx.id();
        if let Some(mut task) = self.tasks.take(task_id) {
            task.emit_signal("done", &[result.to_variant()]);
        } else {
            printerr(&[format!("invalid task {}", task_id).to_variant()]);
        }
    }
}
