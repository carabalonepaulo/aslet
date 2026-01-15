use std::{cell::RefCell, rc::Rc};

use godot::prelude::*;

use crate::{
    api::aslet::Aslet,
    error::{Error, InternalError},
    failed,
    tasks::TaskContext,
};

/// Represents an asynchronous operation in progress.
///
/// An `AsletTask` can be canceled before it completes, and will emit
/// the [`done`] signal once finished.
#[derive(GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct AsletTask {
    aslet: Gd<Aslet>,
    ctx: TaskContext,
    base: Base<RefCounted>,
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
    pub fn new(aslet: Gd<Aslet>, ctx: TaskContext) -> Gd<Self> {
        Gd::from_init_fn(|base| Self { aslet, ctx, base })
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

    /// Waits synchronously for the task to complete.
    ///
    /// **WARNING**: This method is primarily intended for specific, non-critical initialization
    /// or migration scenarios where synchronous behavior is unavoidable, and where blocking
    /// the calling thread is acceptable or desired.
    ///
    /// This function blocks the current thread until the underlying asynchronous
    /// operation associated with this [`AsletTask`] finishes execution.  
    /// When the task emits the `"done"` signal, the function returns the result
    /// provided by that signal as a [`VariantArray`].
    ///
    /// For general use in Godot, especially within the game loop or UI interactions,
    /// it is strongly recommended to use Godot's `await task.done` mechanism
    /// to leverage the asynchronous nature of this library and maintain UI responsiveness.
    ///
    /// # Returns
    ///
    /// Returns a [`VariantArray`] representing the result emitted by the task:
    ///
    /// * `[OK, ...]` — task completed successfully. The remaining elements depend on the specific operation.  
    /// * `[FAILED, code, errmsg]` — task failed. `code` is an `int` representing the error type, and `errmsg` is a `String` describing the error.
    ///
    /// # Notes
    ///
    /// * This method **blocks** the calling thread until the task finishes.  
    ///   It should **not** be called from the main thread if blocking would
    ///   interfere with the game loop or the Godot editor.
    /// * Using `wait()` can lead to unresponsive applications if the task takes a long time to complete.
    ///
    /// # Example
    /// ```gdscript
    /// var result := task.wait()
    /// if result[0] == FAILED:
    ///     push_error(result[1])
    /// else:
    ///     print("Task finished:", result)
    /// ```
    #[func]
    pub fn wait(&mut self) -> VarArray {
        let result: Rc<RefCell<Option<VarArray>>> = Rc::new(RefCell::new(None));
        let callback = Callable::from_fn("wait_callback", {
            let result = result.clone();
            move |args| *result.borrow_mut() = Some(args[0].to())
        });

        self.base_mut().connect("done", &callback);
        {
            let aslet = self.aslet.bind();
            while result.borrow().is_none() {
                aslet.poll(1)
            }
        }
        self.base_mut().disconnect("done", &callback);

        result
            .borrow_mut()
            .take()
            .unwrap_or_else(|| failed!(Error::Internal(InternalError::Unreachable)))
    }
}
