use std::sync::Arc;

use godot::prelude::*;
use sharded_slab::Slab;

use crate::{
    api::task::AsletTask,
    backup::BackupRequest,
    worker::{Worker, messages::InputMessage},
};

/// Represents a connection to a SQLite database.
#[derive(GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct AsletConn {
    conn_id: usize,
    path: String,
    worker: Worker,
    tasks: Arc<Slab<Gd<AsletTask>>>,
}

#[godot_api]
impl AsletConn {
    pub fn new(
        conn_id: usize,
        path: String,
        worker: Worker,
        tasks: Arc<Slab<Gd<AsletTask>>>,
    ) -> Gd<Self> {
        Gd::from_object(Self {
            conn_id,
            path,
            worker,
            tasks,
        })
    }

    /// Starts a new database transaction.
    ///
    /// This function requests the worker to begin a new transaction, returning an [`AsletTask`]
    /// that yields the resulting [`AsletTransaction`] object once it’s ready.
    ///
    /// The transaction uses its own dedicated connection, ensuring isolation from concurrent
    /// asynchronous operations on other connections.
    ///
    /// # Returns
    ///
    /// Returns an [`AsletTask`] that yields **once**, producing a [`VariantArray`] with one of the following forms:
    ///
    /// * `[OK, transaction]` — transaction successfully started.
    /// * `[FAILED, errmsg]` — failed to start transaction, with `errmsg` containing the error message.
    ///
    /// # Example
    /// ```gdscript
    /// var result := await db.transaction().done as Array
    /// if result[0] == OK:
    ///     var tx := result[1] as AsletTransaction
    ///     await tx.exec("insert into test values (?1)", [42]).done
    ///     await tx.commit().done
    /// else:
    ///     push_error(result[1])
    /// ```
    #[func]
    fn transaction(&self) -> Gd<AsletTask> {
        let (task_ctx, task) = super::task::create(&self.tasks);
        self.worker.send(InputMessage::BeginTransaction(
            task_ctx,
            self.path.to_string(),
        ));
        task
    }

    /// Executes a batch insert operation with multiple rows.
    ///
    /// This function efficiently inserts multiple records using a single prepared SQL statement.
    /// Each row in `rows` corresponds to a parameter set for the statement.
    ///
    /// # Parameters
    ///
    /// * `sql` — The SQL insert statement, with placeholders (`?1`, `?2`, etc.) for parameters.
    /// * `rows` — An array of arrays, where each inner array represents the parameter values for one row.
    ///
    /// # Returns
    ///
    /// Returns an [`AsletTask`] that yields **once**, producing a [`VariantArray`] with one of the following forms:
    ///
    /// * `[OK]` — batch insert done successfully.
    /// * `[FAILED, errmsg]` — insert failed, with `errmsg` containing the error message.
    ///
    /// # Example
    /// ```gdscript
    /// var rows := []
    /// for i in 1000:
    ///     rows.push_back(["name", i])
    /// var result := await db.batch_insert("insert into test (name, value) values (?1, ?2)", rows).done as Array
    /// if result[0] == FAILED:
    ///     push_error(result[1])
    /// ```
    #[func]
    fn batch_insert(&self, sql: GString, rows: Array<Array<Variant>>) -> Gd<AsletTask> {
        let (task_ctx, task) = super::task::create(&self.tasks);
        self.worker.send(InputMessage::BatchInsert(
            self.conn_id,
            task_ctx,
            sql.into(),
            rows.into(),
        ));
        task
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
    /// var result := await db.exec("delete from test", []).done as Array
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
    /// var result := await db.fetch("select * from test", []).done as Array
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

    /// Starts an incremental database backup.
    ///
    /// This function creates a [`BackupRequest`] with the destination path, number of pages per step,
    /// and a [`Callable`] for progress reporting, then sends it to the worker thread responsible for the backup.
    ///
    /// # Parameters
    ///
    /// * `dst` — Path to the destination database.
    /// * `step` — Number of pages to process per backup step.
    /// * `progress` — A [`Callable`] that will be called periodically to report progress.
    ///   The callable **must** have the following signature:
    ///   ```gdscript
    ///   func(page_count: int, remaining: int)
    ///   ```
    ///   - `page_count` — total pages in the backup.
    ///   - `remaining` — pages still to be copied.
    ///
    /// # Returns
    ///
    /// Returns an [`AsletTask`] representing the backup task.  
    /// The task yields **once**, returning a [`VariantArray`] with one of the following forms:
    ///
    /// * `[OK]` — backup done successfully.
    /// * `[FAILED, errmsg]` — backup failed, with `errmsg` containing the error message.
    #[func]
    fn backup(&self, dst: GString, step: i32, progress: Callable) -> Option<Gd<AsletTask>> {
        let (task_ctx, task) = super::task::create(&self.tasks);

        self.worker.send(InputMessage::BeginBackup(
            task_ctx,
            BackupRequest {
                src: self.path.clone(),
                dst: dst.to_string(),
                step,
                progress,
            },
        ));

        Some(task)
    }
}

impl Drop for AsletConn {
    fn drop(&mut self) {
        self.worker.send(InputMessage::CloseConn(self.conn_id));
    }
}
