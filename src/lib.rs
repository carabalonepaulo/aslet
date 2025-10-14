mod api;
mod backup;
mod error;
mod result;
mod types;
mod worker;

use godot::prelude::*;

struct AsletExt;

#[gdextension]
unsafe impl ExtensionLibrary for AsletExt {}
