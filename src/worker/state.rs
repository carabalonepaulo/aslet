use std::{
    sync::Arc,
    thread::{JoinHandle, spawn},
};

use crossbeam::channel::{Receiver, Sender};
use godot::{global::printerr, meta::ToGodot};

use crate::worker::messages::{InputMessage, OutputMessage};

use super::dispatch::message_loop;

#[derive(Debug)]
struct InnerState {
    handle: Option<JoinHandle<()>>,
    input_sender: Sender<InputMessage>,
}

impl InnerState {
    pub fn new() -> (Self, Receiver<OutputMessage>) {
        let (input_sender, input_receiver) = crossbeam::channel::unbounded::<InputMessage>();
        let (output_sender, output_receiver) = crossbeam::channel::unbounded::<OutputMessage>();
        let handle = Some(spawn(move || message_loop(input_receiver, output_sender)));

        (
            Self {
                handle,
                input_sender,
            },
            output_receiver,
        )
    }
}

impl Drop for InnerState {
    fn drop(&mut self) {
        let _ = self.input_sender.send(InputMessage::Quit);
        match self.handle.take() {
            Some(handle) => {
                if let Err(_) = handle.join() {
                    printerr(&["worker thread panicked".to_variant()]);
                }
            }
            None => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct Worker(Arc<InnerState>);

impl Worker {
    pub fn new() -> (Self, Receiver<OutputMessage>) {
        let (inner_state, receiver) = InnerState::new();
        (Self(Arc::new(inner_state)), receiver)
    }

    pub fn send(&self, msg: InputMessage) {
        if let Err(_) = self.0.input_sender.send(msg) {
            printerr(&["failed to contact worker, might have panicked".to_variant()]);
        }
    }
}
