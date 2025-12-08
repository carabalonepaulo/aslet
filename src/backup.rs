use godot::{builtin::Callable, classes::ProjectSettings, meta::ToGodot};
use rusqlite::{
    Connection,
    backup::{Backup, StepResult},
};

/// Contains all information required to initialize a [`BackupState`] on a worker thread.
///
/// This struct acts as a setup container for starting a database backup process.
/// It includes the source and destination database paths, the number of pages
/// to copy per step, and a [`Callable`] for progress reporting.
///
/// A `BackupRequest` is always constructed on the main thread and sent to a worker
/// thread. The worker then consumes it to build a [`BackupState`].
///
/// # Thread Safety
///
/// `BackupRequest` implements `Send` unsafely because it holds a [`Callable`],
/// which is not `Send` by default.  
/// This is sound under these constraints:
///
/// * The struct is always **moved** between threads — never shared or referenced concurrently.
/// * The contained [`Callable`] is not invoked or modified across threads simultaneously.
/// * Once transferred, the main thread relinquishes ownership completely.
///
/// In this model, `BackupRequest` acts as an immutable setup payload safely
/// passed to the worker thread.
///
/// # Fields
///
/// * `src` — Path to the source database file.
/// * `dst` — Path to the destination database file.
/// * `step` — Number of pages processed per backup step.
/// * `progress` — A [`Callable`] used to report progress updates back to the main thread.
///
pub struct BackupRequest {
    pub src: String,
    pub dst: String,
    pub step: i32,
    pub progress: Callable,
}

unsafe impl Send for BackupRequest {}

/// Manages an incremental SQLite database backup process across thread boundaries.
///
/// This struct holds the source and destination database connections, and the `rusqlite::backup::Backup`
/// object, allowing for a stepped backup process. The core challenge addressed here is safely
/// moving this state (which includes `rusqlite::Connection`s and a self-referential-like `Backup` object)
/// between threads by leveraging `unsafe` blocks and explicit ownership transfer.
///
/// # Safety
///
/// This struct implements `Send` using an `unsafe` block. This is safe under the following critical invariants:
///
/// 1.  **Exclusive Ownership and Thread Safety:** An instance of `BackupState` is always moved
///     (owned) between threads and never shared via references or accessed concurrently.
///     The `rusqlite::Connection` type is not `Send` or `Sync` by default because `sqlite3` handles
///     are not thread-safe for concurrent access. However, by moving `BackupState` by value,
///     ownership of the `Connection`s (and the `Backup` object that references them) is transferred
///     exclusively to the receiving thread, ensuring no concurrent access to the `sqlite3` handles.
///
/// 2.  **Lifetime Guarantees for `Backup` Object:** The `rusqlite::backup::Backup` object holds
///     mutable references (`&'a Connection`, `&'b mut Connection`) to the `src` and `dst`
///     connections. To store `Backup` alongside its referenced `Connection`s within the same struct,
///     a `std::mem::transmute` is used to cast the `Backup`'s internal references to `'static`
///     lifetimes. This "cheats" the Rust borrow checker.
///
///     This is safe because:
///     *   The `BackupState` struct *owns* `src`, `dst`, and `backup`, ensuring they are
///         co-located and dropped together.
///     *   Rust's drop order guarantees that fields are dropped in declaration order.
///         In `BackupState`, `backup` is declared *before* `src` and `dst`.
///         This ensures that `backup` is dropped *before* `src` and `dst`.
///     *   Crucially, `rusqlite::backup::Backup::drop` performs a significant operation:
///         it calls `sqlite3_backup_finish`. During this `drop` operation, the `backup` object
///         requires its internal references to `src` and `dst` to still be valid.
///     *   Since `src` and `dst` are only dropped *after* `backup` has been dropped, their
///         validity is guaranteed during the entire lifecycle of `backup`, including its `drop`
///         implementation.
///
/// 3.  **Heap allocation via `Box<Connection>`:** The `Connection`s are placed on the heap
///     using `Box` to provide stable memory addresses. This ensures that the references
///     held internally by the `Backup` object remain valid for its entire lifetime.
///
#[allow(unused)]
pub struct BackupState {
    backup: Backup<'static, 'static>,
    src: Box<Connection>,
    dst: Box<Connection>,
    step: i32,
    done: bool,
    progress: Callable,
}

unsafe impl Send for BackupState {}

impl BackupState {
    pub fn new(state: BackupRequest) -> Result<Self, rusqlite::Error> {
        let src_path = ProjectSettings::singleton()
            .globalize_path(&state.src)
            .to_string();
        let dst_path = ProjectSettings::singleton()
            .globalize_path(&state.dst)
            .to_string();

        let src_conn = Box::new(Connection::open(&src_path)?);
        let mut dst_conn = Box::new(Connection::open(&dst_path)?);
        let backup = unsafe {
            let src_ref: *const Connection = &*src_conn;
            let dst_ref: *mut Connection = &mut *dst_conn;
            let backup = Backup::new(&*src_ref, &mut *dst_ref)?;
            std::mem::transmute::<Backup<'_, '_>, Backup<'static, 'static>>(backup)
        };

        Ok(Self {
            src: src_conn,
            dst: dst_conn,
            backup,
            step: state.step,
            done: false,
            progress: state.progress,
        })
    }

    pub fn worker<'a>(&'a mut self) -> Worker<'a> {
        Worker(self)
    }

    pub fn main<'a>(&'a self) -> Main<'a> {
        Main(self)
    }
}

/// Provides the worker-side API for performing incremental backup steps.
///
/// A [`Worker`] instance is created from a mutable reference to a [`BackupState`]
/// by calling [`BackupState::worker`]. It holds a mutable reference of the state,
/// ensuring that only the worker can mutate the backup progress while the borrow exists.
///
pub struct Worker<'a>(&'a mut BackupState);

impl<'a> Worker<'a> {
    pub fn step(&mut self) -> Result<bool, rusqlite::Error> {
        if let StepResult::Done = self.0.backup.step(self.0.step)? {
            self.0.done = true;
        }
        Ok(self.0.done)
    }
}

/// Provides the main-thread API for monitoring backup progress.
///
/// A [`Main`] view is obtained by calling [`BackupState::main`].
/// It borrows the state immutably, giving read-only access to
/// completion status and progress metrics.
///
pub struct Main<'a>(&'a BackupState);

impl<'a> Main<'a> {
    pub fn tick(&self) {
        let progress = self.0.backup.progress();
        self.0.progress.call(&[
            progress.pagecount.to_variant(),
            progress.remaining.to_variant(),
        ]);
    }

    pub fn is_done(&self) -> bool {
        self.0.done
    }
}
