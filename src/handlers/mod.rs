mod album_list;
mod album_tracks;
mod analysis;
mod artist;
mod artists;
mod basic_view;
mod common_key_events;
mod dialog;
mod empty;
mod episode_table;
mod home;
mod input;
mod library;
mod log_stream;
mod playbar;
mod playlist;
mod podcasts;
mod recently_played;
mod search_results;
mod select_device;
mod track_table;

use super::app::{ActiveBlock, App, ArtistBlock, RouteId, SearchResultBlock};
use crate::event::Key;
use crate::focus_manager::ComponentId;
use crate::network::IoEvent;
use rspotify::model::{context::CurrentPlaybackContext, PlayableItem};
use crate::network::PlayingItem;
use std::time::Instant;

pub use input::handler as input_handler;

pub fn handle_app(key: Key, app: &mut App) {
  // First handle any global event and then move to block event
  match key {
    Key::Esc => {
      handle_escape(app);
    }
    // Component entry shortcuts - enter components directly for internal navigation
    Key::Char('L') | Key::Char('l') => {
      app.clear_navigation_stack();
      app.enter_component(ComponentId::Library);
      app.set_current_route_state(Some(ActiveBlock::Library), Some(ActiveBlock::Library));
    }
    Key::Char('P') | Key::Char('p') => {
      app.clear_navigation_stack();
      app.enter_component(ComponentId::MyPlaylists);
      app.set_current_route_state(Some(ActiveBlock::MyPlaylists), Some(ActiveBlock::MyPlaylists));
    }
    Key::Char('S') | Key::Char('s') => {
      app.clear_navigation_stack();
      app.enter_component(ComponentId::SearchInput);
      app.set_current_route_state(Some(ActiveBlock::Input), Some(ActiveBlock::Input));
      // Clear any existing search results focus to avoid dual focus
      app.search_results.selected_block = SearchResultBlock::Empty;
      app.search_results.hovered_block = SearchResultBlock::Empty;
    }
    Key::Char('D') => {
      app.set_current_route_state(Some(ActiveBlock::SelectDevice), Some(ActiveBlock::SelectDevice));
    }
    Key::Char('O') => {
      app.push_navigation_stack(RouteId::LogStream, ActiveBlock::LogStream);
    }
    Key::Ctrl('l') => {
      app.push_navigation_stack(RouteId::LogStream, ActiveBlock::LogStream);
    }
    Key::Char('F') | Key::Char('f') => {
      // Toggle fullscreen/idle mode
      app.is_idle_mode = !app.is_idle_mode;
      // Reset idle timer to prevent automatic idle mode from interfering
      app.last_user_interaction = Instant::now();
      
      if app.is_idle_mode {
        // Fetch larger album art for idle mode
        if let Some(url) = &app.current_album_art_url {
          app.dispatch(IoEvent::FetchAlbumArt(url.clone()));
        }
        app.add_log_message("Entered fullscreen album art mode".to_string());
      } else {
        // Fetch smaller album art for normal mode
        if let Some(url) = &app.current_album_art_url {
          app.dispatch(IoEvent::FetchAlbumArt(url.clone()));
        }
        app.add_log_message("Exited fullscreen album art mode".to_string());
      }
    }
    _ if key == app.user_config.keys.jump_to_album => {
      handle_jump_to_album(app);
    }
    _ if key == app.user_config.keys.jump_to_artist_album => {
      handle_jump_to_artist_album(app);
    }
    _ if key == app.user_config.keys.jump_to_context => {
      handle_jump_to_context(app);
    }
    _ if key == app.user_config.keys.manage_devices => {
      app.add_log_message("Opening device selection screen...".to_string());
      // Navigate to device screen immediately, like 'l' does for log stream
      app.push_navigation_stack(RouteId::SelectedDevice, ActiveBlock::SelectDevice);
      // Then fetch the devices
      app.dispatch(IoEvent::GetDevices);
    }
    _ if key == app.user_config.keys.decrease_volume => {
      app.decrease_volume();
    }
    _ if key == app.user_config.keys.increase_volume => {
      app.increase_volume();
    }
    // Press space to toggle playback
    _ if key == app.user_config.keys.toggle_playback => {
      app.toggle_playback();
    }
    _ if key == app.user_config.keys.seek_backwards => {
      app.seek_backwards();
    }
    _ if key == app.user_config.keys.seek_forwards => {
      app.seek_forwards();
    }
    _ if key == app.user_config.keys.next_track => {
      app.dispatch(IoEvent::NextTrack);
    }
    _ if key == app.user_config.keys.previous_track => {
      app.previous_track();
    }

    _ if key == app.user_config.keys.shuffle => {
      app.shuffle();
    }
    _ if key == app.user_config.keys.repeat => {
      app.repeat();
    }
    _ if key == app.user_config.keys.search => {
      app.set_current_route_state(Some(ActiveBlock::Input), Some(ActiveBlock::Input));
      // Clear any existing search results focus to avoid dual focus
      app.search_results.selected_block = SearchResultBlock::Empty;
      app.search_results.hovered_block = SearchResultBlock::Empty;
    }
    _ if key == app.user_config.keys.copy_song_url => {
      app.copy_song_url();
    }
    _ if key == app.user_config.keys.copy_album_url => {
      app.copy_album_url();
    }
    _ if key == app.user_config.keys.audio_analysis => {
      app.get_audio_analysis();
    }
    _ if key == app.user_config.keys.basic_view => {
      app.push_navigation_stack(RouteId::BasicView, ActiveBlock::BasicView);
    }
    _ => handle_block_events(key, app),
  }
}

// Handle event for the current active block
fn handle_block_events(key: Key, app: &mut App) {
  let current_route = app.get_current_route();
  match current_route.active_block {
    ActiveBlock::Analysis => {
      analysis::handler(key, app);
    }
    ActiveBlock::ArtistBlock => {
      artist::handler(key, app);
    }
    ActiveBlock::Input => {
      input::handler(key, app);
    }
    ActiveBlock::MyPlaylists => {
      playlist::handler(key, app);
    }
    ActiveBlock::TrackTable => {
      track_table::handler(key, app);
    }
    ActiveBlock::EpisodeTable => {
      episode_table::handler(key, app);
    }
    ActiveBlock::SelectDevice => {
      select_device::handler(key, app);
    }
    ActiveBlock::SearchResultBlock => {
      search_results::handler(key, app);
    }
    ActiveBlock::Home => {
      home::handler(key, app);
    }
    ActiveBlock::AlbumList => {
      album_list::handler(key, app);
    }
    ActiveBlock::AlbumTracks => {
      album_tracks::handler(key, app);
    }
    ActiveBlock::Library => {
      library::handler(key, app);
    }
    ActiveBlock::Empty => {
      empty::handler(key, app);
    }
    ActiveBlock::RecentlyPlayed => {
      recently_played::handler(key, app);
    }
    ActiveBlock::Artists => {
      artists::handler(key, app);
    }
    ActiveBlock::Podcasts => {
      podcasts::handler(key, app);
    }
    ActiveBlock::PlayBar => {
      playbar::handler(key, app);
    }
    ActiveBlock::BasicView => {
      basic_view::handler(key, app);
    }
    ActiveBlock::LogStream => {
      log_stream::handler(key, app);
    }
    ActiveBlock::Error => {
      // Error screen no longer exists, do nothing
    }
    ActiveBlock::Dialog(_) => {
      dialog::handler(key, app);
    }
  }
}

fn handle_escape(app: &mut App) {
  match app.get_current_route().active_block {
    ActiveBlock::SearchResultBlock => {
      // Clear search results focus using centralized system
      app.clear_all_focus();
      app.search_results.selected_block = SearchResultBlock::Empty;
      app.search_results.hovered_block = SearchResultBlock::Empty;
    }
    ActiveBlock::ArtistBlock => {
      if let Some(artist) = &mut app.artist {
        artist.artist_selected_block = ArtistBlock::Empty;
      }
      app.clear_focus();
    }
    ActiveBlock::Dialog(_) => {
      app.pop_navigation_stack();
    }
    // These are global views that have no active/inactive distinction
    ActiveBlock::SelectDevice => {
      app.pop_navigation_stack();
    }
    ActiveBlock::Analysis => {
      app.clear_focus();
    }
    ActiveBlock::LogStream => {
      app.pop_navigation_stack();
    }
    _ => {
      app.clear_all_focus();
      app.set_current_route_state(Some(ActiveBlock::Empty), None);
    }
  }
}

fn handle_jump_to_context(app: &mut App) {
  if let Some(current_playback_context) = &app.current_playback_context {
    // TODO: Fix senum::Type for rspotify 0.15
    // if let Some(play_context) = current_playback_context.context.clone() {
    //   match play_context._type {
    //     rspotify::senum::Type::Album => handle_jump_to_album(app),
    //     rspotify::senum::Type::Artist => handle_jump_to_artist_album(app),
    //     rspotify::senum::Type::Playlist => {
    //       app.dispatch(IoEvent::GetPlaylistTracks(format!("spotify:track:{}", play_context.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string())), 0))
    //     }
    //     _ => {}
    //   }
    // }
  }
}

fn handle_jump_to_album(app: &mut App) {
  if let Some(CurrentPlaybackContext {
    item: Some(item), ..
  }) = app.current_playback_context.to_owned()
  {
    match item {
      PlayableItem::Track(track) => {
        app.dispatch(IoEvent::GetAlbumTracks(track.album.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string())));
      }
      PlayableItem::Episode(_episode) => {
        // Note: episode.show field not available in newer API
        // app.dispatch(IoEvent::GetShowEpisodes(Box::new(episode.show)));
      }
    };
  }
}

// NOTE: this only finds the first artist of the song and jumps to their albums
fn handle_jump_to_artist_album(app: &mut App) {
  if let Some(CurrentPlaybackContext {
    item: Some(item), ..
  }) = app.current_playback_context.to_owned()
  {
    match item {
      PlayableItem::Track(track) => {
        if let Some(artist) = track.artists.first() {
          let artist_id = artist.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string());
          if !artist_id.is_empty() {
            app.get_artist(artist_id, artist.name.clone());
            app.push_navigation_stack(RouteId::Artist, ActiveBlock::ArtistBlock);
          }
        }
      }
      PlayableItem::Episode(_episode) => {
        // Do nothing for episode (yet!)
      }
    }
  };
}
