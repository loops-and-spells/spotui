use crate::event::Key;
use crossterm::event;
use std::{sync::{mpsc, Arc, atomic::{AtomicU64, Ordering}}, thread, time::Duration};

#[derive(Debug, Clone, Copy)]
/// Configuration for event handling.
pub struct EventConfig {
  /// The key that is used to exit the application.
  pub exit_key: Key,
  /// The tick rate at which the application will sent an tick event.
  pub tick_rate: Duration,
}

impl Default for EventConfig {
  fn default() -> EventConfig {
    EventConfig {
      exit_key: Key::Ctrl('c'),
      tick_rate: Duration::from_millis(250),
    }
  }
}

/// An occurred event.
pub enum Event<I> {
  /// An input event occurred.
  Input(I),
  /// An tick event occurred.
  Tick,
}

/// A small event handler that wrap crossterm input and tick event. Each event
/// type is handled in its own thread and returned to a common `Receiver`
pub struct Events {
  rx: mpsc::Receiver<Event<Key>>,
  // Need to be kept around to prevent disposing the sender side.
  _tx: mpsc::Sender<Event<Key>>,
  // Shared tick rate that can be updated dynamically
  tick_rate_ms: Arc<AtomicU64>,
}

impl Events {
  /// Constructs an new instance of `Events` with the default config.
  pub fn new(tick_rate: u64) -> Events {
    Events::with_config(EventConfig {
      tick_rate: Duration::from_millis(tick_rate),
      ..Default::default()
    })
  }

  /// Constructs an new instance of `Events` from given config.
  pub fn with_config(config: EventConfig) -> Events {
    let (tx, rx) = mpsc::channel();
    let tick_rate_ms = Arc::new(AtomicU64::new(config.tick_rate.as_millis() as u64));
    let tick_rate_ms_clone = Arc::clone(&tick_rate_ms);

    let event_tx = tx.clone();
    thread::spawn(move || {
      loop {
        // Get current tick rate
        let current_tick_rate = Duration::from_millis(tick_rate_ms_clone.load(Ordering::Relaxed));
        
        // poll for tick rate duration, if no event, sent tick event.
        if event::poll(current_tick_rate).unwrap() {
          if let event::Event::Key(key) = event::read().unwrap() {
            let key = Key::from(key);

            event_tx.send(Event::Input(key)).unwrap();
          }
        }

        event_tx.send(Event::Tick).unwrap();
      }
    });

    Events { rx, _tx: tx, tick_rate_ms }
  }

  /// Attempts to read an event.
  /// This function will block the current thread.
  pub fn next(&self) -> Result<Event<Key>, mpsc::RecvError> {
    self.rx.recv()
  }
  
  /// Update the tick rate dynamically
  pub fn set_tick_rate(&self, tick_rate_ms: u64) {
    self.tick_rate_ms.store(tick_rate_ms, Ordering::Relaxed);
  }
}
