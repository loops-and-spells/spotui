use super::user_config::UserConfig;
use crate::network::IoEvent;
use crate::focus_manager::{FocusManager, ComponentId, FocusState};
use crate::album_art::{AlbumArtManager, PixelatedAlbumArt};
use rspotify::model::PlayableItem;
use anyhow::anyhow;
use rspotify::{
  model::{
    album::{FullAlbum, SavedAlbum, SimplifiedAlbum},
    artist::FullArtist,
    audio::AudioAnalysis,
    context::CurrentPlaybackContext,
    device::DevicePayload,
    page::{CursorBasedPage, Page},
    playing::PlayHistory,
    playlist::{PlaylistTracksRef, SimplifiedPlaylist},
    show::{FullShow, Show, SimplifiedEpisode, SimplifiedShow},
    track::{FullTrack, SavedTrack, SimplifiedTrack},
    user::PrivateUser,
    // PlaylistItem,  // Using network::PlayingItem instead
  },
  model::enums::Country,
};
use std::str::FromStr;
use std::sync::mpsc::Sender;
use std::{
  cmp::{max, min},
  collections::HashSet,
  time::{Instant, SystemTime},
};
use ratatui::layout::Rect;

use arboard::Clipboard;

pub const LIBRARY_OPTIONS: [&str; 7] = [
  "Recently Played",
  "Liked Songs",
  "Albums",
  "Artists",
  "Podcasts",
  "Top Tracks",
  "Top Artists",
];

const DEFAULT_ROUTE: Route = Route {
  id: RouteId::Home,
  active_block: ActiveBlock::Empty,
  hovered_block: ActiveBlock::Library,
};

#[derive(Clone)]
pub struct ScrollableResultPages<T> {
  index: usize,
  pub pages: Vec<T>,
}

impl<T> ScrollableResultPages<T> {
  pub fn new() -> ScrollableResultPages<T> {
    ScrollableResultPages {
      index: 0,
      pages: vec![],
    }
  }

  pub fn get_results(&self, at_index: Option<usize>) -> Option<&T> {
    self.pages.get(at_index.unwrap_or(self.index))
  }

  pub fn get_mut_results(&mut self, at_index: Option<usize>) -> Option<&mut T> {
    self.pages.get_mut(at_index.unwrap_or(self.index))
  }

  pub fn add_pages(&mut self, new_pages: T) {
    self.pages.push(new_pages);
    // Whenever a new page is added, set the active index to the end of the vector
    self.index = self.pages.len() - 1;
  }
}

#[derive(Default)]
pub struct SpotifyResultAndSelectedIndex<T> {
  pub index: usize,
  pub result: T,
}

#[derive(Clone)]
pub struct Library {
  pub selected_index: usize,
  pub saved_tracks: ScrollableResultPages<Page<SavedTrack>>,
  pub saved_albums: ScrollableResultPages<Page<SavedAlbum>>,
  pub saved_shows: ScrollableResultPages<Page<SimplifiedShow>>,
  pub saved_artists: ScrollableResultPages<CursorBasedPage<FullArtist>>,
  pub show_episodes: ScrollableResultPages<Page<SimplifiedEpisode>>,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum SearchResultBlock {
  AlbumSearch,
  SongSearch,
  ArtistSearch,
  PlaylistSearch,
  ShowSearch,
  Empty,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum ArtistBlock {
  TopTracks,
  Albums,
  RelatedArtists,
  Empty,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum DialogContext {
  #[default]
  PlaylistWindow,
  PlaylistSearch,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
  Analysis,
  PlayBar,
  AlbumTracks,
  AlbumList,
  ArtistBlock,
  Empty,
  Error,
  Home,
  Input,
  Library,
  MyPlaylists,
  Podcasts,
  EpisodeTable,
  RecentlyPlayed,
  SearchResultBlock,
  SelectDevice,
  TrackTable,
  Artists,
  BasicView,
  LogStream,
  Dialog(DialogContext),
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
  Analysis,
  AlbumTracks,
  AlbumList,
  Artist,
  BasicView,
  Error,
  Home,
  RecentlyPlayed,
  Search,
  SelectedDevice,
  TrackTable,
  Artists,
  Podcasts,
  PodcastEpisodes,
  Recommendations,
  LogStream,
  Dialog,
}

#[derive(Debug)]
pub struct Route {
  pub id: RouteId,
  pub active_block: ActiveBlock,
  pub hovered_block: ActiveBlock,
}

// Is it possible to compose enums?
#[derive(PartialEq, Debug)]
#[derive(Clone)]
pub enum TrackTableContext {
  MyPlaylists,
  AlbumSearch,
  PlaylistSearch,
  SavedTracks,
  RecommendedTracks,
}

// Is it possible to compose enums?
#[derive(Clone, PartialEq, Debug, Copy)]
pub enum AlbumTableContext {
  Simplified,
  Full,
}

#[derive(Clone, PartialEq, Debug, Copy)]
pub enum EpisodeTableContext {
  Simplified,
  Full,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RecommendationsContext {
  Artist,
  Song,
}

pub struct SearchResult {
  pub albums: Option<Page<SimplifiedAlbum>>,
  pub artists: Option<Page<FullArtist>>,
  pub playlists: Option<Page<SimplifiedPlaylist>>,
  pub tracks: Option<Page<FullTrack>>,
  pub shows: Option<Page<SimplifiedShow>>,
  pub selected_album_index: Option<usize>,
  pub selected_artists_index: Option<usize>,
  pub selected_playlists_index: Option<usize>,
  pub selected_tracks_index: Option<usize>,
  pub selected_shows_index: Option<usize>,
  pub hovered_block: SearchResultBlock,
  pub selected_block: SearchResultBlock,
}

#[derive(Default)]
pub struct TrackTable {
  pub tracks: Vec<FullTrack>,
  pub selected_index: usize,
  pub context: Option<TrackTableContext>,
}

#[derive(Clone)]
pub struct SelectedShow {
  pub show: SimplifiedShow,
}

#[derive(Clone)]
pub struct SelectedFullShow {
  pub show: FullShow,
}

#[derive(Clone)]
pub struct SelectedAlbum {
  pub album: SimplifiedAlbum,
  pub tracks: Page<SimplifiedTrack>,
  pub selected_index: usize,
}

#[derive(Clone)]
pub struct SelectedFullAlbum {
  pub album: FullAlbum,
  pub selected_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IdleAnimation {
  SpinningRecord,
  CoinFlip,
}

#[derive(Clone)]
pub struct Artist {
  pub artist_name: String,
  pub albums: Page<SimplifiedAlbum>,
  pub related_artists: Vec<FullArtist>,
  pub top_tracks: Vec<FullTrack>,
  pub selected_album_index: usize,
  pub selected_related_artist_index: usize,
  pub selected_top_track_index: usize,
  pub artist_hovered_block: ArtistBlock,
  pub artist_selected_block: ArtistBlock,
}

pub struct App {
  pub instant_since_last_current_playback_poll: Instant,
  pub instant_since_last_playback_toggle: Instant,
  pub instant_since_last_device_poll: Instant,
  navigation_stack: Vec<Route>,
  pub audio_analysis: Option<AudioAnalysis>,
  pub home_scroll: u16,
  pub user_config: UserConfig,
  pub artists: Vec<FullArtist>,
  pub artist: Option<Artist>,
  pub album_table_context: AlbumTableContext,
  pub saved_album_tracks_index: usize,
  pub api_error: String,
  pub current_playback_context: Option<CurrentPlaybackContext>,
  pub devices: Option<DevicePayload>,
  // Inputs:
  // input is the string for input;
  // input_idx is the index of the cursor in terms of character;
  // input_cursor_position is the sum of the width of characters preceding the cursor.
  // Reason for this complication is due to non-ASCII characters, they may
  // take more than 1 bytes to store and more than 1 character width to display.
  pub input: Vec<char>,
  pub input_idx: usize,
  pub input_cursor_position: u16,
  pub liked_song_ids_set: HashSet<String>,
  pub followed_artist_ids_set: HashSet<String>,
  pub saved_album_ids_set: HashSet<String>,
  pub saved_show_ids_set: HashSet<String>,
  pub large_search_limit: u32,
  pub library: Library,
  pub playlist_offset: u32,
  // Placeholder types for compilation - TODO: Fix with proper rspotify 0.15 types
  pub playlist_tracks: Option<()>,
  pub playlists: Option<Page<SimplifiedPlaylist>>,
  pub recently_played: SpotifyResultAndSelectedIndex<Option<CursorBasedPage<PlayHistory>>>,
  pub recommended_tracks: Vec<FullTrack>,
  pub recommendations_seed: String,
  pub recommendations_context: Option<RecommendationsContext>,
  pub search_results: SearchResult,
  pub selected_album_simplified: Option<SelectedAlbum>,
  pub selected_album_full: Option<SelectedFullAlbum>,
  pub selected_device_index: Option<usize>,
  pub selected_playlist_index: Option<usize>,
  pub active_playlist_index: Option<usize>,
  pub size: Rect,
  pub last_resize_time: Instant,
  pub small_search_limit: u32,
  pub song_progress_ms: u128,
  pub seek_ms: Option<u128>,
  pub track_table: TrackTable,
  pub episode_table_context: EpisodeTableContext,
  pub selected_show_simplified: Option<SelectedShow>,
  pub selected_show_full: Option<SelectedFullShow>,
  pub user: Option<PrivateUser>,
  pub album_list_index: usize,
  pub artists_list_index: usize,
  pub clipboard: Option<Clipboard>,
  pub shows_list_index: usize,
  pub episode_list_index: usize,
  pub is_loading: bool,
  io_tx: Option<Sender<IoEvent>>,
  pub is_fetching_current_playback: bool,
  pub spotify_token_expiry: SystemTime,
  pub dialog: Option<String>,
  pub confirm: bool,
  pub log_messages: Vec<String>,
  pub log_stream_selected_index: usize,
  pub log_stream_scroll_offset: usize,
  pub focus_manager: FocusManager,
  pub album_art_manager: Option<AlbumArtManager>,
  pub current_album_art: Option<PixelatedAlbumArt>,
  pub current_album_art_url: Option<String>,
  pub last_user_interaction: Instant,
  pub is_idle_mode: bool,
  pub idle_animation: IdleAnimation,
}

impl Default for App {
  fn default() -> Self {
    App {
      audio_analysis: None,
      album_table_context: AlbumTableContext::Full,
      album_list_index: 0,
      artists_list_index: 0,
      shows_list_index: 0,
      episode_list_index: 0,
      artists: vec![],
      artist: None,
      user_config: UserConfig::new(),
      saved_album_tracks_index: 0,
      recently_played: Default::default(),
      size: Rect::default(),
      last_resize_time: Instant::now(),
      selected_album_simplified: None,
      selected_album_full: None,
      home_scroll: 0,
      library: Library {
        saved_tracks: ScrollableResultPages::new(),
        saved_albums: ScrollableResultPages::new(),
        saved_shows: ScrollableResultPages::new(),
        saved_artists: ScrollableResultPages::new(),
        show_episodes: ScrollableResultPages::new(),
        selected_index: 0,
      },
      liked_song_ids_set: HashSet::new(),
      followed_artist_ids_set: HashSet::new(),
      saved_album_ids_set: HashSet::new(),
      saved_show_ids_set: HashSet::new(),
      navigation_stack: vec![DEFAULT_ROUTE],
      large_search_limit: 20,
      small_search_limit: 4,
      api_error: String::new(),
      current_playback_context: None,
      devices: None,
      input: vec![],
      input_idx: 0,
      input_cursor_position: 0,
      playlist_offset: 0,
      playlist_tracks: None,
      playlists: None,
      recommended_tracks: vec![],
      recommendations_context: None,
      recommendations_seed: "".to_string(),
      search_results: SearchResult {
        hovered_block: SearchResultBlock::SongSearch,
        selected_block: SearchResultBlock::Empty,
        albums: None,
        artists: None,
        playlists: None,
        shows: None,
        selected_album_index: None,
        selected_artists_index: None,
        selected_playlists_index: None,
        selected_tracks_index: None,
        selected_shows_index: None,
        tracks: None,
      },
      song_progress_ms: 0,
      seek_ms: None,
      selected_device_index: None,
      selected_playlist_index: None,
      active_playlist_index: None,
      track_table: Default::default(),
      episode_table_context: EpisodeTableContext::Full,
      selected_show_simplified: None,
      selected_show_full: None,
      user: None,
      instant_since_last_current_playback_poll: Instant::now(),
      instant_since_last_playback_toggle: Instant::now(),
      instant_since_last_device_poll: Instant::now(),
      clipboard: Clipboard::new().ok(),
      is_loading: false,
      io_tx: None,
      is_fetching_current_playback: false,
      spotify_token_expiry: SystemTime::now(),
      dialog: None,
      confirm: false,
      log_messages: Vec::new(),
      log_stream_selected_index: 0,
      log_stream_scroll_offset: 0,
      focus_manager: FocusManager::new(),
      album_art_manager: AlbumArtManager::new().ok(),
      current_album_art: None,
      current_album_art_url: None,
      last_user_interaction: Instant::now(),
      is_idle_mode: false,
      idle_animation: IdleAnimation::SpinningRecord,
    }
  }
}

impl App {
  pub fn new(
    io_tx: Sender<IoEvent>,
    user_config: UserConfig,
    spotify_token_expiry: SystemTime,
  ) -> App {
    App {
      io_tx: Some(io_tx),
      user_config,
      spotify_token_expiry,
      ..App::default()
    }
  }

  // Send a network event to the network thread
  pub fn dispatch(&mut self, action: IoEvent) {
    if let Some(io_tx) = &self.io_tx {
      if let Err(e) = io_tx.send(action) {
        self.handle_error(anyhow::anyhow!("Failed to dispatch event: {}", e));
      };
    }
  }

  fn apply_seek(&mut self, seek_ms: u32) {
    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = &self.current_playback_context
    {
      let duration_ms = match item {
        PlayableItem::Track(track) => track.duration.num_milliseconds() as u32,
        PlayableItem::Episode(episode) => episode.duration.num_milliseconds() as u32,
      };

      let event = if seek_ms < duration_ms {
        IoEvent::Seek(seek_ms)
      } else {
        IoEvent::NextTrack
      };

      self.dispatch(event);
    }
  }

  fn poll_current_playback(&mut self) {
    // Poll every 5 seconds
    let poll_interval_ms = 5_000;

    let elapsed = self
      .instant_since_last_current_playback_poll
      .elapsed()
      .as_millis();

    if !self.is_fetching_current_playback && elapsed >= poll_interval_ms {
      self.is_fetching_current_playback = true;
      // Trigger the seek if the user has set a new position
      match self.seek_ms {
        Some(seek_ms) => self.apply_seek(seek_ms as u32),
        None => self.dispatch(IoEvent::GetCurrentPlayback),
      }
    }
  }

  pub fn update_on_tick(&mut self) {
    self.poll_current_playback();
    
    // Poll devices every 30 seconds
    let device_poll_interval_ms = 30_000;
    let device_elapsed = self.instant_since_last_device_poll.elapsed().as_millis();
    
    if device_elapsed >= device_poll_interval_ms {
      self.dispatch(IoEvent::GetDevices);
      self.instant_since_last_device_poll = Instant::now();
    }
    if let Some(CurrentPlaybackContext {
      item: Some(item),
      is_playing,
      progress,
      ..
    }) = &self.current_playback_context
    {
      // Update progress even when the song is not playing,
      // because seeking is possible while paused
      let elapsed = if *is_playing {
        self
          .instant_since_last_current_playback_poll
          .elapsed()
          .as_millis()
      } else {
        0u128
      } + progress.map(|p| p.num_milliseconds() as u128).unwrap_or(0);

      let duration_ms = match item {
        PlayableItem::Track(track) => track.duration.num_milliseconds() as u32,
        PlayableItem::Episode(episode) => episode.duration.num_milliseconds() as u32,
      };

      if elapsed < u128::from(duration_ms) {
        self.song_progress_ms = elapsed;
      } else {
        self.song_progress_ms = duration_ms.into();
      }
    }
  }

  pub fn seek_forwards(&mut self) {
    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = &self.current_playback_context
    {
      let duration_ms = match item {
        PlayableItem::Track(track) => track.duration.num_milliseconds() as u32,
        PlayableItem::Episode(episode) => episode.duration.num_milliseconds() as u32,
      };

      let old_progress = match self.seek_ms {
        Some(seek_ms) => seek_ms,
        None => self.song_progress_ms,
      };

      let new_progress = min(
        old_progress as u32 + self.user_config.behavior.seek_milliseconds,
        duration_ms,
      );

      self.seek_ms = Some(new_progress as u128);
    }
  }

  pub fn seek_backwards(&mut self) {
    let old_progress = match self.seek_ms {
      Some(seek_ms) => seek_ms,
      None => self.song_progress_ms,
    };
    let new_progress = if old_progress as u32 > self.user_config.behavior.seek_milliseconds {
      old_progress as u32 - self.user_config.behavior.seek_milliseconds
    } else {
      0u32
    };
    self.seek_ms = Some(new_progress as u128);
  }

  pub fn get_recommendations_for_seed(
    &mut self,
    seed_artists: Option<Vec<String>>,
    seed_tracks: Option<Vec<String>>,
    first_track: Option<FullTrack>,
  ) {
    let user_country = self.get_user_country();
    // self.dispatch(IoEvent::GetRecommendationsForSeed(
      // seed_artists,
      // seed_tracks,
      // Box::new(first_track),
      // user_country,
    // ));
  }

  pub fn get_recommendations_for_track_id(&mut self, id: String) {
    let user_country = self.get_user_country();
    // self.dispatch(IoEvent::GetRecommendationsForTrackId(id, user_country));
  }

  pub fn increase_volume(&mut self) {
    if let Some(context) = self.current_playback_context.clone() {
      let current_volume = context.device.volume_percent.unwrap_or(50) as u8;
      let next_volume = min(
        current_volume + self.user_config.behavior.volume_increment,
        100,
      );

      if next_volume != current_volume {
        self.dispatch(IoEvent::SetVolume(next_volume));
      }
    }
  }

  pub fn decrease_volume(&mut self) {
    if let Some(context) = self.current_playback_context.clone() {
      let current_volume = context.device.volume_percent.unwrap_or(50) as i8;
      let next_volume = max(
        current_volume - self.user_config.behavior.volume_increment as i8,
        0,
      );

      if next_volume != current_volume {
        self.dispatch(IoEvent::SetVolume(next_volume as u8));
      }
    }
  }

  pub fn handle_error(&mut self, e: anyhow::Error) {
    // Log the error to the log stream with ERROR prefix
    let error_message = format!("ERROR: {}", e);
    self.add_log_message(error_message);
    
    // Auto-open log stream when error occurs (only if not already viewing it)
    if self.get_current_route().active_block != ActiveBlock::LogStream {
      self.push_navigation_stack(RouteId::LogStream, ActiveBlock::LogStream);
    }
    
    // Clear api_error to prevent UI artifacts
    self.api_error = String::new();
  }

  pub fn add_log_message(&mut self, message: String) {
    let timestamp = chrono::Utc::now().format("%H:%M:%S");
    let formatted_message = format!("[{}] {}", timestamp, message);
    
    // Write to disk for debugging
    if let Ok(mut file) = std::fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open("/tmp/spotify-tui-log-stream.log") 
    {
      use std::io::Write;
      let _ = writeln!(file, "=== LOG MESSAGE ===");
      let _ = writeln!(file, "{}", formatted_message);
      let _ = writeln!(file, "Raw message: {:?}", message);
      let _ = writeln!(file, "Contains newlines: {}", message.contains('\n'));
      let _ = writeln!(file, "==================\n");
    }
    
    self.log_messages.push(formatted_message);
    
    // Keep only the last 100 messages to prevent memory issues
    if self.log_messages.len() > 100 {
      self.log_messages.remove(0);
      // Adjust selection index when removing messages from the beginning
      if self.log_stream_selected_index > 0 {
        self.log_stream_selected_index -= 1;
      }
      if self.log_stream_scroll_offset > 0 {
        self.log_stream_scroll_offset -= 1;
      }
    }
    
    // If we're not actively viewing the log stream, auto-scroll to show latest messages
    if self.get_current_route().active_block != ActiveBlock::LogStream {
      self.log_stream_selected_index = self.log_messages.len().saturating_sub(1);
      let visible_height = 10; // Default visible height
      self.log_stream_scroll_offset = self.log_messages.len().saturating_sub(visible_height);
    }
  }

  pub fn toggle_playback(&mut self) {
    // Add a cooldown to prevent rapid toggling
    let elapsed = self.instant_since_last_playback_toggle.elapsed().as_millis();
    if elapsed < 500 { // 500ms cooldown
      return;
    }
    
    self.instant_since_last_playback_toggle = Instant::now();
    
    if let Some(CurrentPlaybackContext {
      is_playing: true, ..
    }) = &self.current_playback_context
    {
      self.dispatch(IoEvent::PausePlayback);
    } else {
      // When no offset or uris are passed, spotify will resume current playback
      self.dispatch(IoEvent::StartPlayback(None, None));
    }
  }

  pub fn previous_track(&mut self) {
    if self.song_progress_ms >= 3_000 {
      self.dispatch(IoEvent::Seek(0));
    } else {
      self.dispatch(IoEvent::PreviousTrack);
    }
  }

  // The navigation_stack actually only controls the large block to the right of `library` and
  // `playlists`
  pub fn push_navigation_stack(&mut self, next_route_id: RouteId, next_active_block: ActiveBlock) {
    if !self
      .navigation_stack
      .last()
      .map(|last_route| last_route.id == next_route_id)
      .unwrap_or(false)
    {
      self.add_log_message(format!("Pushing to navigation stack: {:?} / {:?}", next_route_id, next_active_block));
      self.navigation_stack.push(Route {
        id: next_route_id,
        active_block: next_active_block,
        hovered_block: next_active_block,
      });
      self.add_log_message(format!("Navigation stack after push: {:?}", 
        self.navigation_stack.iter().map(|r| format!("{:?}", r.active_block)).collect::<Vec<_>>()));
    }
  }

  pub fn pop_navigation_stack(&mut self) -> Option<Route> {
    self.add_log_message(format!("Popping navigation stack. Current size: {}", self.navigation_stack.len()));
    if self.navigation_stack.len() == 1 {
      None
    } else {
      let popped = self.navigation_stack.pop();
      self.add_log_message(format!("Navigation stack after pop: {:?}", 
        self.navigation_stack.iter().map(|r| format!("{:?}", r.active_block)).collect::<Vec<_>>()));
      popped
    }
  }

  pub fn clear_navigation_stack(&mut self) {
    self.add_log_message("Clearing navigation stack to return to root".to_string());
    self.navigation_stack.clear();
    self.navigation_stack.push(DEFAULT_ROUTE);
  }

  pub fn get_current_route(&self) -> &Route {
    // if for some reason there is no route return the default
    self.navigation_stack.last().unwrap_or(&DEFAULT_ROUTE)
  }

  pub fn get_navigation_breadcrumb(&self) -> String {
    let mut breadcrumb_parts = Vec::new();
    
    for route in &self.navigation_stack {
      let part = match route.id {
        RouteId::Home => "Library",
        RouteId::TrackTable => {
          match self.track_table.context.as_ref() {
            Some(TrackTableContext::MyPlaylists) => {
              if let Some(selected_playlist_index) = self.selected_playlist_index {
                if let Some(playlists) = &self.playlists {
                  playlists.items.get(selected_playlist_index)
                    .map(|p| p.name.as_str())
                    .unwrap_or("Playlist")
                } else {
                  "Playlist"
                }
              } else {
                "Tracks"
              }
            }
            Some(TrackTableContext::SavedTracks) => "Liked Songs",
            Some(TrackTableContext::RecommendedTracks) => "Recommended",
            Some(TrackTableContext::AlbumSearch) => "Album",
            Some(TrackTableContext::PlaylistSearch) => "Search Results",
            None => "Tracks",
          }
        }
        RouteId::AlbumTracks => "Album",
        RouteId::AlbumList => "Albums",
        RouteId::Artist => {
          if let Some(artist) = &self.artist {
            &artist.artist_name
          } else {
            "Artist"
          }
        }
        RouteId::RecentlyPlayed => "Recently Played",
        RouteId::Search => "Search",
        RouteId::Artists => "Artists",
        RouteId::Podcasts => "Podcasts",
        RouteId::PodcastEpisodes => "Episodes",
        RouteId::Recommendations => "Recommendations",
        RouteId::Analysis => "Audio Analysis",
        RouteId::BasicView => "Basic View",
        RouteId::LogStream => "Log Stream",
        RouteId::SelectedDevice => "Devices",
        RouteId::Error => "Error",
        RouteId::Dialog => "Dialog",
      };
      breadcrumb_parts.push(part.to_string());
    }
    
    breadcrumb_parts.join(" > ")
  }

  fn get_current_route_mut(&mut self) -> &mut Route {
    self.navigation_stack.last_mut().unwrap()
  }

  pub fn set_current_route_state(
    &mut self,
    active_block: Option<ActiveBlock>,
    hovered_block: Option<ActiveBlock>,
  ) {
    let mut current_route = self.get_current_route_mut();
    if let Some(active_block) = active_block {
      current_route.active_block = active_block;
    }
    if let Some(hovered_block) = hovered_block {
      current_route.hovered_block = hovered_block;
    }
  }

  pub fn copy_song_url(&mut self) {
    let clipboard = match &mut self.clipboard {
      Some(ctx) => ctx,
      None => return,
    };

    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = &self.current_playback_context
    {
      match item {
        PlayableItem::Track(track) => {
          if let Err(e) = clipboard.set_text(format!(
            "https://open.spotify.com/track/{}",
            track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string())
          )) {
            self.handle_error(anyhow!("failed to set clipboard content: {}", e));
          }
        }
        PlayableItem::Episode(episode) => {
          if let Err(e) = clipboard.set_text(format!(
            "https://open.spotify.com/episode/{}",
            episode.id.to_owned()
          )) {
            self.handle_error(anyhow!("failed to set clipboard content: {}", e));
          }
        }
      }
    }
  }

  pub fn copy_album_url(&mut self) {
    let clipboard = match &mut self.clipboard {
      Some(ctx) => ctx,
      None => return,
    };

    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = &self.current_playback_context
    {
      match item {
        PlayableItem::Track(track) => {
          if let Err(e) = clipboard.set_text(format!(
            "https://open.spotify.com/album/{}",
            track.album.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string())
          )) {
            self.handle_error(anyhow!("failed to set clipboard content: {}", e));
          }
        }
        PlayableItem::Episode(episode) => {
          if let Err(e) = clipboard.set_text(format!(
            "https://open.spotify.com/show/{}",
            episode.show.id.to_owned()
          )) {
            self.handle_error(anyhow!("failed to set clipboard content: {}", e));
          }
        }
      }
    }
  }

  pub fn set_saved_tracks_to_table(&mut self, saved_track_page: &Page<SavedTrack>) {
    // self.dispatch(IoEvent::SetTracksToTable(
    //   saved_track_page
    //     .items
    //     .clone()
    //     .into_iter()
    //     .map(|item| item.track)
    //     .collect::<Vec<FullTrack>>(),
    // ));
  }

  pub fn set_saved_artists_to_table(&mut self, saved_artists_page: &CursorBasedPage<FullArtist>) {
    // self.dispatch(IoEvent::SetArtistsToTable(
    //   saved_artists_page
    //     .items
    //     .clone()
    //     .into_iter()
    //     .collect::<Vec<FullArtist>>(),
    // ));
  }

  pub fn get_current_user_saved_artists_next(&mut self) {
    match self
      .library
      .saved_artists
      .get_results(Some(self.library.saved_artists.index + 1))
      .cloned()
    {
      Some(saved_artists) => {
        self.set_saved_artists_to_table(&saved_artists);
        self.library.saved_artists.index += 1
      }
      None => {
        if let Some(saved_artists) = &self.library.saved_artists.clone().get_results(None) {
          if let Some(last_artist) = saved_artists.items.last() {
            // self.dispatch(IoEvent::GetFollowedArtists(Some(last_artist.id.to_string()));
          }
        }
      }
    }
  }

  pub fn get_current_user_saved_artists_previous(&mut self) {
    if self.library.saved_artists.index > 0 {
      self.library.saved_artists.index -= 1;
    }

    if let Some(saved_artists) = &self.library.saved_artists.get_results(None).cloned() {
      self.set_saved_artists_to_table(saved_artists);
    }
  }

  pub fn get_current_user_saved_tracks_next(&mut self) {
    // Before fetching the next tracks, check if we have already fetched them
    match self
      .library
      .saved_tracks
      .get_results(Some(self.library.saved_tracks.index + 1))
      .cloned()
    {
      Some(saved_tracks) => {
        self.set_saved_tracks_to_table(&saved_tracks);
        self.library.saved_tracks.index += 1
      }
      None => {
        if let Some(saved_tracks) = &self.library.saved_tracks.get_results(None) {
          let offset = Some(saved_tracks.offset + saved_tracks.limit);
          // self.dispatch(IoEvent::GetCurrentSavedTracks(offset);
        }
      }
    }
  }

  pub fn get_current_user_saved_tracks_previous(&mut self) {
    if self.library.saved_tracks.index > 0 {
      self.library.saved_tracks.index -= 1;
    }

    if let Some(saved_tracks) = &self.library.saved_tracks.get_results(None).cloned() {
      self.set_saved_tracks_to_table(saved_tracks);
    }
  }

  pub fn shuffle(&mut self) {
    if let Some(context) = &self.current_playback_context.clone() {
      self.dispatch(IoEvent::Shuffle(context.shuffle_state));
    };
  }

  pub fn get_current_user_saved_albums_next(&mut self) {
    match self
      .library
      .saved_albums
      .get_results(Some(self.library.saved_albums.index + 1))
      .cloned()
    {
      Some(_) => self.library.saved_albums.index += 1,
      None => {
        if let Some(saved_albums) = &self.library.saved_albums.get_results(None) {
          let offset = Some(saved_albums.offset + saved_albums.limit);
          // self.dispatch(IoEvent::GetCurrentUserSavedAlbums(offset);
        }
      }
    }
  }

  pub fn get_current_user_saved_albums_previous(&mut self) {
    if self.library.saved_albums.index > 0 {
      self.library.saved_albums.index -= 1;
    }
  }

  pub fn current_user_saved_album_delete(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::SearchResultBlock => {
        if let Some(albums) = &self.search_results.albums {
          if let Some(selected_index) = self.search_results.selected_album_index {
            let selected_album = &albums.items[selected_index];
            if let Some(album_id) = selected_album.id.as_ref().map(|id| id.to_string()) {
              // self.dispatch(IoEvent::CurrentUserSavedAlbumDelete(album_id);
            }
          }
        }
      }
      ActiveBlock::AlbumList => {
        if let Some(albums) = self.library.saved_albums.get_results(None) {
          if let Some(selected_album) = albums.items.get(self.album_list_index) {
            let album_id = selected_album.album.id.to_string();
            // self.dispatch(IoEvent::CurrentUserSavedAlbumDelete(album_id);
          }
        }
      }
      ActiveBlock::ArtistBlock => {
        if let Some(artist) = &self.artist {
          if let Some(selected_album) = artist.albums.items.get(artist.selected_album_index) {
            if let Some(album_id) = selected_album.id.as_ref().map(|id| id.to_string()) {
              // self.dispatch(IoEvent::CurrentUserSavedAlbumDelete(album_id);
            }
          }
        }
      }
      _ => (),
    }
  }

  pub fn current_user_saved_album_add(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::SearchResultBlock => {
        if let Some(albums) = &self.search_results.albums {
          if let Some(selected_index) = self.search_results.selected_album_index {
            let selected_album = &albums.items[selected_index];
            if let Some(album_id) = selected_album.id.as_ref().map(|id| id.to_string()) {
              // self.dispatch(IoEvent::CurrentUserSavedAlbumAdd(album_id);
            }
          }
        }
      }
      ActiveBlock::ArtistBlock => {
        if let Some(artist) = &self.artist {
          if let Some(selected_album) = artist.albums.items.get(artist.selected_album_index) {
            if let Some(album_id) = selected_album.id.as_ref().map(|id| id.to_string()) {
              // self.dispatch(IoEvent::CurrentUserSavedAlbumAdd(album_id);
            }
          }
        }
      }
      _ => (),
    }
  }

  pub fn get_current_user_saved_shows_next(&mut self) {
    match self
      .library
      .saved_shows
      .get_results(Some(self.library.saved_shows.index + 1))
      .cloned()
    {
      Some(_) => self.library.saved_shows.index += 1,
      None => {
        if let Some(saved_shows) = &self.library.saved_shows.get_results(None) {
          let offset = Some(saved_shows.offset + saved_shows.limit);
          // self.dispatch(IoEvent::GetCurrentUserSavedShows(offset);
        }
      }
    }
  }

  pub fn get_current_user_saved_shows_previous(&mut self) {
    if self.library.saved_shows.index > 0 {
      self.library.saved_shows.index -= 1;
    }
  }

  pub fn get_episode_table_next(&mut self, show_id: String) {
    match self
      .library
      .show_episodes
      .get_results(Some(self.library.show_episodes.index + 1))
      .cloned()
    {
      Some(_) => self.library.show_episodes.index += 1,
      None => {
        if let Some(show_episodes) = &self.library.show_episodes.get_results(None) {
          let offset = Some(show_episodes.offset + show_episodes.limit);
          // self.dispatch(IoEvent::GetCurrentShowEpisodes(show_id, offset);
        }
      }
    }
  }

  pub fn get_episode_table_previous(&mut self) {
    if self.library.show_episodes.index > 0 {
      self.library.show_episodes.index -= 1;
    }
  }

  pub fn user_unfollow_artists(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::SearchResultBlock => {
        if let Some(artists) = &self.search_results.artists {
          if let Some(selected_index) = self.search_results.selected_artists_index {
            let selected_artist: &FullArtist = &artists.items[selected_index];
            let artist_id = selected_artist.id.to_string();
            // self.dispatch(IoEvent::UserUnfollowArtists(vec![artist_id]);
          }
        }
      }
      ActiveBlock::AlbumList => {
        if let Some(artists) = self.library.saved_artists.get_results(None) {
          if let Some(selected_artist) = artists.items.get(self.artists_list_index) {
            let artist_id = selected_artist.id.to_string();
            // self.dispatch(IoEvent::UserUnfollowArtists(vec![artist_id]);
          }
        }
      }
      ActiveBlock::ArtistBlock => {
        if let Some(artist) = &self.artist {
          let selected_artis = &artist.related_artists[artist.selected_related_artist_index];
          let artist_id = selected_artis.id.to_string();
          // self.dispatch(IoEvent::UserUnfollowArtists(vec![artist_id]);
        }
      }
      _ => (),
    };
  }

  pub fn user_follow_artists(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::SearchResultBlock => {
        if let Some(artists) = &self.search_results.artists {
          if let Some(selected_index) = self.search_results.selected_artists_index {
            let selected_artist: &FullArtist = &artists.items[selected_index];
            let artist_id = selected_artist.id.to_string();
            // self.dispatch(IoEvent::UserFollowArtists(vec![artist_id]);
          }
        }
      }
      ActiveBlock::ArtistBlock => {
        if let Some(artist) = &self.artist {
          let selected_artis = &artist.related_artists[artist.selected_related_artist_index];
          let artist_id = selected_artis.id.to_string();
          // self.dispatch(IoEvent::UserFollowArtists(vec![artist_id]);
        }
      }
      _ => (),
    }
  }

  pub fn user_follow_playlist(&mut self) {
    if let SearchResult {
      playlists: Some(ref playlists),
      selected_playlists_index: Some(selected_index),
      ..
    } = self.search_results
    {
      let selected_playlist: &SimplifiedPlaylist = &playlists.items[selected_index];
      let selected_id = selected_playlist.id.to_string();
      let selected_public = selected_playlist.public;
      let selected_owner_id = selected_playlist.owner.id.to_string();
      // self.dispatch(IoEvent::UserFollowPlaylist(
      //   selected_owner_id,
      //   selected_id,
      //   selected_public,
      // ));
    }
  }

  pub fn user_unfollow_playlist(&mut self) {
    if let (Some(playlists), Some(selected_index), Some(user)) =
      (&self.playlists, self.selected_playlist_index, &self.user)
    {
      let selected_playlist = &playlists.items[selected_index];
      let selected_id = selected_playlist.id.to_string();
      let user_id = user.id.clone();
      // self.dispatch(IoEvent::UserUnfollowPlaylist(user_id, selected_id))
    }
  }

  pub fn user_unfollow_playlist_search_result(&mut self) {
    if let (Some(playlists), Some(selected_index), Some(user)) = (
      &self.search_results.playlists,
      self.search_results.selected_playlists_index,
      &self.user,
    ) {
      let selected_playlist = &playlists.items[selected_index];
      let selected_id = selected_playlist.id.to_string();
      let user_id = user.id.clone();
      // self.dispatch(IoEvent::UserUnfollowPlaylist(user_id, selected_id))
    }
  }

  pub fn user_follow_show(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::SearchResultBlock => {
        if let Some(shows) = &self.search_results.shows {
          if let Some(selected_index) = self.search_results.selected_shows_index {
            if let Some(show_id) = shows.items.get(selected_index).map(|item| item.id.clone()) {
              // self.dispatch(IoEvent::CurrentUserSavedShowAdd(show_id));
            }
          }
        }
      }
      ActiveBlock::EpisodeTable => match self.episode_table_context {
        EpisodeTableContext::Full => {
          if let Some(selected_episode) = self.selected_show_full.clone() {
            let show_id = selected_episode.show.id;
            // self.dispatch(IoEvent::CurrentUserSavedShowAdd(show_id);
          }
        }
        EpisodeTableContext::Simplified => {
          if let Some(selected_episode) = self.selected_show_simplified.clone() {
            let show_id = selected_episode.show.id;
            // self.dispatch(IoEvent::CurrentUserSavedShowAdd(show_id);
          }
        }
      },
      _ => (),
    }
  }

  pub fn user_unfollow_show(&mut self, block: ActiveBlock) {
    match block {
      ActiveBlock::Podcasts => {
        if let Some(shows) = self.library.saved_shows.get_results(None) {
          if let Some(selected_show) = shows.items.get(self.shows_list_index) {
            let show_id = selected_show.id.clone();
            // self.dispatch(IoEvent::CurrentUserSavedShowDelete(show_id);
          }
        }
      }
      ActiveBlock::SearchResultBlock => {
        if let Some(shows) = &self.search_results.shows {
          if let Some(selected_index) = self.search_results.selected_shows_index {
            let show_id = shows.items[selected_index].id.to_owned();
            // self.dispatch(IoEvent::CurrentUserSavedShowDelete(show_id);
          }
        }
      }
      ActiveBlock::EpisodeTable => match self.episode_table_context {
        EpisodeTableContext::Full => {
          if let Some(selected_episode) = self.selected_show_full.clone() {
            let show_id = selected_episode.show.id;
            // self.dispatch(IoEvent::CurrentUserSavedShowDelete(show_id);
          }
        }
        EpisodeTableContext::Simplified => {
          if let Some(selected_episode) = self.selected_show_simplified.clone() {
            let show_id = selected_episode.show.id;
            // self.dispatch(IoEvent::CurrentUserSavedShowDelete(show_id);
          }
        }
      },
      _ => (),
    }
  }

  pub fn get_audio_analysis(&mut self) {
    if let Some(CurrentPlaybackContext {
      item: Some(item), ..
    }) = &self.current_playback_context
    {
      match item {
        PlayableItem::Track(track) => {
          if self.get_current_route().id != RouteId::Analysis {
            let uri = format!("spotify:track:{}", track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()));
            self.dispatch(IoEvent::GetAudioAnalysis(uri));
            self.push_navigation_stack(RouteId::Analysis, ActiveBlock::Analysis);
          }
        }
        PlayableItem::Episode(_episode) => {
          // No audio analysis available for podcast uris, so just default to the empty analysis
          // view to avoid a 400 error code
          self.push_navigation_stack(RouteId::Analysis, ActiveBlock::Analysis);
        }
      }
    }
  }

  pub fn repeat(&mut self) {
    if let Some(context) = &self.current_playback_context.clone() {
      self.dispatch(IoEvent::Repeat(context.repeat_state.into()));
    }
  }

  pub fn get_artist(&mut self, artist_id: String, input_artist_name: String) {
    let user_country = self.get_user_country();
    self.dispatch(IoEvent::GetArtist(artist_id));
  }

  pub fn get_user_country(&self) -> Option<Country> {
    self
      .user
      .to_owned()
      .and_then(|user| user.country)
  }

  // Focus Manager Methods
  
  /// Set focus to a component using the centralized focus manager
  pub fn focus_component(&mut self, component: ComponentId) {
    self.focus_manager.set_focus(component);
  }

  /// Set hover to a component using the centralized focus manager
  pub fn hover_component(&mut self, component: ComponentId) {
    self.focus_manager.set_hover(component);
  }

  /// Navigate to a component (sets hover, not focus)
  pub fn navigate_to_component(&mut self, component: ComponentId) {
    self.focus_manager.navigate_to(component);
  }

  /// Enter a component directly (sets both focus and hover)
  pub fn enter_component(&mut self, component: ComponentId) {
    self.focus_manager.enter_component(component);
  }

  /// Clear focus while keeping hover
  pub fn clear_focus(&mut self) {
    self.focus_manager.clear_focus();
  }

  /// Clear hover
  pub fn clear_hover(&mut self) {
    self.focus_manager.clear_hover();
  }

  /// Clear all focus states
  pub fn clear_all_focus(&mut self) {
    self.focus_manager.clear_all();
  }

  /// Get focus state of a component
  pub fn get_component_focus_state(&self, component: &ComponentId) -> FocusState {
    self.focus_manager.get_focus_state(component)
  }

  /// Check if component is focused
  pub fn is_component_focused(&self, component: &ComponentId) -> bool {
    self.focus_manager.is_focused(component)
  }

  /// Check if component is hovered
  pub fn is_component_hovered(&self, component: &ComponentId) -> bool {
    self.focus_manager.is_hovered(component)
  }

  /// Get currently focused component
  pub fn get_focused_component(&self) -> Option<&ComponentId> {
    self.focus_manager.get_focused()
  }

  /// Get currently hovered component
  pub fn get_hovered_component(&self) -> Option<&ComponentId> {
    self.focus_manager.get_hovered()
  }

  /// Update album art for current playing track
  pub fn update_album_art(&mut self) {
    if let Some(context) = &self.current_playback_context {
      if let Some(item) = &context.item {
        match item {
          PlayableItem::Track(track) => {
            // Get the smallest album image (we'll resize it anyway)
            if let Some(image) = track.album.images.iter().min_by_key(|img| img.width.unwrap_or(1000)) {
              // Only fetch if URL has changed
              if self.current_album_art_url.as_ref() != Some(&image.url) {
                self.current_album_art_url = Some(image.url.clone());
                // Dispatch an event to fetch the album art asynchronously
                self.dispatch(IoEvent::FetchAlbumArt(image.url.clone()));
              }
            }
          }
          PlayableItem::Episode(_) => {
            // Episodes might have show artwork
            self.current_album_art = None;
            self.current_album_art_url = None;
          }
        }
      }
    }
  }

  /// Reset idle timer on user interaction
  pub fn reset_idle_timer(&mut self) {
    self.last_user_interaction = Instant::now();
    if self.is_idle_mode {
      self.is_idle_mode = false;
      // Re-fetch smaller album art for normal mode
      if let Some(url) = &self.current_album_art_url {
        self.dispatch(IoEvent::FetchAlbumArt(url.clone()));
      }
    }
  }

  /// Check if app should enter idle mode
  pub fn check_idle_mode(&mut self, idle_timeout_secs: u64) {
    if self.last_user_interaction.elapsed().as_secs() >= idle_timeout_secs && !self.is_idle_mode {
      self.is_idle_mode = true;
      // Fetch larger album art for idle mode
      if let Some(url) = &self.current_album_art_url {
        self.dispatch(IoEvent::FetchAlbumArt(url.clone()));
      }
    }
  }

}
