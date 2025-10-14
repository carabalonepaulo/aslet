use std::fmt::Display;

const INTERNAL: i64 = 10000;
const RUSQLITE: i64 = 12000;

#[derive(Debug)]
pub enum InternalError {
    InvalidConnection(usize),
    InvalidTransaction,
    TaskCanceled,
}

impl From<&InternalError> for i64 {
    fn from(value: &InternalError) -> Self {
        INTERNAL
            + match value {
                InternalError::InvalidConnection(_) => 1,
                InternalError::InvalidTransaction => 2,
                InternalError::TaskCanceled => 3,
            }
    }
}

impl Display for InternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InternalError::InvalidConnection(conn_id) => {
                write!(f, "invalid connection id: {}", conn_id)
            }
            InternalError::InvalidTransaction => write!(f, "invalid transaction"),
            InternalError::TaskCanceled => write!(f, "task canceled"),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Internal(InternalError),
    Sqlite(rusqlite::Error),
}

impl From<&Error> for i64 {
    fn from(value: &Error) -> Self {
        match value {
            Error::Internal(error) => error.into(),
            Error::Sqlite(error) => match error.sqlite_error_code() {
                Some(code) => code as i64,
                None => {
                    RUSQLITE
                        + match error {
                            rusqlite::Error::SqliteFailure(_, _) => 1,
                            rusqlite::Error::SqliteSingleThreadedMode => 2,
                            rusqlite::Error::FromSqlConversionFailure(_, _, _) => 3,
                            rusqlite::Error::IntegralValueOutOfRange(_, _) => 4,
                            rusqlite::Error::Utf8Error(_) => 5,
                            rusqlite::Error::NulError(_) => 6,
                            rusqlite::Error::InvalidParameterName(_) => 7,
                            rusqlite::Error::InvalidPath(_) => 8,
                            rusqlite::Error::ExecuteReturnedResults => 9,
                            rusqlite::Error::QueryReturnedNoRows => 10,
                            rusqlite::Error::QueryReturnedMoreThanOneRow => 11,
                            rusqlite::Error::InvalidColumnIndex(_) => 12,
                            rusqlite::Error::InvalidColumnName(_) => 13,
                            rusqlite::Error::InvalidColumnType(_, _, _) => 14,
                            rusqlite::Error::StatementChangedRows(_) => 15,
                            rusqlite::Error::ToSqlConversionFailure(_) => 16,
                            rusqlite::Error::InvalidQuery => 17,
                            rusqlite::Error::UnwindingPanic => 18,
                            rusqlite::Error::MultipleStatement => 19,
                            rusqlite::Error::InvalidParameterCount(_, _) => 20,
                            rusqlite::Error::SqlInputError { .. } => 21,
                            rusqlite::Error::InvalidDatabaseIndex(_) => 22,
                            _ => 999,
                        }
                }
            },
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Internal(error) => error.fmt(f),
            Error::Sqlite(error) => error.fmt(f),
        }
    }
}

impl From<InternalError> for Error {
    fn from(value: InternalError) -> Self {
        Self::Internal(value)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}
