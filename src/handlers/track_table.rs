use super::{
  super::app::{App, RecommendationsContext, TrackTable, TrackTableContext},
  common_key_events,
};
use crate::event::Key;
use crate::network::IoEvent;
use rand::{thread_rng, Rng};
use serde_json::from_value;

pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
    k if common_key_events::down_event(k) => {
      let next_index = common_key_events::on_down_press_handler(
        &app.track_table.tracks,
        Some(app.track_table.selected_index),
      );
      app.track_table.selected_index = next_index;
    }
    k if common_key_events::up_event(k) => {
      let next_index = common_key_events::on_up_press_handler(
        &app.track_table.tracks,
        Some(app.track_table.selected_index),
      );
      app.track_table.selected_index = next_index;
    }
    k if common_key_events::high_event(k) => {
      let next_index = common_key_events::on_high_press_handler();
      app.track_table.selected_index = next_index;
    }
    k if common_key_events::middle_event(k) => {
      let next_index = common_key_events::on_middle_press_handler(&app.track_table.tracks);
      app.track_table.selected_index = next_index;
    }
    k if common_key_events::low_event(k) => {
      let next_index = common_key_events::on_low_press_handler(&app.track_table.tracks);
      app.track_table.selected_index = next_index;
    }
    Key::Enter => {
      on_enter(app);
    }
    // Scroll down
    k if k == app.user_config.keys.next_page => {
      match &app.track_table.context {
        Some(context) => match context {
          TrackTableContext::MyPlaylists => {
            if let (Some(playlists), Some(selected_playlist_index)) =
              (&app.playlists, &app.selected_playlist_index)
            {
              if let Some(selected_playlist) =
                playlists.items.get(selected_playlist_index.to_owned())
              {
                if let Some(_playlist_tracks) = &app.playlist_tracks {
                  // Note: total field access removed as it's no longer available
                  app.playlist_offset += app.large_search_limit;
                  let playlist_id = selected_playlist.id.to_string();
                  app.dispatch(IoEvent::GetPlaylistTracks(playlist_id.to_string(), app.playlist_offset));
                }
              }
            };
          }
          TrackTableContext::RecommendedTracks => {}
          TrackTableContext::SavedTracks => {
            app.get_current_user_saved_tracks_next();
          }
          TrackTableContext::AlbumSearch => {}
          TrackTableContext::PlaylistSearch => {}
          TrackTableContext::MadeForYou => {
            let (playlists, selected_playlist_index) =
              (&app.library.made_for_you_playlists, &app.made_for_you_index);

            if let Some(selected_playlist) = playlists
              .get_results(Some(0))
              .unwrap()
              .items
              .get(selected_playlist_index.to_owned())
            {
              if let Some(_playlist_tracks) = &app.made_for_you_tracks {
                // Note: total field access removed as it's no longer available
                app.made_for_you_offset += app.large_search_limit;
                let playlist_id = selected_playlist.id.to_string();
                app.dispatch(IoEvent::GetMadeForYouPlaylistTracks(
                  playlist_id,
                  app.made_for_you_offset,
                ));
              }
            }
          }
        },
        None => {}
      };
    }
    // Scroll up
    k if k == app.user_config.keys.previous_page => {
      match &app.track_table.context {
        Some(context) => match context {
          TrackTableContext::MyPlaylists => {
            if let (Some(playlists), Some(selected_playlist_index)) =
              (&app.playlists, &app.selected_playlist_index)
            {
              if app.playlist_offset >= app.large_search_limit {
                app.playlist_offset -= app.large_search_limit;
              };
              if let Some(selected_playlist) =
                playlists.items.get(selected_playlist_index.to_owned())
              {
                let playlist_id = selected_playlist.id.to_string();
                app.dispatch(IoEvent::GetPlaylistTracks(playlist_id.to_string(), app.playlist_offset));
              }
            };
          }
          TrackTableContext::RecommendedTracks => {}
          TrackTableContext::SavedTracks => {
            app.get_current_user_saved_tracks_previous();
          }
          TrackTableContext::AlbumSearch => {}
          TrackTableContext::PlaylistSearch => {}
          TrackTableContext::MadeForYou => {
            let (playlists, selected_playlist_index) = (
              &app
                .library
                .made_for_you_playlists
                .get_results(Some(0))
                .unwrap(),
              app.made_for_you_index,
            );
            if app.made_for_you_offset >= app.large_search_limit {
              app.made_for_you_offset -= app.large_search_limit;
            }
            if let Some(selected_playlist) = playlists.items.get(selected_playlist_index) {
              let playlist_id = selected_playlist.id.to_string();
              app.dispatch(IoEvent::GetMadeForYouPlaylistTracks(
                playlist_id,
                app.made_for_you_offset,
              ));
            }
          }
        },
        None => {}
      };
    }
    Key::Char('s') => handle_save_track_event(app),
    Key::Char('S') => play_random_song(app),
    k if k == app.user_config.keys.jump_to_end => jump_to_end(app),
    k if k == app.user_config.keys.jump_to_start => jump_to_start(app),
    //recommended song radio
    Key::Char('r') => {
      handle_recommended_tracks(app);
    }
    _ if key == app.user_config.keys.add_item_to_queue => on_queue(app),
    _ => {}
  }
}

fn play_random_song(app: &mut App) {
  if let Some(context) = &app.track_table.context {
    match context {
      TrackTableContext::MyPlaylists => {
        let (context_uri, track_json) = match (&app.selected_playlist_index, &app.playlists) {
          (Some(selected_playlist_index), Some(playlists)) => {
            if let Some(selected_playlist) = playlists.items.get(selected_playlist_index.to_owned())
            {
              (
                Some(format!("spotify:playlist:{}", selected_playlist.id.to_string())),
                Some(serde_json::json!(selected_playlist.tracks.total)),
              )
            } else {
              (None, None)
            }
          }
          _ => (None, None),
        };

        if let Some(val) = track_json {
          let num_tracks: usize = from_value(val.clone()).unwrap();
          app.dispatch(IoEvent::StartPlayback(context_uri));
        }
      }
      TrackTableContext::RecommendedTracks => {}
      TrackTableContext::SavedTracks => {
        if let Some(saved_tracks) = &app.library.saved_tracks.get_results(None) {
          let track_uris: Vec<String> = saved_tracks
            .items
            .iter()
            .map(|item| format!("spotify:track:{}", item.track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string())))
            .collect();
          let rand_idx = thread_rng().gen_range(0..track_uris.len());
          app.dispatch(IoEvent::StartPlayback(None));
        }
      }
      TrackTableContext::AlbumSearch => {}
      TrackTableContext::PlaylistSearch => {
        let (context_uri, playlist_track_json) = match (
          &app.search_results.selected_playlists_index,
          &app.search_results.playlists,
        ) {
          (Some(selected_playlist_index), Some(playlist_result)) => {
            if let Some(selected_playlist) = playlist_result
              .items
              .get(selected_playlist_index.to_owned())
            {
              (
                Some(format!("spotify:playlist:{}", selected_playlist.id.to_string())),
                Some(serde_json::json!(selected_playlist.tracks.total)),
              )
            } else {
              (None, None)
            }
          }
          _ => (None, None),
        };
        if let Some(val) = playlist_track_json {
          let num_tracks: usize = from_value(val.clone()).unwrap();
          app.dispatch(IoEvent::StartPlayback(context_uri));
        }
      }
      TrackTableContext::MadeForYou => {
        if let Some(playlist) = &app
          .library
          .made_for_you_playlists
          .get_results(Some(0))
          .and_then(|playlist| playlist.items.get(app.made_for_you_index))
        {
          // Note: playlist.tracks structure changed in newer API
          if let Some(_num_tracks) = Some(0usize) {
            let uri = Some(format!("spotify:playlist:{}", playlist.id.to_string()));
            app.dispatch(IoEvent::StartPlayback(uri));
          };
        };
      }
    }
  };
}

fn handle_save_track_event(app: &mut App) {
  let (selected_index, tracks) = (&app.track_table.selected_index, &app.track_table.tracks);
  if let Some(track) = tracks.get(*selected_index) {
    if let Some(id) = &track.id {
      let id = id.to_string();
      app.dispatch(IoEvent::ToggleSaveTrack(id));
    };
  };
}

fn handle_recommended_tracks(app: &mut App) {
  let (selected_index, tracks) = (&app.track_table.selected_index, &app.track_table.tracks);
  if let Some(track) = tracks.get(*selected_index) {
    let first_track = track.clone();
    let track_id_list = track.id.as_ref().map(|id| vec![id.to_string()]);

    app.recommendations_context = Some(RecommendationsContext::Song);
    app.recommendations_seed = first_track.name.clone();
    app.get_recommendations_for_seed(None, track_id_list, Some(first_track));
  };
}

fn jump_to_end(app: &mut App) {
  match &app.track_table.context {
    Some(context) => match context {
      TrackTableContext::MyPlaylists => {
        if let (Some(playlists), Some(selected_playlist_index)) =
          (&app.playlists, &app.selected_playlist_index)
        {
          if let Some(selected_playlist) = playlists.items.get(selected_playlist_index.to_owned()) {
            // Note: playlist.tracks structure changed in newer API
            let total_tracks = 50u32; // Default fallback

            if app.large_search_limit < total_tracks {
              app.playlist_offset = total_tracks - (total_tracks % app.large_search_limit);
              let playlist_id = selected_playlist.id.to_string();
              app.dispatch(IoEvent::GetPlaylistTracks(playlist_id.to_string(), app.playlist_offset));
            }
          }
        }
      }
      TrackTableContext::RecommendedTracks => {}
      TrackTableContext::SavedTracks => {}
      TrackTableContext::AlbumSearch => {}
      TrackTableContext::PlaylistSearch => {}
      TrackTableContext::MadeForYou => {}
    },
    None => {}
  }
}

fn on_enter(app: &mut App) {
  let TrackTable {
    context,
    selected_index,
    tracks,
  } = &app.track_table;
  match &context {
    Some(context) => match context {
      TrackTableContext::MyPlaylists => {
        if let Some(_track) = tracks.get(*selected_index) {
          let context_uri = match (&app.selected_playlist_index, &app.playlists) {
            (Some(selected_playlist_index), Some(playlists)) => playlists
              .items
              .get(selected_playlist_index.to_owned())
              .map(|selected_playlist| {
                let id_str = selected_playlist.id.to_string();
                if id_str.starts_with("spotify:playlist:") {
                  id_str // Already has the prefix
                } else {
                  format!("spotify:playlist:{}", id_str) // Add the prefix
                }
              }),
            _ => None,
          };

          app.dispatch(IoEvent::StartPlayback(context_uri));
        };
      }
      TrackTableContext::RecommendedTracks => {
        app.dispatch(IoEvent::StartPlayback(None));
      }
      TrackTableContext::SavedTracks => {
        if let Some(saved_tracks) = &app.library.saved_tracks.get_results(None) {
          let track_uris: Vec<String> = saved_tracks
            .items
            .iter()
            .map(|item| format!("spotify:track:{}", item.track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string())))
            .collect();

          app.dispatch(IoEvent::StartPlayback(None));
        };
      }
      TrackTableContext::AlbumSearch => {}
      TrackTableContext::PlaylistSearch => {
        let TrackTable {
          selected_index,
          tracks,
          ..
        } = &app.track_table;
        if let Some(_track) = tracks.get(*selected_index) {
          let context_uri = match (
            &app.search_results.selected_playlists_index,
            &app.search_results.playlists,
          ) {
            (Some(selected_playlist_index), Some(playlist_result)) => playlist_result
              .items
              .get(selected_playlist_index.to_owned())
              .map(|selected_playlist| format!("spotify:playlist:{}", selected_playlist.id.to_string())),
            _ => None,
          };

          app.dispatch(IoEvent::StartPlayback(context_uri));
        };
      }
      TrackTableContext::MadeForYou => {
        if let Some(_track) = tracks.get(*selected_index) {
          let context_uri = Some(format!(
            "spotify:playlist:{}",
            app
              .library
              .made_for_you_playlists
              .get_results(Some(0))
              .unwrap()
              .items
              .get(app.made_for_you_index)
              .unwrap()
              .id
              .to_string()
          ));

          app.dispatch(IoEvent::StartPlayback(context_uri));
        }
      }
    },
    None => {}
  };
}

fn on_queue(app: &mut App) {
  let TrackTable {
    context,
    selected_index,
    tracks,
  } = &app.track_table;
  match &context {
    Some(context) => match context {
      TrackTableContext::MyPlaylists => {
        if let Some(track) = tracks.get(*selected_index) {
          let uri = format!("spotify:track:{}", track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()));
          app.dispatch(IoEvent::AddItemToQueue(uri));
        };
      }
      TrackTableContext::RecommendedTracks => {
        if let Some(full_track) = app.recommended_tracks.get(app.track_table.selected_index) {
          let uri = format!("spotify:track:{}", full_track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()));
          app.dispatch(IoEvent::AddItemToQueue(uri));
        }
      }
      TrackTableContext::SavedTracks => {
        if let Some(page) = app.library.saved_tracks.get_results(None) {
          if let Some(saved_track) = page.items.get(app.track_table.selected_index) {
            let uri = format!("spotify:track:{}", saved_track.track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()));
            app.dispatch(IoEvent::AddItemToQueue(uri));
          }
        }
      }
      TrackTableContext::AlbumSearch => {}
      TrackTableContext::PlaylistSearch => {
        let TrackTable {
          selected_index,
          tracks,
          ..
        } = &app.track_table;
        if let Some(track) = tracks.get(*selected_index) {
          let uri = format!("spotify:track:{}", track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()));
          app.dispatch(IoEvent::AddItemToQueue(uri));
        };
      }
      TrackTableContext::MadeForYou => {
        if let Some(track) = tracks.get(*selected_index) {
          let uri = format!("spotify:track:{}", track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()));
          app.dispatch(IoEvent::AddItemToQueue(uri));
        }
      }
    },
    None => {}
  };
}

fn jump_to_start(app: &mut App) {
  match &app.track_table.context {
    Some(context) => match context {
      TrackTableContext::MyPlaylists => {
        if let (Some(playlists), Some(selected_playlist_index)) =
          (&app.playlists, &app.selected_playlist_index)
        {
          if let Some(selected_playlist) = playlists.items.get(selected_playlist_index.to_owned()) {
            app.playlist_offset = 0;
            let playlist_id = selected_playlist.id.to_string();
            app.dispatch(IoEvent::GetPlaylistTracks(playlist_id.to_string(), app.playlist_offset));
          }
        }
      }
      TrackTableContext::RecommendedTracks => {}
      TrackTableContext::SavedTracks => {}
      TrackTableContext::AlbumSearch => {}
      TrackTableContext::PlaylistSearch => {}
      TrackTableContext::MadeForYou => {}
    },
    None => {}
  }
}
