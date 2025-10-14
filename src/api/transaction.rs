use std::sync::{
    Arc,
    atomic::{
        AtomicU8,
        Ordering::{self},
    },
};

use godot::prelude::*;
use sharded_slab::Slab;

use crate::{
    api::task::AsletTask,
    worker::{Worker, messages::InputMessage},
};

#[derive(Debug, Clone)]
pub struct TransactionState(Arc<AtomicU8>);

impl TransactionState {
    const ACTIVE: u8 = 0;
    const COMMITTED: u8 = 1;
    const ROLLED_BACK: u8 = 2;

    pub fn new() -> Self {
        Self(Arc::new(AtomicU8::new(Self::ACTIVE)))
    }

    pub fn is_active(&self) -> bool {
        self.0.load(Ordering::Acquire) == Self::ACTIVE
    }

    pub fn commit(&self) {
        self.0.store(Self::COMMITTED, Ordering::Release);
    }

    pub fn rollback(&self) {
        self.0.store(Self::ROLLED_BACK, Ordering::Release);
    }
}

/// Represents a database transaction.
///
/// A `Transaction` groups multiple operations (queries, inserts, updates)
/// into a single unit of work. Changes can be committed or rolled back.
///
/// Each `Transaction` holds its own dedicated database connection rather than
/// sharing one. This isolation ensures that the transaction’s state remains
/// unaffected by asynchronous operations or concurrent interactions occurring
/// on other connections.
#[derive(GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct AsletTransaction {
    conn_id: usize,
    worker: Worker,
    state: TransactionState,
    tasks: Arc<Slab<Gd<AsletTask>>>,
}

#[godot_api]
impl AsletTransaction {
    /// Creates a new [`AsletTransaction`].
    pub fn new(conn_id: usize, worker: Worker, tasks: Arc<Slab<Gd<AsletTask>>>) -> Gd<Self> {
        Gd::from_object(Self {
            conn_id,
            worker,
            tasks,
            state: TransactionState::new(),
        })
    }

    /// Executes a SQL statement that does not return rows.
    ///
    /// This function is used for statements like `INSERT`, `UPDATE`, or `DELETE`.
    /// It sends the SQL command and parameters to the worker thread and returns an [`AsletTask`].
    ///
    /// # Parameters
    ///
    /// * `sql` — The SQL statement to execute.
    /// * `params` — Statement parameters to bind, as an array of [`Variant`] values.
    ///
    /// # Returns
    ///
    /// Returns an [`AsletTask`] that yields **once**, producing a [`VariantArray`] with one of the following forms:
    ///
    /// * `[OK, n]` — statement executed successfully, where `n` is the number of affected rows.
    /// * `[FAILED, errmsg]` — execution failed, with `errmsg` containing the error message.
    ///
    /// # Example
    /// ```gdscript
    /// var result := await tx.exec("delete from test", []).done as Array
    /// if result[0] == OK:
    ///     print("Deleted rows:", result[1])
    /// else:
    ///     push_error(result[1])
    /// ```
    #[func]
    fn exec(&self, sql: GString, params: Array<Variant>) -> Gd<AsletTask> {
        let (task_ctx, task) = super::task::create(&self.tasks);
        self.worker.send(InputMessage::Exec(
            self.conn_id,
            task_ctx,
            sql.into(),
            params.into(),
        ));
        task
    }

    /// Executes a SQL query and retrieves rows.
    ///
    /// This function sends the query and its parameters to the worker thread for execution.
    /// It returns an [`AsletTask`] representing the asynchronous operation.
    ///
    /// # Parameters
    ///
    /// * `sql` — The SQL query to execute.
    /// * `params` — Query parameters to bind, as an array of [`Variant`] values.
    ///
    /// # Returns
    ///
    /// Returns an [`AsletTask`] that yields **once**, producing a [`VariantArray`] with one of the following forms:
    ///
    /// * `[OK, rows]` — query executed successfully, with `rows` as an `Array[Array[Variant]]`
    ///   where each inner array represents a row.
    /// * `[FAILED, errmsg]` — query failed, with `errmsg` containing the error message.
    ///
    /// # Example
    /// ```gdscript
    /// var result := await tx.fetch("select * from test", []).done as Array
    /// if result[0] == OK:
    ///     for row in result[1]: print(row) # [id, name, value]
    /// else:
    ///     push_error(result[1])
    /// ```
    #[func]
    fn fetch(&self, sql: GString, params: Array<Variant>) -> Gd<AsletTask> {
        let (task_ctx, task) = super::task::create(&self.tasks);
        self.worker.send(InputMessage::Fetch(
            self.conn_id,
            task_ctx,
            sql.into(),
            params.into(),
        ));
        task
    }

    /// Commits all changes made during the transaction.
    ///
    /// This function finalizes the transaction, permanently applying all changes
    /// made since it began. Once committed, the transaction becomes invalid for further use.
    ///
    /// # Returns
    ///
    /// Returns an [`AsletTask`] that yields **once**, producing a [`VariantArray`] with one of the following forms:
    ///
    /// * `[OK]` — commit done successfully.
    /// * `[FAILED, errmsg]` — commit failed, with `errmsg` containing the error message.
    ///
    /// # Example
    /// ```gdscript
    /// var result := await tx.commit().done as Array
    /// if result[0] == OK:
    ///     print("Transaction committed")
    /// else:
    ///     push_error(result[1])
    /// ```
    #[func]
    fn commit(&self) -> Gd<AsletTask> {
        let (task_ctx, task) = super::task::create(&self.tasks);
        self.worker.send(InputMessage::Commit(
            task_ctx,
            self.conn_id,
            self.state.clone(),
        ));
        task
    }

    /// Rolls back all changes made during the transaction.
    ///
    /// This function reverts all operations performed within the transaction,
    /// restoring the database to its previous state.
    ///
    /// # Returns
    ///
    /// Returns an [`AsletTask`] that yields **once**, producing a [`VariantArray`] with one of the following forms:
    ///
    /// * `[OK]` — transaction rolled back successfully.
    /// * `[FAILED, errmsg]` — rollback failed, with `errmsg` containing the error message.
    ///
    /// # Example
    /// ```gdscript
    /// var result := await tx.rollback().done as Array
    /// if result[0] == FAILED:
    ///     push_error(result[1])
    /// ```
    #[func]
    fn rollback(&self) -> Gd<AsletTask> {
        let (task_ctx, task) = super::task::create(&self.tasks);
        self.worker.send(InputMessage::Rollback(
            task_ctx,
            self.conn_id,
            self.state.clone(),
        ));
        task
    }
}

impl Drop for AsletTransaction {
    fn drop(&mut self) {
        self.rollback();
    }
}
