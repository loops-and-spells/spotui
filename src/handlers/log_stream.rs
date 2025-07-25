use super::{
  super::app::App,
  common_key_events,
};
use crate::event::Key;

pub fn handler(key: Key, app: &mut App) {
  match key {
    Key::Esc => {
      app.pop_navigation_stack();
    }
    k if common_key_events::down_event(k) => {
      if !app.log_messages.is_empty() {
        let new_index = if app.log_stream_selected_index < app.log_messages.len() - 1 {
          app.log_stream_selected_index + 1
        } else {
          app.log_stream_selected_index
        };
        app.log_stream_selected_index = new_index;
        
        // Update scroll offset to keep selection visible
        update_scroll_offset(app);
      }
    }
    k if common_key_events::up_event(k) => {
      if !app.log_messages.is_empty() && app.log_stream_selected_index > 0 {
        app.log_stream_selected_index -= 1;
        
        // Update scroll offset to keep selection visible
        update_scroll_offset(app);
      }
    }
    k if common_key_events::high_event(k) => {
      // Jump to top
      if !app.log_messages.is_empty() {
        app.log_stream_selected_index = 0;
        app.log_stream_scroll_offset = 0;
      }
    }
    k if common_key_events::low_event(k) => {
      // Jump to bottom
      if !app.log_messages.is_empty() {
        app.log_stream_selected_index = app.log_messages.len() - 1;
        update_scroll_offset(app);
      }
    }
    Key::PageUp => {
      // Page up
      if !app.log_messages.is_empty() {
        let page_size = 10; // Adjust based on visible height if needed
        app.log_stream_selected_index = app.log_stream_selected_index.saturating_sub(page_size);
        update_scroll_offset(app);
      }
    }
    Key::PageDown => {
      // Page down
      if !app.log_messages.is_empty() {
        let page_size = 10; // Adjust based on visible height if needed
        let max_index = app.log_messages.len() - 1;
        app.log_stream_selected_index = std::cmp::min(
          app.log_stream_selected_index + page_size,
          max_index
        );
        update_scroll_offset(app);
      }
    }
    _ => {}
  }
}

fn update_scroll_offset(app: &mut App) {
  // Assume a reasonable visible height for now (could be passed as parameter)
  let visible_height = 10; // This should ideally be calculated from layout
  
  if app.log_messages.is_empty() {
    return;
  }
  
  let selected = app.log_stream_selected_index;
  let scroll_offset = app.log_stream_scroll_offset;
  
  // If selection is above visible area, scroll up
  if selected < scroll_offset {
    app.log_stream_scroll_offset = selected;
  }
  // If selection is below visible area, scroll down
  else if selected >= scroll_offset + visible_height {
    app.log_stream_scroll_offset = selected.saturating_sub(visible_height - 1);
  }
  
  // Ensure scroll offset doesn't exceed bounds
  let max_scroll = app.log_messages.len().saturating_sub(visible_height);
  if app.log_stream_scroll_offset > max_scroll {
    app.log_stream_scroll_offset = max_scroll;
  }
}