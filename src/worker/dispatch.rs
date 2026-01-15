use crossbeam::channel::{Receiver, Sender};
use godot::{classes::ProjectSettings, global::printerr, meta::ToGodot, obj::Singleton};
use rusqlite::{Connection, ToSql};
use slab::Slab;

use crate::{
    api::transaction::TransactionState,
    backup::BackupState,
    error::{Error, InternalError},
    types::{Columns, Row, Rows},
    worker::messages::{InputMessage, OutputMessage},
};

pub fn message_loop(input_receiver: Receiver<InputMessage>, output_sender: Sender<OutputMessage>) {
    let mut conn_pool: Slab<Connection> = Slab::new();

    macro_rules! handle {
        ($task_ctx:ident, $task:expr, $output:expr) => {{
            if $task_ctx.is_canceled() {
                let _ = output_sender.send(OutputMessage::Canceled($task_ctx));
                continue;
            }

            let result = $task.map_err(Error::from);
            $task_ctx.done();
            if let Err(_) = output_sender.send($output($task_ctx, result)) {
                printerr(&["aslet instance was dropped prematurely".to_variant()]);
            }
        }};
    }

    for msg in input_receiver {
        match msg {
            InputMessage::Open(task_ctx, path) => {
                handle!(task_ctx, open(&mut conn_pool, path), OutputMessage::Open);
            }
            InputMessage::BeginTransaction(ctx, path) => {
                handle!(
                    ctx,
                    begin_transaction(&mut conn_pool, path),
                    OutputMessage::TransactionStarted
                );
            }
            InputMessage::Rollback(ctx, conn_id, state) => {
                handle!(
                    ctx,
                    rollback(&mut conn_pool, conn_id, state),
                    OutputMessage::TransactionRolledBack
                );
            }
            InputMessage::Commit(ctx, conn_id, state) => {
                handle!(
                    ctx,
                    commit(&mut conn_pool, conn_id, state),
                    OutputMessage::TransactionCommitted
                );
            }
            InputMessage::BatchInsert(conn_id, ctx, query, rows) => {
                handle!(
                    ctx,
                    batch_insert(&mut conn_pool, conn_id, rows, query),
                    OutputMessage::Exec
                );
            }
            InputMessage::Exec(conn_id, ctx, query, params) => {
                handle!(
                    ctx,
                    exec(&conn_pool, conn_id, params, query),
                    OutputMessage::Exec
                );
            }
            InputMessage::Fetch(conn_id, ctx, query, params) => {
                handle!(
                    ctx,
                    fetch(&conn_pool, conn_id, params, query),
                    OutputMessage::Fetch
                );
            }
            InputMessage::BeginBackup(ctx, request) => {
                handle!(
                    ctx,
                    BackupState::new(request).map_err(Error::from),
                    OutputMessage::Backup
                );
            }
            InputMessage::BackupStep(ctx, mut backup) => {
                handle!(
                    ctx,
                    backup.worker().step().map(|_| backup).map_err(Error::from),
                    OutputMessage::Backup
                );
            }
            InputMessage::CloseConn(conn_id) => {
                if let None = conn_pool.try_remove(conn_id) {
                    let err_msg = format!("can't close connection, invalid id {}", conn_id);
                    printerr(&[err_msg.to_variant()]);
                }
            }
            InputMessage::Quit => break,
        }
    }
}

fn get_conn(conn_pool: &Slab<Connection>, conn_id: usize) -> Result<&Connection, Error> {
    conn_pool
        .get(conn_id)
        .ok_or_else(|| InternalError::InvalidConnection(conn_id).into())
}

fn get_conn_mut(
    conn_pool: &mut Slab<Connection>,
    conn_id: usize,
) -> Result<&mut Connection, Error> {
    conn_pool
        .get_mut(conn_id)
        .ok_or_else(|| InternalError::InvalidConnection(conn_id).into())
}

fn open(conn_pool: &mut Slab<Connection>, path: String) -> Result<(usize, String), Error> {
    let real_path = ProjectSettings::singleton()
        .globalize_path(&path)
        .to_string();
    let conn = Connection::open(real_path)?;
    let conn_id = conn_pool.insert(conn);
    Ok((conn_id, path))
}

fn begin_transaction(conn_pool: &mut Slab<Connection>, path: String) -> Result<usize, Error> {
    let real_path = ProjectSettings::singleton()
        .globalize_path(&path)
        .to_string();

    let conn = Connection::open(real_path)?;
    conn.execute("BEGIN TRANSACTION;", [])?;
    let conn_id = conn_pool.insert(conn);
    Ok(conn_id)
}

fn batch_insert(
    conn_pool: &mut Slab<Connection>,
    conn_id: usize,
    rows: Rows,
    query: String,
) -> Result<i64, Error> {
    let conn = get_conn_mut(conn_pool, conn_id)?;
    let tx = conn.transaction()?;
    let mut affected = 0;

    {
        let mut stmt = tx.prepare(&query)?;
        for row in rows.as_ref().iter() {
            let params: Vec<&dyn ToSql> = row.as_ref().iter().map(|v| v as &dyn ToSql).collect();
            let n = stmt.execute(params.as_slice())?;
            affected += n;
        }
    };

    tx.commit()?;
    Ok(affected as i64)
}

fn exec(
    conn_pool: &Slab<Connection>,
    conn_id: usize,
    params: Row,
    query: String,
) -> Result<i64, Error> {
    let conn = get_conn(conn_pool, conn_id)?;
    let mut stmt = conn.prepare_cached(&query)?;
    let params: Vec<&dyn ToSql> = params.as_ref().iter().map(|v| v as &dyn ToSql).collect();
    Ok(stmt.execute(params.as_slice()).map(|v| v as i64)?)
}

fn fetch(
    conn_pool: &Slab<Connection>,
    conn_id: usize,
    params: Row,
    query: String,
) -> Result<(Rows, Columns), Error> {
    let params: Vec<&dyn ToSql> = params.as_ref().iter().map(|v| v as &dyn ToSql).collect();
    let conn = get_conn(conn_pool, conn_id)?;
    let mut stmt = conn.prepare_cached(&query)?;

    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let rows = stmt.query_map(params.as_slice(), |row| Ok(Row::from(row)))?;
    let result: Result<Rows, _> = rows.collect::<rusqlite::Result<Vec<Row>>>().map(Into::into);

    Ok(result.map(|v| (v, columns.into()))?)
}

fn rollback(
    conn_pool: &mut Slab<Connection>,
    conn_id: usize,
    state: TransactionState,
) -> Result<(), Error> {
    if !state.is_active() {
        return Err(InternalError::InvalidTransaction.into());
    }

    let conn = get_conn(conn_pool, conn_id)?;
    conn.execute("ROLLBACK;", []).map_err(Error::from)?;
    state.rollback();
    conn_pool.remove(conn_id);
    Ok(())
}

fn commit(
    conn_pool: &mut Slab<Connection>,
    conn_id: usize,
    state: TransactionState,
) -> Result<(), Error> {
    if !state.is_active() {
        return Err(InternalError::InvalidTransaction.into());
    }

    let conn = get_conn(conn_pool, conn_id)?;
    conn.execute("COMMIT;", []).map_err(Error::from)?;
    state.commit();
    conn_pool.remove(conn_id);
    Ok(())
}
