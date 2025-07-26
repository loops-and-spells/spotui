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
    album::SimplifiedAlbum,
    artist::FullArtist,
    page::Page,
    playlist::SimplifiedPlaylist,
    show::SimplifiedShow,
    track::FullTrack,
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
  GetMadeForYouPlaylistTracks(String, u32),
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
      IoEvent::GetMadeForYouPlaylistTracks(playlist_id, offset) => {
        // TODO: Implement get made for you playlist tracks
        // TODO: Implement GetMadeForYouPlaylistTracks
      }
      IoEvent::GetShowEpisodes(show) => {
        // TODO: Implement get show episodes
        // TODO: Implement GetShowEpisodes
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
            
            self.spotify.start_context_playback(context, None, offset, None).await
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
            
            self.spotify.start_context_playback(context, None, offset, None).await
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
            self.spotify.start_uris_playback(track_ids, None, None, None).await
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
      self.spotify.resume_playback(None, None).await
    };
    
    match result {
      Ok(_) => {
        self.log_error("SUCCESS: Started playback");
        let mut app = self.app.lock().await;
        app.add_log_message("Playback started".to_string());
        // Playback started - already logged
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
    match self.spotify.pause_playback(None).await {
      Ok(_) => {
        let mut app = self.app.lock().await;
        app.add_log_message("Playback paused".to_string());
        // Playback paused - already logged
      },
      Err(e) => {
        let error_msg = format!("{:?}", e);
        if error_msg.contains("status: 403") {
          let mut app = self.app.lock().await;
          app.add_log_message("Spotify Premium required for pause control".to_string());
          app.handle_error(anyhow::anyhow!("Spotify Premium required for playback controls"));
        } else {
          let mut app = self.app.lock().await;
          app.add_log_message(format!("Pause error: {}", e));
          app.handle_error(anyhow::anyhow!("Error pausing playback: {}", e));
        }
      }
    }
  }

  async fn next_track(&mut self) {
    match self.spotify.next_track(None).await {
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
    match self.spotify.previous_track(None).await {
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
    match self.spotify.seek_track(duration, None).await {
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
    match self.spotify.shuffle(state, None).await {
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
    match self.spotify.repeat(spotify_state, None).await {
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
        
        if let Err(e) = self.spotify.write_token_cache().await {
          // Warning: Failed to update token cache
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
        
        // For now, just set the tracks directly to the track table
        app.track_table.tracks = saved_tracks.iter().map(|saved_track| {
          saved_track.track.clone()
        }).collect();
        
        // Set context so the UI knows we're showing saved tracks
        app.track_table.context = Some(TrackTableContext::SavedTracks);
        
        app.add_log_message(format!("Loaded {} liked songs", saved_tracks.len()));
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
}