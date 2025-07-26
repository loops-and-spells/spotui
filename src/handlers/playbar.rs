use super::{
  super::app::{ActiveBlock, App},
  common_key_events,
};
use crate::event::Key;
use crate::network::{IoEvent, PlayingItem};
use rspotify::model::{context::CurrentPlaybackContext, PlayableItem};

pub fn handler(_key: Key, app: &mut App) {
  // PlayBar is no longer keyboard navigable - immediately move focus away
  app.set_current_route_state(Some(ActiveBlock::Empty), Some(ActiveBlock::MyPlaylists));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn on_left_press() {
    let mut app = App::default();
    app.set_current_route_state(Some(ActiveBlock::PlayBar), Some(ActiveBlock::PlayBar));

    handler(Key::Up, &mut app);
    let current_route = app.get_current_route();
    assert_eq!(current_route.active_block, ActiveBlock::Empty);
    assert_eq!(current_route.hovered_block, ActiveBlock::MyPlaylists);
  }
}
