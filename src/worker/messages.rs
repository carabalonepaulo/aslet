use crate::{
    api::{task::TaskContext, transaction::TransactionState},
    backup::{BackupRequest, BackupState},
    error::Error,
    types::{Columns, Row, Rows},
};

pub enum InputMessage {
    Open(TaskContext, String),
    BatchInsert(usize, TaskContext, String, Rows),
    Exec(usize, TaskContext, String, Row),
    Fetch(usize, TaskContext, String, Row),

    BeginTransaction(TaskContext, String),
    Rollback(TaskContext, usize, TransactionState),
    Commit(TaskContext, usize, TransactionState),
    BeginBackup(TaskContext, BackupRequest),
    BackupStep(TaskContext, BackupState),

    CloseConn(usize),
    Quit,
}

pub enum OutputMessage {
    Open(TaskContext, Result<(usize, String), Error>),
    Exec(TaskContext, Result<i64, Error>),
    Fetch(TaskContext, Result<(Rows, Columns), Error>),
    TransactionStarted(TaskContext, Result<usize, Error>),
    TransactionCommitted(TaskContext, Result<(), Error>),
    TransactionRolledBack(TaskContext, Result<(), Error>),
    Backup(TaskContext, Result<BackupState, Error>),
    Canceled(TaskContext),
}
