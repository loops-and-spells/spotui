use crate::event::Key;
use crossterm::event;
use std::{
    sync::{
        mpsc::{self, TryRecvError},
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

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
  /// Terminal was resized
  Resize(u16, u16),
}

/// A small event handler that wrap crossterm input and tick event. Each event
/// type is handled in its own thread and returned to a common `Receiver`
pub struct Events {
  rx: mpsc::Receiver<Event<Key>>,
  // Need to be kept around to prevent disposing the sender side.
  _input_tx: mpsc::Sender<Event<Key>>,
  _tick_tx: mpsc::Sender<Event<Key>>,
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
    
    // Clone for input thread
    let input_tx = tx.clone();
    let _input_tx_handle = input_tx.clone();
    
    // Clone for tick thread
    let tick_tx = tx.clone();
    let _tick_tx_handle = tick_tx.clone();
    let tick_rate_ms_clone = Arc::clone(&tick_rate_ms);

    // Spawn dedicated input thread - polls frequently for immediate response
    thread::spawn(move || {
      loop {
        // Poll with very short timeout (1ms) for instant input response
        match event::poll(Duration::from_millis(1)) {
          Ok(true) => {
            match event::read() {
              Ok(event::Event::Key(key)) => {
                let key = Key::from(key);
                if input_tx.send(Event::Input(key)).is_err() {
                  break; // Channel closed, exit thread
                }
              }
              Ok(event::Event::Resize(width, height)) => {
                if input_tx.send(Event::Resize(width, height)).is_err() {
                  break; // Channel closed, exit thread
                }
              }
              Ok(_) => {} // Ignore other events like mouse
              Err(_) => {
                // Error reading event, continue to next iteration
                // This prevents the thread from crashing on resize errors
                continue;
              }
            }
          }
          Ok(false) => {} // No event available
          Err(_) => {
            // Error polling, sleep briefly and continue
            thread::sleep(Duration::from_millis(10));
          }
        }
      }
    });

    // Spawn dedicated tick thread - sends tick events at configured rate
    thread::spawn(move || {
      let mut last_tick = Instant::now();
      
      loop {
        // Get current tick rate
        let current_tick_rate = Duration::from_millis(tick_rate_ms_clone.load(Ordering::Relaxed));
        
        // Sleep until next tick
        let elapsed = last_tick.elapsed();
        if elapsed < current_tick_rate {
          thread::sleep(current_tick_rate - elapsed);
        }
        
        // Send tick event
        if tick_tx.send(Event::Tick).is_err() {
          break; // Channel closed, exit thread
        }
        
        last_tick = Instant::now();
      }
    });

    Events { 
      rx, 
      _input_tx: _input_tx_handle,
      _tick_tx: _tick_tx_handle,
      tick_rate_ms 
    }
  }

  /// Attempts to read an event.
  /// This function will block the current thread.
  pub fn next(&self) -> Result<Event<Key>, mpsc::RecvError> {
    self.rx.recv()
  }
  
  /// Try to read an event without blocking
  pub fn try_next(&self) -> Result<Event<Key>, TryRecvError> {
    self.rx.try_recv()
  }
  
  /// Update the tick rate dynamically
  pub fn set_tick_rate(&self, tick_rate_ms: u64) {
    self.tick_rate_ms.store(tick_rate_ms, Ordering::Relaxed);
  }
}