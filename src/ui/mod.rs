pub mod audio_analysis;
pub mod util;
use super::{
  app::{
    ActiveBlock, AlbumTableContext, App, ArtistBlock, EpisodeTableContext, RecommendationsContext,
    RouteId, SearchResultBlock, LIBRARY_OPTIONS,
  },
  banner::BANNER,
  user_config::Theme,
};
use rspotify::model::show::ResumePoint;
use crate::network::{PlayingItem, RepeatState};
use rspotify::model::{RepeatState as SpotifyRepeatState, PlayableItem};
use ratatui::{
  backend::{Backend, CrosstermBackend},
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  symbols::border,
  text::{Line, Span, Text},
  widgets::{Block, Borders, BorderType, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Table, Wrap},
  Frame,
};
use util::{
  create_artist_string, display_track_progress, get_artist_highlight_state, get_color,
  get_percentage_width, get_search_results_highlight_state, get_track_progress_percentage,
  millis_to_minutes, BASIC_VIEW_HEIGHT, SMALL_TERMINAL_WIDTH,
};

pub enum TableId {
  Album,
  AlbumList,
  Artist,
  Podcast,
  Song,
  RecentlyPlayed,
  MadeForYou,
  PodcastEpisodes,
}

#[derive(PartialEq)]
pub enum ColumnId {
  None,
  Title,
  Liked,
}

impl Default for ColumnId {
  fn default() -> Self {
    ColumnId::None
  }
}

pub struct TableHeader<'a> {
  id: TableId,
  items: Vec<TableHeaderItem<'a>>,
}

impl TableHeader<'_> {
  pub fn get_index(&self, id: ColumnId) -> Option<usize> {
    self.items.iter().position(|item| item.id == id)
  }
}

#[derive(Default)]
pub struct TableHeaderItem<'a> {
  id: ColumnId,
  text: &'a str,
  width: u16,
}

pub struct TableItem {
  id: String,
  format: Vec<String>,
}

/// Helper function to create a block with rounded corners and btop++ style
fn create_styled_block<'a>(title: &'a str, highlight_color: Color) -> Block<'a> {
  Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .title(Span::styled(
      title,
      Style::default().fg(highlight_color).add_modifier(Modifier::BOLD),
    ))
    .border_style(Style::default().fg(highlight_color))
}

/// Create a title with the first letter styled for focus
fn create_focus_title<'a>(title: &'a str, theme: &Theme, highlight_state: (bool, bool)) -> Vec<Span<'a>> {
  if title.is_empty() {
    return vec![Span::raw(title)];
  }
  
  let first_char = &title[0..1];
  let rest = if title.len() > 1 { &title[1..] } else { "" };
  
  vec![
    Span::styled(
      first_char,
      Style::default()
        .fg(theme.focus_letter)
        .add_modifier(Modifier::BOLD),
    ),
    Span::styled(
      rest,
      get_color(highlight_state, *theme),
    ),
  ]
}


pub fn draw_input_and_help_box<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  // Check for the width and change the contraints accordingly
  let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      if app.size.width >= SMALL_TERMINAL_WIDTH && !app.user_config.behavior.enforce_wide_search_bar
      {
        [Constraint::Percentage(65), Constraint::Percentage(35)].as_ref()
      } else {
        [Constraint::Percentage(90), Constraint::Percentage(10)].as_ref()
      },
    )
    .split(layout_chunk);

  let current_route = app.get_current_route();

  let highlight_state = (
    current_route.active_block == ActiveBlock::Input,
    current_route.hovered_block == ActiveBlock::Input,
  );

  let input_string: String = app.input.iter().collect();
  let lines = Text::from((&input_string).as_str());
  let search_title_spans = create_focus_title("Search", &app.user_config.theme, highlight_state);
  let input = Paragraph::new(lines).block(
    Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .title(Line::from(search_title_spans))
      .border_style(get_color(highlight_state, app.user_config.theme))
  );
  f.render_widget(input, chunks[0]);

  let (device_text, text_color) = if let Some(context) = &app.current_playback_context {
    (context.device.name.clone(), app.user_config.theme.active)
  } else if let Some(devices) = &app.devices {
    if devices.devices.is_empty() {
      ("NO DEVICES".to_string(), Color::Red)
    } else if let Some(idx) = app.selected_device_index {
      if let Some(device) = devices.devices.get(idx) {
        (device.name.clone(), app.user_config.theme.inactive)
      } else if let Some(first_device) = devices.devices.first() {
        (first_device.name.clone(), app.user_config.theme.inactive)
      } else {
        ("NO DEVICES".to_string(), Color::Red)
      }
    } else if let Some(first_device) = devices.devices.first() {
      (first_device.name.clone(), app.user_config.theme.inactive)
    } else {
      ("NO DEVICES".to_string(), Color::Red)
    }
  } else {
    ("NO DEVICES".to_string(), Color::Red)
  };

  let device_highlight_state = (
    current_route.active_block == ActiveBlock::SelectDevice,
    current_route.hovered_block == ActiveBlock::SelectDevice,
  );
  
  let device_title_spans = create_focus_title("Device", &app.user_config.theme, device_highlight_state);
  let block = Block::default()
    .title(Line::from(device_title_spans))
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(text_color));

  let lines = Text::from(device_text.as_str());
  let device_display = Paragraph::new(lines)
    .block(block)
    .style(Style::default().fg(text_color));
  f.render_widget(device_display, chunks[1]);
}

pub fn draw_main_layout(f: &mut Frame, app: &App) {
  let margin = util::get_main_layout_margin(app);
  // Responsive layout: new one kicks in at width 150 or higher
  if app.size.width >= SMALL_TERMINAL_WIDTH && !app.user_config.behavior.enforce_wide_search_bar {
    let parent_layout = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Min(1), Constraint::Length(8)].as_ref())
      .margin(margin)
      .split(f.area());

    // Nested main block with potential routes
    draw_routes::<CrosstermBackend<std::io::Stdout>>(f, app, parent_layout[0]);

    // Currently playing (now taller)
    draw_playbar::<CrosstermBackend<std::io::Stdout>>(f, app, parent_layout[1]);
  } else {
    let parent_layout = Layout::default()
      .direction(Direction::Vertical)
      .constraints(
        [
          Constraint::Length(3),
          Constraint::Min(1),
          Constraint::Length(8),
        ]
        .as_ref(),
      )
      .margin(margin)
      .split(f.area());

    // Search input and help
    draw_input_and_help_box::<CrosstermBackend<std::io::Stdout>>(f, app, parent_layout[0]);

    // Nested main block with potential routes
    draw_routes::<CrosstermBackend<std::io::Stdout>>(f, app, parent_layout[1]);

    // Currently playing (now taller)
    draw_playbar::<CrosstermBackend<std::io::Stdout>>(f, app, parent_layout[2]);
  }

  // Possibly draw confirm dialog
  draw_dialog::<CrosstermBackend<std::io::Stdout>>(f, app);
}

pub fn draw_breadcrumb_box(f: &mut Frame, app: &App, layout_chunk: Rect) {
  let breadcrumb_text = app.get_navigation_breadcrumb();
  
  let block = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(app.user_config.theme.inactive));

  let lines = Text::from(breadcrumb_text.as_str());
  let breadcrumb = Paragraph::new(lines)
    .block(block)
    .style(Style::default().fg(app.user_config.theme.text));
  
  f.render_widget(breadcrumb, layout_chunk);
}

pub fn draw_routes<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
    .split(layout_chunk);

  draw_user_block(f, app, chunks[0]);

  // Split the right side into breadcrumb (top) and main content (bottom)
  let right_chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
    .split(chunks[1]);

  // Draw breadcrumb box at the top of the right side
  draw_breadcrumb_box(f, app, right_chunks[0]);

  let current_route = app.get_current_route();

  match current_route.id {
    RouteId::Search => {
      draw_search_results::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::TrackTable => {
      draw_song_table::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::AlbumTracks => {
      draw_album_table::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::RecentlyPlayed => {
      draw_recently_played_table::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::Artist => {
      draw_artist_albums::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::AlbumList => {
      draw_album_list::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::PodcastEpisodes => {
      draw_show_episodes::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::Home => {
      draw_home::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::MadeForYou => {
      draw_made_for_you::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::Artists => {
      draw_artist_table::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::Podcasts => {
      draw_podcast_table::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::Recommendations => {
      draw_recommendations_table::<CrosstermBackend<std::io::Stdout>>(f, app, right_chunks[1]);
    }
    RouteId::SelectedDevice => {} // This is handled as a "full screen" route in main.rs
    RouteId::Analysis => {} // This is handled as a "full screen" route in main.rs
    RouteId::BasicView => {} // This is handled as a "full screen" route in main.rs
    RouteId::LogStream => {} // This is handled as a "full screen" route in main.rs
    RouteId::Error => {} // Error screen no longer exists, errors are handled via log stream
    RouteId::Dialog => {} // This is handled in the draw_dialog function in mod.rs
  };
}

pub fn draw_library_block<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::Library,
    current_route.hovered_block == ActiveBlock::Library,
  );
  draw_selectable_list::<&str>(
    f,
    app,
    layout_chunk,
    "Library",
    &LIBRARY_OPTIONS,
    highlight_state,
    Some(app.library.selected_index),
  );
}

pub fn draw_playlist_block<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let playlist_items = match &app.playlists {
    Some(p) => p.items.iter().map(|item| item.name.to_owned()).collect(),
    None => vec![],
  };

  let current_route = app.get_current_route();

  let highlight_state = (
    current_route.active_block == ActiveBlock::MyPlaylists,
    current_route.hovered_block == ActiveBlock::MyPlaylists,
  );

  draw_selectable_list::<String>(
    f,
    app,
    layout_chunk,
    "Playlists",
    &playlist_items,
    highlight_state,
    app.selected_playlist_index,
  );
}

pub fn draw_user_block(f: &mut Frame, app: &App, layout_chunk: Rect) {
  // Check for width to make a responsive layout
  if app.size.width >= SMALL_TERMINAL_WIDTH && !app.user_config.behavior.enforce_wide_search_bar {
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints(
        [
          Constraint::Length(3),
          Constraint::Percentage(30),
          Constraint::Percentage(70),
        ]
        .as_ref(),
      )
      .split(layout_chunk);

    // Search input and help
    draw_input_and_help_box::<CrosstermBackend<std::io::Stdout>>(f, app, chunks[0]);
    draw_library_block::<CrosstermBackend<std::io::Stdout>>(f, app, chunks[1]);
    draw_playlist_block::<CrosstermBackend<std::io::Stdout>>(f, app, chunks[2]);
  } else {
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
      .split(layout_chunk);

    // Search input and help
    draw_library_block::<CrosstermBackend<std::io::Stdout>>(f, app, chunks[0]);
    draw_playlist_block::<CrosstermBackend<std::io::Stdout>>(f, app, chunks[1]);
  }
}

pub fn draw_search_results<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints(
      [
        Constraint::Percentage(35),
        Constraint::Percentage(35),
        Constraint::Percentage(25),
      ]
      .as_ref(),
    )
    .split(layout_chunk);

  {
    let song_artist_block = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
      .split(chunks[0]);

    let currently_playing_id = app
      .current_playback_context
      .clone()
      .and_then(|context| {
        context.item.and_then(|item| match item {
          PlayableItem::Track(track) => track.id.map(|id| id.to_string()),
          PlayableItem::Episode(episode) => Some(episode.id.to_string()),
        })
      })
      .unwrap_or_else(|| "".to_string());

    let songs = match &app.search_results.tracks {
      Some(tracks) => tracks
        .items
        .iter()
        .map(|item| {
          let mut song_name = "".to_string();
          let id = item.clone().id.map(|id| id.to_string()).unwrap_or_else(|| "".to_string());
          if currently_playing_id == id {
            song_name += "▶ "
          }
          if app.liked_song_ids_set.contains(&id) {
            song_name += &app.user_config.padded_liked_icon();
          }

          song_name += &item.name;
          song_name += &format!(" - {}", &create_artist_string(&item.artists));
          song_name
        })
        .collect(),
      None => vec![],
    };

    draw_search_result_list(
      f,
      app,
      song_artist_block[0],
      "Songs",
      &songs,
      get_search_results_highlight_state(app, SearchResultBlock::SongSearch),
      app.search_results.selected_tracks_index,
    );

    let artists = match &app.search_results.artists {
      Some(artists) => artists
        .items
        .iter()
        .map(|item| {
          let mut artist = String::new();
          if app.followed_artist_ids_set.contains(&item.id.to_string()) {
            artist.push_str(&app.user_config.padded_liked_icon());
          }
          artist.push_str(&item.name.to_owned());
          artist
        })
        .collect(),
      None => vec![],
    };

    draw_search_result_list(
      f,
      app,
      song_artist_block[1],
      "Artists",
      &artists,
      get_search_results_highlight_state(app, SearchResultBlock::ArtistSearch),
      app.search_results.selected_artists_index,
    );
  }

  {
    let albums_playlist_block = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
      .split(chunks[1]);

    let albums = match &app.search_results.albums {
      Some(albums) => albums
        .items
        .iter()
        .map(|item| {
          let mut album_artist = String::new();
          if let Some(album_id) = &item.id {
            if app.saved_album_ids_set.contains(&album_id.to_string()) {
              album_artist.push_str(&app.user_config.padded_liked_icon());
            }
          }
          album_artist.push_str(&format!(
            "{} - {} ({})",
            item.name.to_owned(),
            create_artist_string(&item.artists),
            item.album_type.as_deref().unwrap_or("unknown")
          ));
          album_artist
        })
        .collect(),
      None => vec![],
    };

    draw_search_result_list(
      f,
      app,
      albums_playlist_block[0],
      "Albums",
      &albums,
      get_search_results_highlight_state(app, SearchResultBlock::AlbumSearch),
      app.search_results.selected_album_index,
    );

    let playlists = match &app.search_results.playlists {
      Some(playlists) => playlists
        .items
        .iter()
        .map(|item| item.name.to_owned())
        .collect(),
      None => vec![],
    };
    draw_search_result_list(
      f,
      app,
      albums_playlist_block[1],
      "Playlists",
      &playlists,
      get_search_results_highlight_state(app, SearchResultBlock::PlaylistSearch),
      app.search_results.selected_playlists_index,
    );
  }

  {
    let podcasts_block = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([Constraint::Percentage(100)].as_ref())
      .split(chunks[2]);

    let podcasts = match &app.search_results.shows {
      Some(podcasts) => podcasts
        .items
        .iter()
        .map(|item| {
          let mut show_name = String::new();
          if app.saved_show_ids_set.contains(&item.id.to_string()) {
            show_name.push_str(&app.user_config.padded_liked_icon());
          }
          show_name.push_str(&format!("{:} - {}", item.name, item.publisher));
          show_name
        })
        .collect(),
      None => vec![],
    };
    draw_search_result_list(
      f,
      app,
      podcasts_block[0],
      "Podcasts",
      &podcasts,
      get_search_results_highlight_state(app, SearchResultBlock::ShowSearch),
      app.search_results.selected_shows_index,
    );
  }
}

struct AlbumUi {
  selected_index: usize,
  items: Vec<TableItem>,
  title: String,
}

pub fn draw_artist_table<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::Artist,
    items: vec![TableHeaderItem {
      text: "Artist",
      width: get_percentage_width(layout_chunk.width, 1.0),
      ..Default::default()
    }],
  };

  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::Artists,
    current_route.hovered_block == ActiveBlock::Artists,
  );
  let items = app
    .artists
    .iter()
    .map(|item| TableItem {
      id: item.id.to_string(),
      format: vec![item.name.to_owned()],
    })
    .collect::<Vec<TableItem>>();

  draw_table::<CrosstermBackend<std::io::Stdout>>(
    f,
    app,
    layout_chunk,
    ("", &header),
    &items,
    app.artists_list_index,
    highlight_state,
  )
}

pub fn draw_podcast_table<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::Podcast,
    items: vec![
      TableHeaderItem {
        text: "Name",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Publisher(s)",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();

  let highlight_state = (
    current_route.active_block == ActiveBlock::Podcasts,
    current_route.hovered_block == ActiveBlock::Podcasts,
  );

  if let Some(saved_shows) = app.library.saved_shows.get_results(None) {
    let items = saved_shows
      .items
      .iter()
      .map(|show_page| TableItem {
        id: show_page.id.to_string(),
        format: vec![
          show_page.name.to_owned(),
          show_page.publisher.to_owned(),
        ],
      })
      .collect::<Vec<TableItem>>();

    draw_table::<CrosstermBackend<std::io::Stdout>>(
      f,
      app,
      layout_chunk,
      ("", &header),
      &items,
      app.shows_list_index,
      highlight_state,
    )
  };
}

pub fn draw_album_table<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::Album,
    items: vec![
      TableHeaderItem {
        id: ColumnId::Liked,
        text: "",
        width: 2,
      },
      TableHeaderItem {
        text: "#",
        width: 3,
        ..Default::default()
      },
      TableHeaderItem {
        id: ColumnId::Title,
        text: "Title",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0) - 5,
      },
      TableHeaderItem {
        text: "Artist",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Length",
        width: get_percentage_width(layout_chunk.width, 1.0 / 5.0),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::AlbumTracks,
    current_route.hovered_block == ActiveBlock::AlbumTracks,
  );

  let album_ui = match &app.album_table_context {
    AlbumTableContext::Simplified => {
      app
        .selected_album_simplified
        .as_ref()
        .map(|selected_album_simplified| AlbumUi {
          items: selected_album_simplified
            .tracks
            .items
            .iter()
            .map(|item| TableItem {
              id: item.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()),
              format: vec![
                "".to_string(),
                item.track_number.to_string(),
                item.name.to_owned(),
                create_artist_string(&item.artists),
                millis_to_minutes(item.duration.num_milliseconds() as u128),
              ],
            })
            .collect::<Vec<TableItem>>(),
          title: format!(
            "{} by {}",
            selected_album_simplified.album.name,
            create_artist_string(&selected_album_simplified.album.artists)
          ),
          selected_index: selected_album_simplified.selected_index,
        })
    }
    AlbumTableContext::Full => match app.selected_album_full.clone() {
      Some(selected_album) => Some(AlbumUi {
        items: selected_album
          .album
          .tracks
          .items
          .iter()
          .map(|item| TableItem {
            id: item.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()),
            format: vec![
              "".to_string(),
              item.track_number.to_string(),
              item.name.to_owned(),
              create_artist_string(&item.artists),
              millis_to_minutes(item.duration.num_milliseconds() as u128),
            ],
          })
          .collect::<Vec<TableItem>>(),
        title: format!(
          "{} by {}",
          selected_album.album.name,
          create_artist_string(&selected_album.album.artists)
        ),
        selected_index: app.saved_album_tracks_index,
      }),
      None => None,
    },
  };

  if let Some(album_ui) = album_ui {
    draw_table::<CrosstermBackend<std::io::Stdout>>(
      f,
      app,
      layout_chunk,
      (&album_ui.title, &header),
      &album_ui.items,
      album_ui.selected_index,
      highlight_state,
    );
  };
}

pub fn draw_recommendations_table<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::Song,
    items: vec![
      TableHeaderItem {
        id: ColumnId::Liked,
        text: "",
        width: 2,
      },
      TableHeaderItem {
        id: ColumnId::Title,
        text: "Title",
        width: get_percentage_width(layout_chunk.width, 0.3),
      },
      TableHeaderItem {
        text: "Artist",
        width: get_percentage_width(layout_chunk.width, 0.3),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Album",
        width: get_percentage_width(layout_chunk.width, 0.3),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Length",
        width: get_percentage_width(layout_chunk.width, 0.1),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::TrackTable,
    current_route.hovered_block == ActiveBlock::TrackTable,
  );

  let items = app
    .track_table
    .tracks
    .iter()
    .map(|item| TableItem {
      id: item.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()),
      format: vec![
        "".to_string(),
        item.name.to_owned(),
        create_artist_string(&item.artists),
        item.album.name.to_owned(),
        millis_to_minutes(item.duration.num_milliseconds() as u128),
      ],
    })
    .collect::<Vec<TableItem>>();
  // match RecommendedContext
  let recommendations_ui = match &app.recommendations_context {
    Some(RecommendationsContext::Song) => format!(
      "Recommendations based on Song \'{}\'",
      &app.recommendations_seed
    ),
    Some(RecommendationsContext::Artist) => format!(
      "Recommendations based on Artist \'{}\'",
      &app.recommendations_seed
    ),
    None => "Recommendations".to_string(),
  };
  draw_table::<CrosstermBackend<std::io::Stdout>>(
    f,
    app,
    layout_chunk,
    (&recommendations_ui[..], &header),
    &items,
    app.track_table.selected_index,
    highlight_state,
  )
}

pub fn draw_song_table<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::Song,
    items: vec![
      TableHeaderItem {
        id: ColumnId::Liked,
        text: "",
        width: 2,
      },
      TableHeaderItem {
        id: ColumnId::Title,
        text: "Title",
        width: get_percentage_width(layout_chunk.width, 0.3),
      },
      TableHeaderItem {
        text: "Artist",
        width: get_percentage_width(layout_chunk.width, 0.3),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Album",
        width: get_percentage_width(layout_chunk.width, 0.3),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Length",
        width: get_percentage_width(layout_chunk.width, 0.1),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::TrackTable,
    current_route.hovered_block == ActiveBlock::TrackTable,
  );

  let items = app
    .track_table
    .tracks
    .iter()
    .map(|item| TableItem {
      id: item.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()),
      format: vec![
        "".to_string(),
        item.name.to_owned(),
        create_artist_string(&item.artists),
        item.album.name.to_owned(),
        millis_to_minutes(item.duration.num_milliseconds() as u128),
      ],
    })
    .collect::<Vec<TableItem>>();

  draw_table::<CrosstermBackend<std::io::Stdout>>(
    f,
    app,
    layout_chunk,
    ("", &header),
    &items,
    app.track_table.selected_index,
    highlight_state,
  )
}

pub fn draw_basic_view(f: &mut Frame, app: &App) {
  // If space is negative, do nothing because the widget would not fit
  if let Some(s) = app.size.height.checked_sub(BASIC_VIEW_HEIGHT) {
    let space = s / 2;
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints(
        [
          Constraint::Length(space),
          Constraint::Length(BASIC_VIEW_HEIGHT),
          Constraint::Length(space),
        ]
        .as_ref(),
      )
      .split(f.area());

    draw_playbar::<CrosstermBackend<std::io::Stdout>>(f, app, chunks[1]);
  }
}

pub fn draw_playbar<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  // First split horizontally to make room for album art
  let horizontal_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      if app.current_album_art.is_some() {
        [Constraint::Length(20), Constraint::Min(1)].as_ref()
      } else {
        [Constraint::Min(1)].as_ref()
      }
    )
    .split(layout_chunk);

  // If we have album art, draw it in the left chunk
  if app.current_album_art.is_some() {
    draw_album_art(f, app, horizontal_chunks[0]);
  }

  // Use the right chunk (or full area if no art) for the playbar
  let playbar_chunk = if app.current_album_art.is_some() {
    horizontal_chunks[1]
  } else {
    horizontal_chunks[0]
  };

  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints(
      [
        Constraint::Percentage(50),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
      ]
      .as_ref(),
    )
    .margin(1)
    .split(playbar_chunk);

  // If no track is playing, render paragraph showing which device is selected, if no selected
  // give hint to choose a device
  if let Some(current_playback_context) = &app.current_playback_context {
    if let Some(track_item) = &current_playback_context.item {
      let play_title = if current_playback_context.is_playing {
        "Playing"
      } else {
        "Paused"
      };

      let shuffle_text = if current_playback_context.shuffle_state {
        "On"
      } else {
        "Off"
      };

      let repeat_text = match current_playback_context.repeat_state {
        SpotifyRepeatState::Off => "Off",
        SpotifyRepeatState::Track => "Track",
        SpotifyRepeatState::Context => "All",
      };

      let title = format!(
        "{:-7} ({} | Shuffle: {:-3} | Repeat: {:-5} | Volume: {:-2}%)",
        play_title,
        current_playback_context.device.name,
        shuffle_text,
        repeat_text,
        current_playback_context.device.volume_percent.unwrap_or(0)
      );

      let current_route = app.get_current_route();
      let highlight_state = (
        current_route.active_block == ActiveBlock::PlayBar,
        current_route.hovered_block == ActiveBlock::PlayBar,
      );

      let title_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
          &title,
          get_color(highlight_state, app.user_config.theme),
        ))
        .border_style(get_color(highlight_state, app.user_config.theme));

      f.render_widget(title_block, layout_chunk);

      let (item_id, name, duration_ms) = match track_item {
        PlayableItem::Track(track) => (
          track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()),
          track.name.to_owned(),
          track.duration,
        ),
        PlayableItem::Episode(episode) => (
          episode.id.to_string(),
          episode.name.to_owned(),
          episode.duration,
        ),
      };

      let track_name = if app.liked_song_ids_set.contains(&item_id) {
        format!("{}{}", &app.user_config.padded_liked_icon(), name)
      } else {
        name
      };

      let play_bar_text = match track_item {
        PlayableItem::Track(track) => create_artist_string(&track.artists),
        PlayableItem::Episode(episode) => format!("{}", episode.name), // Note: episode.show not available in newer API
      };

      let lines = Text::from(Span::styled(
        play_bar_text,
        Style::default().fg(app.user_config.theme.playbar_text),
      ));

      let artist = Paragraph::new(lines)
        .style(Style::default().fg(app.user_config.theme.playbar_text))
        .block(
          Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
              &track_name,
              Style::default()
                .fg(app.user_config.theme.selected)
                .add_modifier(Modifier::BOLD),
            ))
        );
      f.render_widget(artist, chunks[0]);

      let progress_ms = match app.seek_ms {
        Some(seek_ms) => seek_ms,
        None => app.song_progress_ms,
      };

      let perc = get_track_progress_percentage(progress_ms, duration_ms.num_milliseconds() as u32);

      let song_progress_label = display_track_progress(progress_ms, duration_ms.num_milliseconds() as u32);
      let modifier = if app.user_config.behavior.enable_text_emphasis {
        Modifier::ITALIC | Modifier::BOLD
      } else {
        Modifier::empty()
      };
      let song_progress = Gauge::default()
        .gauge_style(
          Style::default()
            .fg(app.user_config.theme.playbar_progress)
            .bg(app.user_config.theme.playbar_background)
            .add_modifier(modifier),
        )
        .percent(perc)
        .label(Span::styled(
          &song_progress_label,
          Style::default().fg(app.user_config.theme.playbar_progress_text),
        ));
      f.render_widget(song_progress, chunks[2]);
    } else {
      // Clear the playbar area when no track is playing
      let device_text = format!(
        "Connected to: {} - No track playing",
        current_playback_context.device.name
      );
      let empty_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
          &device_text,
          Style::default().fg(app.user_config.theme.inactive),
        ))
        .border_style(Style::default().fg(app.user_config.theme.inactive));
      f.render_widget(empty_block, layout_chunk);
    }
  } else {
    // Clear the playbar area when no playback context exists
    let empty_block = Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .title(Span::styled(
        "No active playback - Press 'd' to select a device",
        Style::default().fg(app.user_config.theme.inactive),
      ))
      .border_style(Style::default().fg(app.user_config.theme.inactive));
    f.render_widget(empty_block, layout_chunk);
  }
}

fn draw_home<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(7), Constraint::Length(93)].as_ref())
    .margin(2)
    .split(layout_chunk);

  let current_route = app.get_current_route();
  let highlight_state = (
    current_route.active_block == ActiveBlock::Home,
    current_route.hovered_block == ActiveBlock::Home,
  );

  let welcome = Block::default()
    .title(Span::styled(
      "Welcome!",
      get_color(highlight_state, app.user_config.theme),
    ))
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(get_color(highlight_state, app.user_config.theme));
  f.render_widget(welcome, layout_chunk);

  let changelog = include_str!("../../CHANGELOG.md").to_string();

  // If debug mode show the "Unreleased" header. Otherwise it is a release so there should be no
  // unreleased features
  let clean_changelog = if cfg!(debug_assertions) {
    changelog
  } else {
    changelog.replace("\n## [Unreleased]\n", "")
  };

  // Banner text with correct styling
  let top_text = Text::from(BANNER);

  let bottom_text_raw = format!(
    "{}{}",
    "\nPlease report any bugs or missing features to https://github.com/Rigellute/spotify-tui\n\n",
    clean_changelog
  );
  let bottom_text = Text::from(bottom_text_raw.as_str());

  // Contains the banner
  let top_text = Paragraph::new(top_text)
    .style(Style::default().fg(app.user_config.theme.banner))
    .block(Block::default());
  f.render_widget(top_text, chunks[0]);

  // CHANGELOG
  let bottom_text = Paragraph::new(bottom_text)
    .style(Style::default().fg(app.user_config.theme.text))
    .block(Block::default())
    .wrap(Wrap { trim: false })
    .scroll((app.home_scroll, 0));
  f.render_widget(bottom_text, chunks[1]);
}

fn draw_artist_albums<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      [
        Constraint::Percentage(33),
        Constraint::Percentage(33),
        Constraint::Percentage(33),
      ]
      .as_ref(),
    )
    .split(layout_chunk);

  if let Some(artist) = &app.artist {
    let top_tracks = artist
      .top_tracks
      .iter()
      .map(|top_track| {
        let mut name = String::new();
        if let Some(context) = &app.current_playback_context {
          let track_id = match &context.item {
            Some(PlayableItem::Track(track)) => track.id.as_ref().map(|id| id.to_string()),
            Some(PlayableItem::Episode(episode)) => Some(episode.id.to_string()),
            _ => None,
          };

          if track_id == top_track.id.as_ref().map(|id| id.to_string()) {
            name.push_str("▶ ");
          }
        };
        name.push_str(&top_track.name);
        name
      })
      .collect::<Vec<String>>();

    draw_selectable_list(
      f,
      app,
      chunks[0],
      "Top Tracks",
      &top_tracks,
      get_artist_highlight_state(app, ArtistBlock::TopTracks),
      Some(artist.selected_top_track_index),
    );

    let albums = &artist
      .albums
      .items
      .iter()
      .map(|item| {
        let mut album_artist = String::new();
        if let Some(album_id) = &item.id {
          if app.saved_album_ids_set.contains(&album_id.to_string()) {
            album_artist.push_str(&app.user_config.padded_liked_icon());
          }
        }
        album_artist.push_str(&format!(
          "{} - {} ({})",
          item.name.to_owned(),
          create_artist_string(&item.artists),
          item.album_type.as_deref().unwrap_or("unknown")
        ));
        album_artist
      })
      .collect::<Vec<String>>();

    draw_selectable_list(
      f,
      app,
      chunks[1],
      "Albums",
      albums,
      get_artist_highlight_state(app, ArtistBlock::Albums),
      Some(artist.selected_album_index),
    );

    let related_artists = artist
      .related_artists
      .iter()
      .map(|item| {
        let mut artist = String::new();
        if app.followed_artist_ids_set.contains(&item.id.to_string()) {
          artist.push_str(&app.user_config.padded_liked_icon());
        }
        artist.push_str(&item.name.to_owned());
        artist
      })
      .collect::<Vec<String>>();

    draw_selectable_list(
      f,
      app,
      chunks[2],
      "Related artists",
      &related_artists,
      get_artist_highlight_state(app, ArtistBlock::RelatedArtists),
      Some(artist.selected_related_artist_index),
    );
  };
}

pub fn draw_device_list(f: &mut Frame, app: &App) {
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
    .margin(5)
    .split(f.area());

  let device_instructions: Vec<Line> = vec![
        "To play tracks, please select a device. ",
        "Use `j/k` or up/down arrow keys to move up and down and <Enter> to select. ",
        "Your choice here will be cached so you can jump straight back in when you next open `spotify-tui`. ",
        "You can change the playback device at any time by pressing `d`.",
    ].into_iter().map(|instruction| Line::from(Span::raw(instruction))).collect();

  let instructions = Paragraph::new(device_instructions)
    .style(Style::default().fg(app.user_config.theme.text))
    .wrap(Wrap { trim: true })
    .block(
      Block::default().borders(Borders::NONE).title(Span::styled(
        "Welcome to spotify-tui!",
        Style::default()
          .fg(app.user_config.theme.active)
          .add_modifier(Modifier::BOLD),
      ))
    );
  f.render_widget(instructions, chunks[0]);

  let no_device_message = Span::raw("No devices found: Make sure a device is active");

  let items = match &app.devices {
    Some(items) => {
      if items.devices.is_empty() {
        vec![ListItem::new(no_device_message)]
      } else {
        items
          .devices
          .iter()
          .map(|device| ListItem::new(Span::raw(&device.name)))
          .collect()
      }
    }
    None => vec![ListItem::new(no_device_message)],
  };

  let mut state = ListState::default();
  state.select(app.selected_device_index);
  let list = List::new(items)
    .block(
      Block::default()
        .title(Line::from(vec![
          Span::styled(
            "D",
            Style::default()
              .fg(app.user_config.theme.active)
              .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
          ),
          Span::styled(
            "evices",
            Style::default().fg(app.user_config.theme.active),
          ),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.user_config.theme.inactive))
    )
    .style(Style::default().fg(app.user_config.theme.text))
    .highlight_style(
      Style::default()
        .fg(app.user_config.theme.active)
        .add_modifier(Modifier::BOLD),
    );
  f.render_stateful_widget(list, chunks[1], &mut state);
}

pub fn draw_album_list<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::AlbumList,
    items: vec![
      TableHeaderItem {
        text: "Name",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Artists",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Release Date",
        width: get_percentage_width(layout_chunk.width, 1.0 / 5.0),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();

  let highlight_state = (
    current_route.active_block == ActiveBlock::AlbumList,
    current_route.hovered_block == ActiveBlock::AlbumList,
  );

  let selected_song_index = app.album_list_index;

  if let Some(saved_albums) = app.library.saved_albums.get_results(None) {
    let items = saved_albums
      .items
      .iter()
      .map(|album_page| TableItem {
        id: album_page.album.id.to_string(),
        format: vec![
          format!(
            "{}{}",
            app.user_config.padded_liked_icon(),
            &album_page.album.name
          ),
          create_artist_string(&album_page.album.artists),
          album_page.album.release_date.to_owned(),
        ],
      })
      .collect::<Vec<TableItem>>();

    draw_table::<CrosstermBackend<std::io::Stdout>>(
      f,
      app,
      layout_chunk,
      ("", &header),
      &items,
      selected_song_index,
      highlight_state,
    )
  };
}

pub fn draw_show_episodes<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::PodcastEpisodes,
    items: vec![
      TableHeaderItem {
        // Column to mark an episode as fully played
        text: "",
        width: 2,
        ..Default::default()
      },
      TableHeaderItem {
        text: "Date",
        width: get_percentage_width(layout_chunk.width, 0.5 / 5.0) - 2,
        ..Default::default()
      },
      TableHeaderItem {
        text: "Name",
        width: get_percentage_width(layout_chunk.width, 3.5 / 5.0),
        id: ColumnId::Title,
      },
      TableHeaderItem {
        text: "Duration",
        width: get_percentage_width(layout_chunk.width, 1.0 / 5.0),
        ..Default::default()
      },
    ],
  };

  let current_route = app.get_current_route();

  let highlight_state = (
    current_route.active_block == ActiveBlock::EpisodeTable,
    current_route.hovered_block == ActiveBlock::EpisodeTable,
  );

  if let Some(episodes) = app.library.show_episodes.get_results(None) {
    let items = episodes
      .items
      .iter()
      .map(|episode| {
        let (played_str, time_str) = match episode.resume_point {
          Some(ResumePoint {
            fully_played,
            resume_position,
          }) => (
            if fully_played {
              " ✔".to_owned()
            } else {
              "".to_owned()
            },
            format!(
              "{} / {}",
              millis_to_minutes(resume_position.num_milliseconds() as u128),
              millis_to_minutes(episode.duration.num_milliseconds() as u128)
            ),
          ),
          None => (
            "".to_owned(),
            millis_to_minutes(episode.duration.num_milliseconds() as u128),
          ),
        };
        TableItem {
          id: episode.id.to_string(),
          format: vec![
            played_str,
            episode.release_date.to_owned(),
            episode.name.to_owned(),
            time_str,
          ],
        }
      })
      .collect::<Vec<TableItem>>();

    let title = match &app.episode_table_context {
      EpisodeTableContext::Simplified => match &app.selected_show_simplified {
        Some(selected_show) => {
          format!(
            "{} by {}",
            selected_show.show.name.to_owned(),
            selected_show.show.publisher
          )
        }
        None => "Episodes".to_owned(),
      },
      EpisodeTableContext::Full => match &app.selected_show_full {
        Some(selected_show) => {
          format!(
            "{} by {}",
            selected_show.show.name.to_owned(),
            selected_show.show.publisher
          )
        }
        None => "Episodes".to_owned(),
      },
    };

    draw_table::<CrosstermBackend<std::io::Stdout>>(
      f,
      app,
      layout_chunk,
      (&title, &header),
      &items,
      app.episode_list_index,
      highlight_state,
    );
  };
}

pub fn draw_made_for_you<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::MadeForYou,
    items: vec![TableHeaderItem {
      text: "Name",
      width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
      ..Default::default()
    }],
  };

  if let Some(playlists) = &app.library.made_for_you_playlists.get_results(None) {
    let items = playlists
      .items
      .iter()
      .map(|playlist| TableItem {
        id: playlist.id.to_string(),
        format: vec![playlist.name.to_owned()],
      })
      .collect::<Vec<TableItem>>();

    let current_route = app.get_current_route();
    let highlight_state = (
      current_route.active_block == ActiveBlock::MadeForYou,
      current_route.hovered_block == ActiveBlock::MadeForYou,
    );

    draw_table::<CrosstermBackend<std::io::Stdout>>(
      f,
      app,
      layout_chunk,
      ("", &header),
      &items,
      app.made_for_you_index,
      highlight_state,
    );
  }
}

pub fn draw_recently_played_table<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let header = TableHeader {
    id: TableId::RecentlyPlayed,
    items: vec![
      TableHeaderItem {
        id: ColumnId::Liked,
        text: "",
        width: 2,
      },
      TableHeaderItem {
        id: ColumnId::Title,
        text: "Title",
        // We need to subtract the fixed value of the previous column
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0) - 2,
      },
      TableHeaderItem {
        text: "Artist",
        width: get_percentage_width(layout_chunk.width, 2.0 / 5.0),
        ..Default::default()
      },
      TableHeaderItem {
        text: "Length",
        width: get_percentage_width(layout_chunk.width, 1.0 / 5.0),
        ..Default::default()
      },
    ],
  };

  if let Some(recently_played) = &app.recently_played.result {
    let current_route = app.get_current_route();

    let highlight_state = (
      current_route.active_block == ActiveBlock::RecentlyPlayed,
      current_route.hovered_block == ActiveBlock::RecentlyPlayed,
    );

    let selected_song_index = app.recently_played.index;

    let items = recently_played
      .items
      .iter()
      .map(|item| TableItem {
        id: item.track.id.as_ref().map(|id| id.to_string()).unwrap_or_else(|| "".to_string()),
        format: vec![
          "".to_string(),
          item.track.name.to_owned(),
          create_artist_string(&item.track.artists),
          millis_to_minutes(item.track.duration.num_milliseconds() as u128),
        ],
      })
      .collect::<Vec<TableItem>>();

    draw_table::<CrosstermBackend<std::io::Stdout>>(
      f,
      app,
      layout_chunk,
      ("", &header),
      &items,
      selected_song_index,
      highlight_state,
    )
  };
}

fn draw_selectable_list<S>(
  f: &mut Frame,
  app: &App,
  layout_chunk: Rect,
  title: &str,
  items: &[S],
  highlight_state: (bool, bool),
  selected_index: Option<usize>,
) where
  S: std::convert::AsRef<str>,
{
  let mut state = ListState::default();
  state.select(selected_index);

  let lst_items: Vec<ListItem> = items
    .iter()
    .map(|i| ListItem::new(Span::raw(i.as_ref())))
    .collect();

  let title_spans = create_focus_title(title, &app.user_config.theme, highlight_state);
  let list = List::new(lst_items)
    .block(
      Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(get_color(highlight_state, app.user_config.theme))
    )
    .style(Style::default().fg(app.user_config.theme.text))
    .highlight_style(
      get_color(highlight_state, app.user_config.theme).add_modifier(Modifier::BOLD),
    );
  f.render_stateful_widget(list, layout_chunk, &mut state);
}

// Special version for search results without focus letters
fn draw_search_result_list<S>(
  f: &mut Frame,
  app: &App,
  layout_chunk: Rect,
  title: &str,
  items: &[S],
  highlight_state: (bool, bool),
  selected_index: Option<usize>,
) where
  S: std::convert::AsRef<str>,
{
  let mut state = ListState::default();
  state.select(selected_index);

  let lst_items: Vec<ListItem> = items
    .iter()
    .map(|i| ListItem::new(Span::raw(i.as_ref())))
    .collect();

  // Use plain title without focus letters for search results
  let list = List::new(lst_items)
    .block(
      Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(get_color(highlight_state, app.user_config.theme))
    )
    .style(Style::default().fg(app.user_config.theme.text))
    .highlight_style(
      get_color(highlight_state, app.user_config.theme).add_modifier(Modifier::BOLD),
    );
  f.render_stateful_widget(list, layout_chunk, &mut state);
}

fn draw_dialog<B>(f: &mut Frame, app: &App)
{
  if let ActiveBlock::Dialog(_) = app.get_current_route().active_block {
    if let Some(playlist) = app.dialog.as_ref() {
      let bounds = f.area();
      // maybe do this better
      let width = std::cmp::min(bounds.width - 2, 45);
      let height = 8;
      let left = (bounds.width - width) / 2;
      let top = bounds.height / 4;

      let rect = Rect::new(left, top, width, height);

      f.render_widget(Clear, rect);

      let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.user_config.theme.inactive));

      f.render_widget(block, rect);

      let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
        .split(rect);

      // suggestion: possibly put this as part of
      // app.dialog, but would have to introduce lifetime
      let text = vec![
        Line::from(Span::raw("Are you sure you want to delete the playlist: ")),
        Line::from(Span::styled(
          playlist.as_str(),
          Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw("?")),
      ];

      let text = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);

      f.render_widget(text, vchunks[0]);

      let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .horizontal_margin(3)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)].as_ref())
        .split(vchunks[1]);

      let ok_text = Span::raw("Ok");
      let ok = Paragraph::new(ok_text)
        .style(Style::default().fg(if app.confirm {
          app.user_config.theme.hovered
        } else {
          app.user_config.theme.inactive
        }))
        .alignment(Alignment::Center);

      f.render_widget(ok, hchunks[0]);

      let cancel_text = Span::raw("Cancel");
      let cancel = Paragraph::new(cancel_text)
        .style(Style::default().fg(if app.confirm {
          app.user_config.theme.inactive
        } else {
          app.user_config.theme.hovered
        }))
        .alignment(Alignment::Center);

      f.render_widget(cancel, hchunks[1]);
    }
  }
}

fn draw_table<B>(
  f: &mut Frame,
  app: &App,
  layout_chunk: Rect,
  table_layout: (&str, &TableHeader), // (title, header colums)
  items: &[TableItem], // The nested vector must have the same length as the `header_columns`
  selected_index: usize,
  highlight_state: (bool, bool),
) {
  let selected_style =
    get_color(highlight_state, app.user_config.theme).add_modifier(Modifier::BOLD);

  let track_playing_index = app.current_playback_context.to_owned().and_then(|ctx| {
    ctx.item.and_then(|item| {
      let playing_item = PlayableItem::from(item);
      match playing_item {
        PlayableItem::Track(track) => items
          .iter()
          .position(|item| track.id.as_ref().map(|id| id.to_string() == item.id).unwrap_or(false)),
        PlayableItem::Episode(episode) => items.iter().position(|item| episode.id.to_string() == item.id),
      }
    })
  });

  let (title, header) = table_layout;

  // Make sure that the selected item is visible on the page. Need to add some rows of padding
  // to chunk height for header and header space to get a true table height
  let padding = 5;
  let offset = layout_chunk
    .height
    .checked_sub(padding)
    .and_then(|height| selected_index.checked_sub(height as usize))
    .unwrap_or(0);

  let rows = items.iter().skip(offset).enumerate().map(|(i, item)| {
    let mut formatted_row = item.format.clone();
    let mut style = Style::default().fg(app.user_config.theme.text); // default styling

    // if table displays songs
    match header.id {
      TableId::Song | TableId::RecentlyPlayed | TableId::Album => {
        // First check if the song should be highlighted because it is currently playing
        if let Some(title_idx) = header.get_index(ColumnId::Title) {
          if let Some(track_playing_offset_index) =
            track_playing_index.and_then(|idx| idx.checked_sub(offset))
          {
            if i == track_playing_offset_index {
              formatted_row[title_idx] = format!("▶ {}", &formatted_row[title_idx]);
              style = Style::default()
                .fg(app.user_config.theme.active)
                .add_modifier(Modifier::BOLD);
            }
          }
        }

        // Show this the liked icon if the song is liked
        if let Some(liked_idx) = header.get_index(ColumnId::Liked) {
          if app.liked_song_ids_set.contains(item.id.as_str()) {
            formatted_row[liked_idx] = app.user_config.padded_liked_icon();
          }
        }
      }
      TableId::PodcastEpisodes => {
        if let Some(name_idx) = header.get_index(ColumnId::Title) {
          if let Some(track_playing_offset_index) =
            track_playing_index.and_then(|idx| idx.checked_sub(offset))
          {
            if i == track_playing_offset_index {
              formatted_row[name_idx] = format!("▶ {}", &formatted_row[name_idx]);
              style = Style::default()
                .fg(app.user_config.theme.active)
                .add_modifier(Modifier::BOLD);
            }
          }
        }
      }
      _ => {}
    }

    // Next check if the item is under selection.
    if Some(i) == selected_index.checked_sub(offset) {
      style = selected_style;
    }

    // Return row styled data
    Row::new(formatted_row).style(style)
  });

  let widths = header
    .items
    .iter()
    .map(|h| Constraint::Length(h.width))
    .collect::<Vec<ratatui::layout::Constraint>>();

  let table = Table::new(rows, &widths)
    .header(
      Row::new(header.items.iter().map(|h| h.text))
        .style(Style::default().fg(app.user_config.theme.header))
    )
    .block(
      Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(app.user_config.theme.text))
        .title(Span::styled(
          title,
          get_color(highlight_state, app.user_config.theme),
        ))
        .border_style(get_color(highlight_state, app.user_config.theme))
    )
    .style(Style::default().fg(app.user_config.theme.text))
    .widths(&widths);
  f.render_widget(table, layout_chunk);
}

pub fn draw_log_stream<B>(f: &mut Frame, app: &App, layout_chunk: Rect)
{
  let is_active = app.get_current_route().active_block == ActiveBlock::LogStream;
  
  let log_items = if app.log_messages.is_empty() {
    vec![ListItem::new(Span::styled(
      "No log messages yet",
      Style::default().fg(app.user_config.theme.inactive),
    ))]
  } else {
    // Calculate visible range based on scroll offset and chunk height
    let visible_height = layout_chunk.height.saturating_sub(2) as usize; // Account for borders
    let total_messages = app.log_messages.len();
    
    // When not active, show last messages (original behavior)
    // When active, use scroll offset for navigation
    let (start_index, end_index) = if is_active {
      let start = app.log_stream_scroll_offset;
      let end = std::cmp::min(start + visible_height, total_messages);
      (start, end)
    } else {
      // Show last messages when not active
      let start = if total_messages > visible_height {
        total_messages - visible_height
      } else {
        0
      };
      (start, total_messages)
    };
    
    app.log_messages[start_index..end_index]
      .iter()
      .enumerate()
      .flat_map(|(i, message)| {
        let actual_index = start_index + i;
        // Check if this is an error message and style accordingly
        let is_error = message.contains("] ERROR:");
        
        let style = if is_active && actual_index == app.log_stream_selected_index {
          if is_error {
            Style::default()
              .bg(app.user_config.theme.hovered)
              .fg(Color::Red)
              .add_modifier(Modifier::BOLD)
          } else {
            Style::default()
              .bg(app.user_config.theme.hovered)
              .fg(app.user_config.theme.text)
          }
        } else if is_error {
          Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD)
        } else {
          Style::default().fg(app.user_config.theme.text)
        };
        
        // Split the message by newlines and create a ListItem for each line
        message.lines().map(move |line| {
          ListItem::new(Span::styled(line.to_string(), style))
        }).collect::<Vec<_>>()
      })
      .collect()
  };

  let border_style = if is_active {
    Style::default().fg(app.user_config.theme.active)
  } else {
    Style::default().fg(app.user_config.theme.inactive)
  };

  let title = if is_active {
    Line::from(vec![
      Span::styled(
        "L",
        Style::default()
          .fg(app.user_config.theme.header)
          .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
      ),
      Span::styled(
        format!("og Stream [{}/{}]", app.log_stream_selected_index + 1, app.log_messages.len()),
        Style::default().fg(app.user_config.theme.header),
      ),
    ])
  } else {
    Line::from(vec![
      Span::styled(
        "L",
        Style::default()
          .fg(app.user_config.theme.header)
          .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
      ),
      Span::styled(
        "og Stream",
        Style::default().fg(app.user_config.theme.header),
      ),
    ])
  };

  let log_list = List::new(log_items)
    .block(
      Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
    )
    .style(Style::default().fg(app.user_config.theme.text));

  f.render_widget(log_list, layout_chunk);
}

pub fn draw_log_stream_full_screen(f: &mut Frame, app: &App) {
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(3), Constraint::Min(10)].as_ref())
    .margin(2)
    .split(f.area());

  let instructions: Vec<Line> = vec![
    "Use j/k or ↑/↓ to navigate, Page Up/Down for faster scrolling",
    "Press 'g' for top, 'G' for bottom, Esc to go back",
  ].into_iter().map(|instruction| Line::from(Span::raw(instruction))).collect();

  let help_text = Paragraph::new(instructions)
    .style(Style::default().fg(app.user_config.theme.inactive))
    .wrap(Wrap { trim: true })
    .block(
      Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
          "Log Stream Help",
          Style::default()
            .fg(app.user_config.theme.header)
            .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(app.user_config.theme.inactive))
    );
  f.render_widget(help_text, chunks[0]);

  // Use the existing log stream drawing function for the main content
  draw_log_stream::<CrosstermBackend<std::io::Stdout>>(f, app, chunks[1]);
}

fn draw_album_art(f: &mut Frame, app: &App, layout_chunk: Rect) {
  if let Some(art) = &app.current_album_art {
    // Create a block for the album art
    let block = Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .border_style(Style::default().fg(app.user_config.theme.inactive));
    
    let inner_area = block.inner(layout_chunk);
    f.render_widget(block, layout_chunk);
    
    // Convert pixelated art to colored text
    let lines = crate::album_art::render_pixelated_art(art);
    
    // Calculate centering offsets
    let y_offset = inner_area.height.saturating_sub(lines.len() as u16) / 2;
    let x_offset = inner_area.width.saturating_sub(art.width as u16) / 2;
    
    // Render each line of pixels
    for (y, line) in lines.iter().enumerate() {
      let y_pos = inner_area.y + y_offset + y as u16;
      if y_pos >= inner_area.y + inner_area.height {
        break;
      }
      
      for (x, (ch, color)) in line.iter().enumerate() {
        let x_pos = inner_area.x + x_offset + x as u16;
        if x_pos >= inner_area.x + inner_area.width {
          break;
        }
        
        // Render each pixel as a colored block
        let pixel = Span::styled(ch, Style::default().fg(*color));
        let paragraph = Paragraph::new(pixel);
        let pixel_area = Rect {
          x: x_pos,
          y: y_pos,
          width: 1,
          height: 1,
        };
        f.render_widget(paragraph, pixel_area);
      }
    }
  }
}

/// Draw the idle mode screensaver with large album art
pub fn draw_idle_mode(f: &mut Frame, app: &App) {
  // Split the screen into vertical sections
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Percentage(10),  // Top margin
      Constraint::Percentage(80),  // Album art area
      Constraint::Percentage(10),  // Bottom area for progress bar
    ].as_ref())
    .split(f.area());

  // Draw large album art in the center
  if app.current_album_art.is_some() {
    // Split horizontally to center the album art
    let horizontal_chunks = Layout::default()
      .direction(Direction::Horizontal)
      .constraints([
        Constraint::Percentage(10),
        Constraint::Percentage(80),
        Constraint::Percentage(10),
      ].as_ref())
      .split(chunks[1]);

    // Draw the album art in the center chunk
    draw_large_album_art(f, app, horizontal_chunks[1]);
  }

  // Draw progress bar at the bottom
  if let Some(context) = &app.current_playback_context {
    if let Some(item) = &context.item {
      let progress_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
          Constraint::Percentage(50),
          Constraint::Percentage(50),
        ].as_ref())
        .margin(2)
        .split(chunks[2]);

      // Draw track info
      let track_info = match item {
        PlayableItem::Track(track) => {
          format!("{} - {}", track.name, create_artist_string(&track.artists))
        }
        PlayableItem::Episode(episode) => episode.name.clone(),
      };

      let track_paragraph = Paragraph::new(track_info)
        .style(Style::default().fg(app.user_config.theme.selected))
        .alignment(Alignment::Center);
      f.render_widget(track_paragraph, progress_chunks[0]);

      // Draw progress bar
      let (progress_ms, duration_ms) = match item {
        PlayableItem::Track(track) => {
          let duration = track.duration.num_milliseconds() as u32;
          let progress = context.progress
            .map(|p| p.num_milliseconds() as u32)
            .unwrap_or(0);
          (progress, duration)
        }
        PlayableItem::Episode(episode) => {
          let duration = episode.duration.num_milliseconds() as u32;
          let progress = context.progress
            .map(|p| p.num_milliseconds() as u32)
            .unwrap_or(0);
          (progress, duration)
        }
      };
      
      let progress_perc = get_track_progress_percentage(progress_ms as u128, duration_ms);
      let progress_ratio = f64::from(progress_perc) / 100.0;
      
      let progress_bar = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default()
          .fg(app.user_config.theme.selected)
          .bg(app.user_config.theme.inactive))
        .ratio(progress_ratio);
      f.render_widget(progress_bar, progress_chunks[1]);
    }
  }
}

/// Draw large album art for idle mode
fn draw_large_album_art(f: &mut Frame, app: &App, layout_chunk: Rect) {
  if let Some(art) = &app.current_album_art {
    // Create a block for the album art
    let block = Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .border_style(Style::default().fg(app.user_config.theme.inactive));
    
    let inner_area = block.inner(layout_chunk);
    f.render_widget(block, layout_chunk);
    
    // For large display, render with double-width characters
    let mut lines = Vec::new();
    for row in &art.pixels {
      let mut line = Vec::new();
      for pixel in row {
        // Use double-width block for screensaver mode
        line.push(("██".to_string(), pixel.to_ratatui_color()));
      }
      lines.push(line);
    }
    
    // Calculate centering offsets (accounting for double-width)
    let y_offset = inner_area.height.saturating_sub(lines.len() as u16) / 2;
    let x_offset = inner_area.width.saturating_sub(art.width as u16 * 2) / 2;
    
    // Render each line of pixels
    for (y, line) in lines.iter().enumerate() {
      let y_pos = inner_area.y + y_offset + y as u16;
      if y_pos >= inner_area.y + inner_area.height {
        break;
      }
      
      for (x, (ch, color)) in line.iter().enumerate() {
        let x_pos = inner_area.x + x_offset + (x as u16 * 2);
        if x_pos + 2 > inner_area.x + inner_area.width {
          break;
        }
        
        // Render each pixel as a colored block
        let pixel = Span::styled(ch, Style::default().fg(*color));
        let paragraph = Paragraph::new(pixel);
        let pixel_area = Rect {
          x: x_pos,
          y: y_pos,
          width: 2,
          height: 1,
        };
        f.render_widget(paragraph, pixel_area);
      }
    }
  }
}
