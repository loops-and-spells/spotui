use crate::app::{
  ActiveBlock, AlbumTableContext, App, Artist, ArtistBlock, EpisodeTableContext, RouteId,
  ScrollableResultPages, SelectedAlbum, SelectedFullAlbum, SelectedFullShow, SelectedShow,
  TrackTableContext,
};
use crate::config::ClientConfig;
use anyhow::Result;
use rspotify::{
  AuthCodeSpotify,
  clients::{BaseClient, OAuthClient},
  model::{
    album::{SimplifiedAlbum, FullAlbum},
    artist::FullArtist,
    page::Page,
    playlist::SimplifiedPlaylist,
    show::SimplifiedShow,
    track::{FullTrack, SavedTrack},
    show::SimplifiedEpisode,
    PlayableItem,
    CurrentPlaybackContext,
    enums::{Country, RepeatState as SpotifyRepeatState, SearchType, AdditionalType},
  },
};
use serde_json;
use std::{
  sync::Arc,
  time::{Duration, Instant, SystemTime},
  fs::OpenOptions,
  io::Write,
};
use tokio::sync::Mutex;
use futures::stream::TryStreamExt;
use chrono::{Duration as ChronoDuration};

#[derive(Debug)]
pub enum IoEvent {
  GetPlaylists,
  GetUser,
  GetCurrentPlayback,
  UpdateSearchLimits(u32, u32),
  RefreshAuthentication,
  GetPlaylistTracks(String, u32),
  GetAlbumTracks(String),
  GetArtist(String),
  GetArtistAlbums(String),
  GetShow(String),
  GetEpisodes(String),
  GetRecommendations(String, String, String, String, String),
  GetSearchResults(String),
  StartPlayback(Option<String>, Option<String>),
  PausePlayback,
  NextTrack,
  PreviousTrack,
  Seek(u32),
  Shuffle(bool),
  Repeat(RepeatState),
  VolumeUp,
  VolumeDown,
  SetVolume(u8),
  TransferPlaybackToDevice(String),
  GetDevices,
  ToggleSaveTrack(String),
  GetAudioAnalysis(String),
  AddItemToQueue(String),
  CurrentUserSavedAlbumAdd(String),
  GetShowEpisodes(Box<SimplifiedShow>),
  GetAlbum(String),
  GetAlbumForTrack(String),
  GetRecentlyPlayed,
  GetCurrentSavedTracks(Option<u32>),
  GetCurrentUserSavedAlbums(Option<u32>),
  GetFollowedArtists(Option<String>),
  GetCurrentUserSavedShows(Option<u32>),
  GetTopTracks,
  GetTopArtists,
  FetchAlbumArt(String),
}

// Compatibility types
#[derive(Debug, Clone)]
pub enum PlayingItem {
  Track(FullTrack),
  Episode(SimplifiedEpisode),
}

impl From<PlayableItem> for PlayingItem {
  fn from(item: PlayableItem) -> Self {
    match item {
      PlayableItem::Track(track) => PlayingItem::Track(track),
      PlayableItem::Episode(episode) => {
        // Convert FullEpisode to SimplifiedEpisode
        let simplified_episode = SimplifiedEpisode {
          audio_preview_url: episode.audio_preview_url,
          description: episode.description,
          duration: episode.duration,
          explicit: episode.explicit,
          external_urls: episode.external_urls,
          href: episode.href,
          id: episode.id,
          images: episode.images,
          is_externally_hosted: episode.is_externally_hosted,
          is_playable: episode.is_playable,
          language: episode.language,
          languages: episode.languages,
          name: episode.name,
          release_date: episode.release_date,
          release_date_precision: episode.release_date_precision,
          resume_point: episode.resume_point,
        };
        PlayingItem::Episode(simplified_episode)
      },
    }
  }
}

#[derive(Debug, Clone)]
pub enum RepeatState {
  Off,
  Track,
  Context,
}

impl From<SpotifyRepeatState> for RepeatState {
  fn from(state: SpotifyRepeatState) -> Self {
    match state {
      SpotifyRepeatState::Off => RepeatState::Off,
      SpotifyRepeatState::Track => RepeatState::Track,
      SpotifyRepeatState::Context => RepeatState::Context,
    }
  }
}

impl Into<SpotifyRepeatState> for RepeatState {
  fn into(self) -> SpotifyRepeatState {
    match self {
      RepeatState::Off => SpotifyRepeatState::Off,
      RepeatState::Track => SpotifyRepeatState::Track,
      RepeatState::Context => SpotifyRepeatState::Context,
    }
  }
}

pub struct Network {
  pub spotify: AuthCodeSpotify,
  pub client_config: ClientConfig,
  pub app: Arc<Mutex<App>>,
  pub large_search_limit: u32,
  pub small_search_limit: u32,
}

impl Network {
  pub fn new(spotify: AuthCodeSpotify, client_config: ClientConfig, app: &Arc<Mutex<App>>) -> Self {
    Self {
      spotify,
      client_config,
      app: Arc::clone(app),
      large_search_limit: 20,
      small_search_limit: 4,
    }
  }

  fn log_error(&self, message: &str) {
    // Don't print to stdout - this interferes with TUI
    
    if let Ok(mut file) = OpenOptions::new()
      .create(true)
      .append(true)
      .open("/tmp/spotify-tui-errors.log") 
    {
      let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
      let _ = writeln!(file, "[{}] {}", timestamp, message);
    }
  }

  pub async fn handle_network_event(&mut self, io_event: IoEvent) {
    match io_event {
      IoEvent::GetPlaylists => {
        self.get_playlists().await;
      }
      IoEvent::GetUser => {
        self.get_user().await;
      }
      IoEvent::GetCurrentPlayback => {
        self.get_current_playback().await;
      }
      IoEvent::UpdateSearchLimits(large, small) => {
        self.large_search_limit = large;
        self.small_search_limit = small;
      }
      IoEvent::RefreshAuthentication => {
        self.refresh_authentication().await;
      }
      IoEvent::GetPlaylistTracks(playlist_id, offset) => {
        self.get_playlist_tracks(&playlist_id, offset).await;
      }
      IoEvent::StartPlayback(context_uri, offset) => {
        self.start_playback(context_uri.as_deref(), offset).await;
      }
      IoEvent::PausePlayback => {
        self.pause_playback().await;
      }
      IoEvent::NextTrack => {
        self.next_track().await;
      }
      IoEvent::PreviousTrack => {
        self.previous_track().await;
      }
      IoEvent::Seek(position_ms) => {
        self.seek(position_ms).await;
      }
      IoEvent::Shuffle(state) => {
        self.shuffle(state).await;
      }
      IoEvent::Repeat(state) => {
        self.repeat(state).await;
      }
      IoEvent::SetVolume(volume) => {
        self.set_volume(volume).await;
      }
      IoEvent::TransferPlaybackToDevice(device_id) => {
        self.transfer_playback_to_device(device_id).await;
      }
      IoEvent::GetDevices => {
        self.get_devices().await;
      }
      IoEvent::ToggleSaveTrack(track_id) => {
        // TODO: Implement toggle save track
        self.log_error(&format!("TODO: ToggleSaveTrack: {}", track_id));
      }
      IoEvent::AddItemToQueue(uri) => {
        // TODO: Implement add to queue
        self.log_error(&format!("TODO: AddItemToQueue: {}", uri));
      }
      IoEvent::CurrentUserSavedAlbumAdd(album_id) => {
        // TODO: Implement save album
        // TODO: Implement CurrentUserSavedAlbumAdd
      }
      IoEvent::GetShowEpisodes(show) => {
        // TODO: Implement get show episodes
        // TODO: Implement GetShowEpisodes
      }
      IoEvent::GetArtist(artist_id) => {
        self.get_artist(artist_id).await;
      }
      IoEvent::GetAlbumTracks(album_id) => {
        self.get_album_tracks(album_id).await;
      }
      IoEvent::GetAlbum(album_id) => {
        // TODO: Implement get album
        // TODO: Implement GetAlbum
      }
      IoEvent::GetAlbumForTrack(track_id) => {
        // TODO: Implement get album for track
        // TODO: Implement GetAlbumForTrack
      }
      IoEvent::GetRecentlyPlayed => {
        self.get_recently_played().await;
      }
      IoEvent::GetCurrentSavedTracks(offset) => {
        self.get_current_saved_tracks(offset).await;
      }
      IoEvent::GetCurrentUserSavedAlbums(offset) => {
        self.get_current_user_saved_albums(offset).await;
      }
      IoEvent::GetFollowedArtists(after) => {
        self.get_followed_artists(after).await;
      }
      IoEvent::GetCurrentUserSavedShows(offset) => {
        self.get_current_user_saved_shows(offset).await;
      }
      IoEvent::GetTopTracks => {
        self.get_top_tracks().await;
      }
      IoEvent::GetTopArtists => {
        self.get_top_artists().await;
      }
      IoEvent::FetchAlbumArt(url) => {
        self.fetch_album_art(url).await;
      }
      // Add more handlers as needed
      _ => {
        // Unhandled network event
      }
    }
  }

  async fn get_playlists(&mut self) {
    self.log_error("DEBUG: Starting get_playlists");
    use futures::StreamExt;
    
    let mut stream = self.spotify.current_user_playlists();
    let mut playlists = Vec::new();
    let mut count = 0;
    
    while let Some(playlist_result) = stream.next().await {
      match playlist_result {
        Ok(playlist) => {
          playlists.push(playlist);
          count += 1;
          if count >= 50 { // Limit to 50 playlists
            break;
          }
        }
        Err(e) => {
          let error_msg = format!("DETAILED ERROR getting playlists: {:?}", e);
          let type_msg = format!("Error type: {}", std::any::type_name_of_val(&e));
          self.log_error(&error_msg);
          self.log_error(&type_msg);
          let mut app = self.app.lock().await;
          app.handle_error(anyhow::anyhow!("Failed to load playlists: {}", e));
          return;
        }
      }
    }
    
    self.log_error(&format!("SUCCESS: Got {} playlists", playlists.len()));
    
    // Store playlists in app state
    let mut app = self.app.lock().await;
    // Create a Page structure to match the expected type
    let page = Page {
      items: playlists,
      limit: 50,
      offset: 0,
      total: 50, // This would ideally come from the API response
      next: None,
      previous: None,
      href: String::new(),
    };
    app.playlists = Some(page);
    // Set loading to false after playlists are loaded
    app.is_loading = false;
  }

  async fn get_user(&mut self) {
    match self.spotify.me().await {
      Ok(user) => {
        let mut app = self.app.lock().await;
        // Note: user_country field may need to be added to App struct
        // app.user_country = user.country;
        // User info received - logged via app.add_log_message
      }
      Err(e) => {
        // Error handled via app.handle_error
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Failed to get user info: {}", e));
      }
    }
  }

  async fn get_current_playback(&mut self) {
    // Try to get the full playback context which includes device information
    match self.spotify.current_playback(None, None::<&[_]>).await {
      Ok(Some(context)) => {
        let mut app = self.app.lock().await;
        
        // Don't log playback status on every poll to avoid spam
        
        // Store the playback context  
        app.current_playback_context = Some(context);
        
        // Update album art for the current track
        app.update_album_art();
        
        // Reset polling state
        app.is_fetching_current_playback = false;
        app.instant_since_last_current_playback_poll = std::time::Instant::now();
      }
      Ok(None) => {
        let mut app = self.app.lock().await;
        app.current_playback_context = None;
        
        // Reset polling state
        app.is_fetching_current_playback = false;
        app.instant_since_last_current_playback_poll = std::time::Instant::now();
      }
      Err(e) => {
        let mut app = self.app.lock().await;
        // Don't log polling errors to avoid spam
        
        // Reset polling state even on error
        app.is_fetching_current_playback = false;
        app.instant_since_last_current_playback_poll = std::time::Instant::now();
      }
    }
  }

  async fn get_playlist_tracks(&mut self, playlist_id: &str, offset: u32) {
    use rspotify::model::PlaylistId;
    
    self.log_error(&format!("DEBUG: get_playlist_tracks called with ID: '{}'", playlist_id));
    
    // Extract just the ID from the Spotify URI (e.g., "spotify:playlist:ID" -> "ID")
    let id_part = if playlist_id.starts_with("spotify:playlist:") {
      &playlist_id[17..] // Skip "spotify:playlist:"
    } else {
      playlist_id // Assume it's already just the ID
    };
    
    let playlist_id = match PlaylistId::from_id(id_part) {
      Ok(id) => id,
      Err(e) => {
        let error_msg = format!("ERROR: Invalid playlist ID '{}' (extracted: '{}'): {:?}", playlist_id, id_part, e);
        self.log_error(&error_msg);
        return;
      }
    };
    let mut stream = self.spotify.playlist_items(playlist_id, None, None);
    let mut playlist_items = Vec::new();
    
    while let Some(item) = stream.try_next().await.unwrap_or(None) {
      playlist_items.push(item);
    }
    
    self.log_error(&format!("SUCCESS: Got {} playlist items", playlist_items.len()));
    
    // Convert PlaylistItems to FullTracks (only tracks, not episodes)
    let mut tracks = Vec::new();
    for item in playlist_items {
      if let Some(track) = item.track {
        match track {
          PlayableItem::Track(full_track) => {
            tracks.push(full_track);
          }
          PlayableItem::Episode(_) => {
            // Skip episodes for now since track_table expects only tracks
          }
        }
      }
    }
    
    self.log_error(&format!("SUCCESS: Extracted {} tracks from playlist", tracks.len()));
    
    let mut app = self.app.lock().await;
    // Store playlist tracks in app.track_table for display in right panel
    app.track_table.tracks = tracks;
    app.track_table.context = Some(TrackTableContext::MyPlaylists);
    app.track_table.selected_index = 0;
  }

  async fn get_album_tracks(&mut self, album_id: String) {
    use rspotify::model::AlbumId;
    use futures::TryStreamExt;
    
    self.log_error(&format!("DEBUG: get_album_tracks called with ID: '{}'", album_id));
    
    // Extract just the ID from the Spotify URI if present
    let id_part = if album_id.starts_with("spotify:album:") {
      &album_id[14..] // Skip "spotify:album:"
    } else {
      &album_id // Assume it's already just the ID
    };
    
    let album_id = match AlbumId::from_id(id_part) {
      Ok(id) => id,
      Err(e) => {
        let error_msg = format!("ERROR: Invalid album ID '{}' (extracted: '{}'): {:?}", album_id, id_part, e);
        self.log_error(&error_msg);
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Invalid album ID: {}", e));
        return;
      }
    };
    
    // Get the album details first to get album name and other info
    let album = match self.spotify.album(album_id.clone(), None).await {
      Ok(album) => {
        self.log_error(&format!("SUCCESS: Got album: {}", album.name));
        album
      }
      Err(e) => {
        let error_msg = format!("ERROR getting album details: {:?}", e);
        self.log_error(&error_msg);
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Failed to get album: {}", e));
        return;
      }
    };
    
    // Get album tracks using the stream API
    let mut stream = self.spotify.album_track(album_id, None);
    let mut tracks = Vec::new();
    
    while let Some(track) = stream.try_next().await.unwrap_or(None) {
      // Convert SimplifiedTrack to FullTrack-like structure for the UI
      tracks.push(FullTrack {
        artists: track.artists,
        available_markets: track.available_markets.clone().unwrap_or_default(),
        disc_number: track.disc_number,
        duration: track.duration,
        explicit: track.explicit,
        external_ids: Default::default(),
        external_urls: track.external_urls,
        href: track.href.clone(),
        id: track.id,
        is_local: track.is_local,
        is_playable: track.is_playable,
        linked_from: track.linked_from,
        restrictions: track.restrictions,
        name: track.name,
        popularity: 0, // SimplifiedTrack doesn't have popularity
        preview_url: track.preview_url,
        track_number: track.track_number,
        album: SimplifiedAlbum {
          album_type: Some(format!("{:?}", album.album_type)),
          artists: album.artists.clone(),
          available_markets: album.available_markets.clone().unwrap_or_default(),
          external_urls: album.external_urls.clone(),
          href: Some(album.href.clone()),
          id: Some(album.id.clone()),
          images: album.images.clone(),
          name: album.name.clone(),
          release_date: Some(album.release_date.clone()),
          release_date_precision: Some(format!("{:?}", album.release_date_precision)),
          restrictions: None,
          album_group: None,
        },
      });
    }
    
    self.log_error(&format!("SUCCESS: Got {} tracks from album", tracks.len()));
    
    let mut app = self.app.lock().await;
    // Store album tracks in app.track_table for display
    app.track_table.tracks = tracks;
    app.track_table.context = Some(TrackTableContext::AlbumSearch);
    app.track_table.selected_index = 0;
    
    // Store the album URI for playback
    app.selected_album_full = Some(SelectedFullAlbum {
      album,
      selected_index: 0,
    });
  }

  async fn start_playback(&mut self, context_uri: Option<&str>, offset_uri: Option<String>) {
    self.log_error(&format!("DEBUG: start_playback called with context_uri: {:?}, offset_uri: {:?}", context_uri, offset_uri));
    
    // Add to log stream for visibility
    {
      let mut app = self.app.lock().await;
      app.add_log_message(format!("Starting playback - Context: {:?}, Offset: {:?}", context_uri, offset_uri));
    }
    
    // Log detailed information
    if let Some(uri) = context_uri {
      self.log_error(&format!("DEBUG: Context URI type: {}", 
        if uri.contains("playlist") { "playlist" } 
        else if uri.contains("album") { "album" } 
        else if uri.contains("track") { "track" } 
        else { "unknown" }
      ));
    }
    if let Some(ref offset) = offset_uri {
      self.log_error(&format!("DEBUG: Offset URI: {}", offset));
    }
    
    let result = if let Some(uri) = context_uri {
      self.log_error(&format!("DEBUG: Starting playback with context URI: {}", uri));
      
      // Parse the URI to get the appropriate ID and call the right API
      if uri.starts_with("spotify:playlist:") {
        let playlist_id = &uri[17..]; // Remove "spotify:playlist:" prefix
        match rspotify::model::PlaylistId::from_id(playlist_id) {
          Ok(id) => {
            // Convert to PlayContextId 
            use rspotify::model::PlayContextId;
            let context = PlayContextId::Playlist(id);
            
            // For playlists, if we have an offset_uri (track URI), use it
            let offset = offset_uri.as_ref().map(|uri| {
              self.log_error(&format!("DEBUG: Using track URI offset: {}", uri));
              rspotify::model::Offset::Uri(uri.clone())
            });
            
            // Get current device ID from app state
            let device_id = {
              let app = self.app.lock().await;
              app.current_playback_context.as_ref()
                .and_then(|ctx| ctx.device.id.as_ref())
                .map(|id| id.to_string())
            };
            
            self.spotify.start_context_playback(context, device_id.as_deref(), offset, None).await
          }
          Err(e) => {
            self.log_error(&format!("ERROR: Invalid playlist ID in URI '{}': {:?}", uri, e));
            return;
          }
        }
      } else if uri.starts_with("spotify:album:") {
        let album_id = &uri[14..]; // Remove "spotify:album:" prefix
        match rspotify::model::AlbumId::from_id(album_id) {
          Ok(id) => {
            use rspotify::model::PlayContextId;
            let context = PlayContextId::Album(id);
            // For albums, if we have an offset_uri (track URI), use it
            let offset = offset_uri.as_ref().map(|uri| {
              self.log_error(&format!("DEBUG: Using track URI offset for album: {}", uri));
              rspotify::model::Offset::Uri(uri.clone())
            });
            
            // Get current device ID from app state
            let device_id = {
              let app = self.app.lock().await;
              app.current_playback_context.as_ref()
                .and_then(|ctx| ctx.device.id.as_ref())
                .map(|id| id.to_string())
            };
            
            self.spotify.start_context_playback(context, device_id.as_deref(), offset, None).await
          }
          Err(e) => {
            self.log_error(&format!("ERROR: Invalid album ID in URI '{}': {:?}", uri, e));
            return;
          }
        }
      } else if uri.starts_with("spotify:track:") {
        let track_id = &uri[14..]; // Remove "spotify:track:" prefix
        match rspotify::model::TrackId::from_id(track_id) {
          Ok(id) => {
            // For individual tracks, use start_uris_playback
            use rspotify::model::PlayableId;
            let track_ids = vec![PlayableId::Track(id)];
            // Get current device ID from app state
            let device_id = {
              let app = self.app.lock().await;
              app.current_playback_context.as_ref()
                .and_then(|ctx| ctx.device.id.as_ref())
                .map(|id| id.to_string())
            };
            
            self.spotify.start_uris_playback(track_ids, device_id.as_deref(), None, None).await
          }
          Err(e) => {
            self.log_error(&format!("ERROR: Invalid track ID in URI '{}': {:?}", uri, e));
            return;
          }
        }
      } else {
        self.log_error(&format!("ERROR: Unsupported URI format: {}", uri));
        return;
      }
    } else {
      // Resume current playback
      self.log_error("DEBUG: Resuming current playback");
      // Get current device ID from app state
      let device_id = {
        let app = self.app.lock().await;
        app.current_playback_context.as_ref()
          .and_then(|ctx| ctx.device.id.as_ref())
          .map(|id| id.to_string())
      };
      
      self.spotify.resume_playback(device_id.as_deref(), None).await
    };
    
    match result {
      Ok(_) => {
        self.log_error("SUCCESS: Started playback");
        let mut app = self.app.lock().await;
        app.add_log_message("Playback started".to_string());
        // Update the playback state when resuming
        if context_uri.is_none() && offset_uri.is_none() {
          // This was a resume operation, update the state
          if let Some(ref mut context) = app.current_playback_context {
            context.is_playing = true;
          }
          // Schedule a playback state refresh
          app.dispatch(IoEvent::GetCurrentPlayback);
        }
      }
      Err(e) => {
        let error_msg = format!("ERROR: Failed to start playback: {:?}", e);
        self.log_error(&error_msg);
        
        // Extract and format detailed error information
        let error_str = format!("{:?}", e);
        
        // Handle both Http(StatusCode) and ApiError formats
        if error_str.contains("Http(StatusCode(Response") {
          // Extract status code
          let status = if error_str.contains("status: 400") { 
            "400 Bad Request" 
          } else if error_str.contains("status: 403") { 
            "403 Forbidden" 
          } else if error_str.contains("status: 404") { 
            "404 Not Found" 
          } else { 
            "Unknown Status" 
          };
          
          let mut app = self.app.lock().await;
          
          // For now, add a simple error message since HTTP errors don't include body
          app.add_log_message(format!("ERROR: Playback failed - {}", status));
          app.add_log_message("Check that a Spotify device is active and try again".to_string());
          
          // Log the full error for debugging
          self.log_error(&format!("Full HTTP error: {}", error_str));
        }
        // Try to extract and format the error response body if it exists
        else if let Some(start) = error_str.find("ApiError(") {
          if let Some(end) = error_str.rfind(')') {
            let api_error = &error_str[start+9..end];
            
            // Log the error in parts for better readability
            self.log_error("=== SPOTIFY API ERROR ===");
            let api_status = if error_str.contains("status: 400") { "400 Bad Request" } else if error_str.contains("status: 403") { "403 Forbidden" } else { "Unknown" };
            self.log_error(&format!("Status: {}", api_status));
            
            // Try to extract JSON body
            if let Some(body_start) = api_error.find("body: Some(\"") {
              if let Some(body_end) = api_error[body_start..].find("\")") {
                let body = &api_error[body_start+12..body_start+body_end];
                // Unescape the JSON string
                let unescaped_body = body.replace("\\\"", "\"").replace("\\n", "\n");
                
                self.log_error("Response body:");
                // Split into multiple lines for readability
                for line in unescaped_body.lines() {
                  self.log_error(&format!("  {}", line));
                }
                
                // Try to parse and pretty print JSON
                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&unescaped_body) {
                  if let Ok(pretty_json) = serde_json::to_string_pretty(&json_value) {
                    let mut app = self.app.lock().await;
                    // Add the entire error as a single multi-line message
                    let error_message = format!(
                      "=== SPOTIFY API ERROR ({}) ===\n{}\n==========================================",
                      api_status, pretty_json
                    );
                    app.add_log_message(error_message);
                  }
                }
              }
            }
            
            self.log_error("=========================");
          }
        }
        
        // Check if it's a 400 error
        if error_msg.contains("status: 400") {
          self.log_error("BAD REQUEST: The request format is incorrect");
          let mut app = self.app.lock().await;
          if !error_str.contains("body: Some") {
            app.add_log_message(format!("Bad Request (400): {}", error_str));
          }
        }
        // Check if it's a 403 error which usually means Premium required or no active device
        else if error_msg.contains("status: 403") {
          let user_error = "Playback failed: Spotify Premium subscription required. Please upgrade to Premium and ensure you have an active device (open Spotify and start playing music on any device).";
          self.log_error("PREMIUM REQUIRED: Playback control needs Spotify Premium");
          
          // Add to log stream and show in UI
          let mut app = self.app.lock().await;
          app.add_log_message("Spotify Premium required for playback control".to_string());
          app.handle_error(anyhow::anyhow!("{}", user_error));
        } else {
          let mut app = self.app.lock().await;
          app.add_log_message(format!("Playback error: {}", e));
          app.handle_error(anyhow::anyhow!("Failed to start playback: {}", e));
        }
      }
    }
  }

  async fn pause_playback(&mut self) {
    // Get current device ID from app state
    let device_id = {
      let app = self.app.lock().await;
      app.current_playback_context.as_ref()
        .and_then(|ctx| ctx.device.id.as_ref())
        .map(|id| id.to_string())
    };
    
    match self.spotify.pause_playback(device_id.as_deref()).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.add_log_message("Playback paused".to_string());
        // Update the playback state locally
        if let Some(ref mut context) = app.current_playback_context {
          context.is_playing = false;
        }
        // Schedule a playback state refresh
        app.dispatch(IoEvent::GetCurrentPlayback);
      },
      Err(e) => {
        let error_msg = format!("{:?}", e);
        self.log_error(&format!("Pause error: {}", error_msg));
        
        // For 403 errors, don't show the premium error immediately
        // It might be a temporary issue with the device
        if error_msg.contains("status: 403") {
          let mut app = self.app.lock().await;
          // Just log it without showing an error dialog
          app.add_log_message("Failed to pause - try again or check device".to_string());
          // Update the state anyway to keep UI in sync
          if let Some(ref mut context) = app.current_playback_context {
            context.is_playing = false;
          }
        } else if error_msg.contains("status: 404") {
          let mut app = self.app.lock().await;
          app.add_log_message("No active device found for pause".to_string());
        } else {
          let mut app = self.app.lock().await;
          app.add_log_message(format!("Pause error: {}", e));
          // Don't show error dialog for pause failures
        }
      }
    }
  }

  async fn next_track(&mut self) {
    // Get current device ID from app state
    let device_id = {
      let app = self.app.lock().await;
      app.current_playback_context.as_ref()
        .and_then(|ctx| ctx.device.id.as_ref())
        .map(|id| id.to_string())
    };
    
    match self.spotify.next_track(device_id.as_deref()).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.add_log_message("Skipped to next track".to_string());
        // Skipped to next - already logged
      },
      Err(e) => {
        let error_msg = format!("{:?}", e);
        if error_msg.contains("status: 403") {
          let mut app = self.app.lock().await;
          app.add_log_message("Spotify Premium required for next track control".to_string());
          app.handle_error(anyhow::anyhow!("Spotify Premium required for playback controls"));
        } else {
          let mut app = self.app.lock().await;
          app.add_log_message(format!("Next track error: {}", e));
          app.handle_error(anyhow::anyhow!("Error skipping to next track: {}", e));
        }
      }
    }
  }

  async fn previous_track(&mut self) {
    // Get current device ID from app state
    let device_id = {
      let app = self.app.lock().await;
      app.current_playback_context.as_ref()
        .and_then(|ctx| ctx.device.id.as_ref())
        .map(|id| id.to_string())
    };
    
    match self.spotify.previous_track(device_id.as_deref()).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.add_log_message("Skipped to previous track".to_string());
        // Skipped to previous - already logged
      },
      Err(e) => {
        let error_msg = format!("{:?}", e);
        if error_msg.contains("status: 403") {
          let mut app = self.app.lock().await;
          app.add_log_message("Spotify Premium required for previous track control".to_string());
          app.handle_error(anyhow::anyhow!("Spotify Premium required for playback controls"));
        } else {
          let mut app = self.app.lock().await;
          app.add_log_message(format!("Previous track error: {}", e));
          app.handle_error(anyhow::anyhow!("Error skipping to previous track: {}", e));
        }
      }
    }
  }

  async fn seek(&mut self, position_ms: u32) {
    let duration = ChronoDuration::milliseconds(position_ms as i64);
    // Get current device ID from app state
    let device_id = {
      let app = self.app.lock().await;
      app.current_playback_context.as_ref()
        .and_then(|ctx| ctx.device.id.as_ref())
        .map(|id| id.to_string())
    };
    
    match self.spotify.seek_track(duration, device_id.as_deref()).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.add_log_message(format!("Seeked to position: {}ms", position_ms));
      }
      Err(e) => {
        let error_msg = format!("{:?}", e);
        if error_msg.contains("status: 403") {
          let mut app = self.app.lock().await;
          app.add_log_message("Spotify Premium required for seek control".to_string());
          app.handle_error(anyhow::anyhow!("Spotify Premium required for playback controls"));
        } else {
          let mut app = self.app.lock().await;
          app.add_log_message(format!("Seek error: {}", e));
          app.handle_error(anyhow::anyhow!("Error seeking to position: {}", e));
        }
      }
    }
  }

  async fn shuffle(&mut self, state: bool) {
    // Get current device ID from app state
    let device_id = {
      let app = self.app.lock().await;
      app.current_playback_context.as_ref()
        .and_then(|ctx| ctx.device.id.as_ref())
        .map(|id| id.to_string())
    };
    
    match self.spotify.shuffle(state, device_id.as_deref()).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.add_log_message(format!("Set shuffle to: {}", state));
      }
      Err(e) => {
        let error_msg = format!("{:?}", e);
        if error_msg.contains("status: 403") {
          let mut app = self.app.lock().await;
          app.add_log_message("Spotify Premium required for shuffle control".to_string());
          app.handle_error(anyhow::anyhow!("Spotify Premium required for playback controls"));
        } else {
          let mut app = self.app.lock().await;
          app.add_log_message(format!("Shuffle error: {}", e));
          app.handle_error(anyhow::anyhow!("Error setting shuffle: {}", e));
        }
      }
    }
  }

  async fn repeat(&mut self, state: RepeatState) {
    let spotify_state: SpotifyRepeatState = state.into();
    // Get current device ID from app state
    let device_id = {
      let app = self.app.lock().await;
      app.current_playback_context.as_ref()
        .and_then(|ctx| ctx.device.id.as_ref())
        .map(|id| id.to_string())
    };
    
    match self.spotify.repeat(spotify_state, device_id.as_deref()).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.add_log_message(format!("Set repeat to: {:?}", spotify_state));
      }
      Err(e) => {
        let error_msg = format!("{:?}", e);
        if error_msg.contains("status: 403") {
          let mut app = self.app.lock().await;
          app.add_log_message("Spotify Premium required for repeat control".to_string());
          app.handle_error(anyhow::anyhow!("Spotify Premium required for playback controls"));
        } else {
          let mut app = self.app.lock().await;
          app.add_log_message(format!("Repeat error: {}", e));
          app.handle_error(anyhow::anyhow!("Error setting repeat mode: {}", e));
        }
      }
    }
  }

  async fn set_volume(&mut self, volume: u8) {
    match self.spotify.volume(volume, None).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.add_log_message(format!("Set volume to: {}%", volume));
      }
      Err(e) => {
        let error_msg = format!("{:?}", e);
        if error_msg.contains("status: 403") {
          let mut app = self.app.lock().await;
          app.add_log_message("Spotify Premium required for volume control".to_string());
          app.handle_error(anyhow::anyhow!("Spotify Premium required for volume control"));
        } else {
          let mut app = self.app.lock().await;
          app.add_log_message(format!("Volume error: {}", e));
          app.handle_error(anyhow::anyhow!("Error setting volume: {}", e));
        }
      }
    }
  }

  async fn transfer_playback_to_device(&mut self, device_id: String) {
    self.log_error(&format!("DEBUG: Transferring playback to device: {}", device_id));
    
    // Transfer playback with play=true to activate the device
    match self.spotify.transfer_playback(&device_id, Some(true)).await {
          Ok(_) => {
            self.log_error("SUCCESS: Playback transferred to device");
            let mut app = self.app.lock().await;
            app.add_log_message(format!("Playback transferred to device"));
            
            // Store the active device ID for future playback commands
            if let Some(devices) = &app.devices {
              if let Some(device) = devices.devices.iter().find(|d| d.id.as_ref().map(|id| id.to_string()) == Some(device_id.to_string())) {
                app.current_playback_context = Some(rspotify::model::CurrentPlaybackContext {
                  device: device.clone(),
                  repeat_state: rspotify::model::RepeatState::Off,
                  shuffle_state: false,
                  context: None,
                  timestamp: chrono::Utc::now(),
                  progress: None,
                  is_playing: false,
                  item: None,
                  currently_playing_type: rspotify::model::CurrentlyPlayingType::Track,
                  actions: rspotify::model::Actions {
                    disallows: Vec::new(),
                  },
                });
              }
            }
          }
      Err(e) => {
        self.log_error(&format!("ERROR transferring playback: {:?}", e));
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Failed to transfer playback: {}", e));
      }
    }
  }

  async fn get_devices(&mut self) {
    match self.spotify.device().await {
      Ok(devices) => {
        let mut app = self.app.lock().await;
        app.add_log_message(format!("Found {} devices", devices.len()));
        // Create DevicePayload structure
        let device_payload = rspotify::model::device::DevicePayload { devices };
        app.devices = Some(device_payload);
        
        // Only set selected index if there are devices
        if !app.devices.as_ref().unwrap().devices.is_empty() {
          app.selected_device_index = Some(0); // Select first device by default
        }
        
        // No need to navigate - we're already on the device screen
        app.add_log_message("Devices loaded successfully".to_string());
      }
      Err(e) => {
        let mut app = self.app.lock().await;
        app.add_log_message(format!("Error fetching devices: {}", e));
        // Error already logged
      }
    }
  }

  async fn refresh_authentication(&mut self) {
    // Refreshing authentication token
    
    match self.spotify.refresh_token().await {
      Ok(_) => {
        // Token refreshed successfully
        
        // Update token cache
        let config_paths = match self.client_config.get_or_build_paths() {
          Ok(paths) => paths,
          Err(e) => {
            // Error getting config paths
            return;
          }
        };
        
        // Manually write the token cache
        if let Ok(token_guard) = self.spotify.token.lock().await {
          if let Some(token) = token_guard.as_ref() {
            match serde_json::to_string_pretty(token) {
              Ok(token_json) => {
                match std::fs::write(&config_paths.token_cache_path, token_json) {
                  Ok(_) => {
                    self.log_error("Successfully updated token cache");
                  }
                  Err(e) => {
                    self.log_error(&format!("Failed to write token cache file: {}", e));
                  }
                }
              }
              Err(e) => {
                self.log_error(&format!("Failed to serialize token: {}", e));
              }
            }
          }
        }
        
        // Update app token expiry
        if let Ok(token_guard) = self.spotify.token.lock().await {
          if let Some(token) = token_guard.as_ref() {
            if let Some(expires_at) = token.expires_at {
              let mut app = self.app.lock().await;
              app.spotify_token_expiry = expires_at.into();
            }
          }
        }
      }
      Err(e) => {
        // Error refreshing token - handled below
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Authentication failed: {}", e));
      }
    }
  }

  async fn get_current_saved_tracks(&mut self, offset: Option<u32>) {
    self.log_error("DEBUG: Starting get_current_saved_tracks");
    use futures::{StreamExt, TryStreamExt};
    
    // Create a stream starting from the offset
    let stream = self.spotify.current_user_saved_tracks(None);
    
    // Skip to the offset if provided
    let skip_count = offset.unwrap_or(0) as usize;
    let tracks: Result<Vec<_>, _> = stream.skip(skip_count).take(50).try_collect().await;
    
    match tracks {
      Ok(saved_tracks) => {
        self.log_error(&format!("SUCCESS: Got {} saved tracks", saved_tracks.len()));
        let mut app = self.app.lock().await;
        
        // Set the tracks in the track table for display
        app.track_table.tracks = saved_tracks.iter().map(|saved_track| {
          saved_track.track.clone()
        }).collect();
        
        // Create a Page<SavedTrack> to store in library.saved_tracks
        let page = Page {
          href: String::new(), // Not available from stream API
          items: saved_tracks,
          total: 50, // We don't have total count from stream API
          limit: 50,
          offset: offset.unwrap_or(0),
          next: None,
          previous: None,
        };
        
        // Initialize or update the saved tracks in the library
        app.library.saved_tracks = ScrollableResultPages::new();
        app.library.saved_tracks.pages.push(page);
        
        // Set context so the UI knows we're showing saved tracks
        app.track_table.context = Some(TrackTableContext::SavedTracks);
        
        let track_count = app.track_table.tracks.len();
        app.add_log_message(format!("Loaded {} liked songs", track_count));
      }
      Err(e) => {
        let error_msg = format!("DETAILED ERROR getting saved tracks: {:?}", e);
        let type_msg = format!("Error type: {}", std::any::type_name_of_val(&e));
        self.log_error(&error_msg);
        self.log_error(&type_msg);
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Failed to load saved tracks: {}", e));
      }
    }
  }

  async fn get_current_user_saved_albums(&mut self, offset: Option<u32>) {
    self.log_error("DEBUG: Starting get_current_user_saved_albums");
    use futures::{StreamExt, TryStreamExt};
    
    let stream = self.spotify.current_user_saved_albums(None);
    let skip_count = offset.unwrap_or(0) as usize;
    let albums: Result<Vec<_>, _> = stream.skip(skip_count).take(50).try_collect().await;
    
    match albums {
      Ok(saved_albums) => {
        self.log_error(&format!("SUCCESS: Got {} saved albums", saved_albums.len()));
        let mut app = self.app.lock().await;
        
        // Create a Page-like structure for the UI
        use rspotify::model::page::Page;
        let page = Page {
          items: saved_albums,
          total: 0, // We don't have the total from stream
          limit: 50,
          offset: offset.unwrap_or(0),
          href: String::new(),
          next: None,
          previous: None,
        };
        
        // Store the page in the library
        app.library.saved_albums.add_pages(page);
        
        let album_count = app.library.saved_albums.get_results(None).map(|p| p.items.len()).unwrap_or(0);
        app.add_log_message(format!("Loaded {} saved albums", album_count));
      }
      Err(e) => {
        let error_msg = format!("DETAILED ERROR getting saved albums: {:?}", e);
        let type_msg = format!("Error type: {}", std::any::type_name_of_val(&e));
        self.log_error(&error_msg);
        self.log_error(&type_msg);
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Failed to load saved albums: {}", e));
      }
    }
  }

  async fn get_followed_artists(&mut self, after: Option<String>) {
    self.log_error("DEBUG: Starting get_followed_artists");
    match self.spotify.current_user_followed_artists(after.as_deref(), Some(50)).await {
      Ok(cursor_page) => {
        self.log_error(&format!("SUCCESS: Got {} followed artists", cursor_page.items.len()));
        let mut app = self.app.lock().await;
        
        // Store the artists - saved_artists expects a CursorBasedPage
        app.library.saved_artists.add_pages(cursor_page.clone());
        
        // Also populate the artists vec for the UI
        app.artists = cursor_page.items.clone();
        
        app.add_log_message(format!("Loaded {} followed artists", cursor_page.items.len()));
      }
      Err(e) => {
        let error_msg = format!("DETAILED ERROR getting followed artists: {:?}", e);
        let type_msg = format!("Error type: {}", std::any::type_name_of_val(&e));
        self.log_error(&error_msg);
        self.log_error(&type_msg);
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Failed to load followed artists: {}", e));
      }
    }
  }

  async fn get_recently_played(&mut self) {
    self.log_error("DEBUG: Starting get_recently_played");
    
    // Get the last 50 recently played tracks
    match self.spotify.current_user_recently_played(Some(50), None).await {
      Ok(history) => {
        self.log_error(&format!("SUCCESS: Got {} recently played tracks", history.items.len()));
        let mut app = self.app.lock().await;
        
        // Store recently played in the app state
        app.recently_played.result = Some(history);
        
        let track_count = app.recently_played.result.as_ref().map(|h| h.items.len()).unwrap_or(0);
        app.add_log_message(format!("Loaded {} recently played tracks", track_count));
      }
      Err(e) => {
        let error_msg = format!("DETAILED ERROR getting recently played: {:?}", e);
        let type_msg = format!("Error type: {}", std::any::type_name_of_val(&e));
        self.log_error(&error_msg);
        self.log_error(&type_msg);
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Failed to load recently played tracks: {}", e));
      }
    }
  }

  async fn get_current_user_saved_shows(&mut self, _offset: Option<u32>) {
    self.log_error("DEBUG: Starting get_current_user_saved_shows");
    let mut app = self.app.lock().await;
    app.add_log_message("Podcasts feature requires additional work - the API returns a different Show type than expected".to_string());
    // TODO: The get_saved_show API returns Show, but the UI expects SimplifiedShow
    // This would require converting between the types or updating the UI
  }

  async fn get_top_tracks(&mut self) {
    self.log_error("DEBUG: Starting get_top_tracks");
    use rspotify::model::enums::TimeRange;
    
    // Get medium term (6 months) by default
    match self.spotify.current_user_top_tracks_manual(Some(TimeRange::MediumTerm), Some(50), Some(0)).await {
      Ok(page) => {
        self.log_error(&format!("SUCCESS: Got {} top tracks", page.items.len()));
        let mut app = self.app.lock().await;
        
        // Set the tracks directly to the track table
        app.track_table.tracks = page.items.clone();
        
        // Set context so the UI knows we're showing top tracks
        app.track_table.context = Some(TrackTableContext::SavedTracks); // Using SavedTracks context for now
        
        app.add_log_message(format!("Loaded {} top tracks (last 6 months)", page.items.len()));
      }
      Err(e) => {
        let error_msg = format!("DETAILED ERROR getting top tracks: {:?}", e);
        let type_msg = format!("Error type: {}", std::any::type_name_of_val(&e));
        self.log_error(&error_msg);
        self.log_error(&type_msg);
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Failed to load top tracks: {}", e));
      }
    }
  }

  async fn get_top_artists(&mut self) {
    self.log_error("DEBUG: Starting get_top_artists");
    use rspotify::model::enums::TimeRange;
    
    // Get medium term (6 months) by default
    match self.spotify.current_user_top_artists_manual(Some(TimeRange::MediumTerm), Some(50), Some(0)).await {
      Ok(page) => {
        self.log_error(&format!("SUCCESS: Got {} top artists", page.items.len()));
        let mut app = self.app.lock().await;
        
        // Set the artists directly
        app.artists = page.items.clone();
        
        app.add_log_message(format!("Loaded {} top artists (last 6 months)", page.items.len()));
      }
      Err(e) => {
        let error_msg = format!("DETAILED ERROR getting top artists: {:?}", e);
        let type_msg = format!("Error type: {}", std::any::type_name_of_val(&e));
        self.log_error(&error_msg);
        self.log_error(&type_msg);
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Failed to load top artists: {}", e));
      }
    }
  }

  async fn get_artist(&mut self, artist_id: String) {
    self.log_error(&format!("DEBUG: Starting get_artist for ID: {}", artist_id));
    use rspotify::model::ArtistId;
    use futures::{StreamExt, TryStreamExt};
    
    // Parse the artist ID from Spotify URI format if needed
    let artist_id_str = if artist_id.starts_with("spotify:artist:") {
      artist_id.replace("spotify:artist:", "")
    } else {
      artist_id
    };
    
    // Create ArtistId from string
    let artist_id = match ArtistId::from_id(&artist_id_str) {
      Ok(id) => id,
      Err(e) => {
        self.log_error(&format!("ERROR parsing artist ID: {:?}", e));
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Invalid artist ID: {}", e));
        return;
      }
    };
    
    match self.spotify.artist(artist_id.clone()).await {
      Ok(full_artist) => {
        self.log_error(&format!("SUCCESS: Got artist: {}", full_artist.name));
        
        // Get the artist's top tracks
        let top_tracks = match self.spotify.artist_top_tracks(artist_id.clone(), None).await {
          Ok(tracks) => {
            self.log_error(&format!("Got {} top tracks for artist", tracks.len()));
            tracks
          }
          Err(e) => {
            self.log_error(&format!("ERROR getting artist top tracks: {:?}", e));
            vec![]
          }
        };
        
        // Get the artist's albums using stream
        let albums_stream = self.spotify.artist_albums(artist_id.clone(), None, None);
        let albums_result: Result<Vec<_>, _> = albums_stream.take(50).try_collect().await;
        
        let albums = match albums_result {
          Ok(items) => {
            self.log_error(&format!("Got {} albums for artist", items.len()));
            let total = items.len() as u32; // Capture length before move
            Page {
              href: String::new(),
              items,
              limit: 50,
              next: None,
              offset: 0,
              previous: None,
              total,
            }
          }
          Err(e) => {
            self.log_error(&format!("ERROR getting artist albums: {:?}", e));
            Page {
              href: String::new(),
              items: vec![],
              limit: 50,
              next: None,
              offset: 0,
              previous: None,
              total: 0,
            }
          }
        };
        
        // Get related artists
        let related_artists = match self.spotify.artist_related_artists(artist_id).await {
          Ok(artists) => {
            self.log_error(&format!("Got {} related artists", artists.len()));
            artists
          }
          Err(e) => {
            self.log_error(&format!("ERROR getting related artists: {:?}", e));
            vec![]
          }
        };
        
        let mut app = self.app.lock().await;
        
        // Create the Artist struct
        let artist_data = Artist {
          artist_name: full_artist.name.clone(),
          albums,
          related_artists,
          top_tracks,
          selected_album_index: 0,
          selected_related_artist_index: 0,
          selected_top_track_index: 0,
          artist_hovered_block: ArtistBlock::TopTracks,
          artist_selected_block: ArtistBlock::Empty,
        };
        
        app.artist = Some(artist_data);
        app.add_log_message(format!("Loaded artist: {}", full_artist.name));
      }
      Err(e) => {
        self.log_error(&format!("ERROR getting artist: {:?}", e));
        let mut app = self.app.lock().await;
        app.handle_error(anyhow::anyhow!("Failed to load artist: {}", e));
      }
    }
  }

  async fn fetch_album_art(&mut self, url: String) {
    let mut app = self.app.lock().await;
    
    // Get idle mode state before borrowing manager
    let is_idle = app.is_idle_mode;
    
    if let Some(manager) = &mut app.album_art_manager {
      // Use different sizes based on idle mode
      // For idle mode, fetch larger size for better quality when scaling
      // For normal mode, also fetch larger size since we're scaling it up in the playbar
      let size = if is_idle { 256 } else { 64 };
      
      match manager.get_album_art(&url, size).await {
        Ok(art) => {
          app.current_album_art = Some(art);
          app.add_log_message(format!("Successfully fetched album art ({}x{}) from: {}", size, size, url));
        }
        Err(e) => {
          app.add_log_message(format!("Failed to fetch album art: {}", e));
          // Use placeholder art on failure
          app.current_album_art = Some(crate::album_art::AlbumArtManager::get_placeholder_art(size));
        }
      }
    }
  }
}