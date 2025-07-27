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
  create_artist_string, get_artist_highlight_state, get_color,
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
  // Calculate playbar height dynamically based on terminal height
  let playbar_height = (f.area().height / 5).max(6).min(14);
  
  if app.size.width >= SMALL_TERMINAL_WIDTH && !app.user_config.behavior.enforce_wide_search_bar {
    let parent_layout = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Min(1), Constraint::Length(playbar_height)].as_ref())
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
          Constraint::Length(playbar_height),
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
  // Get dynamic colors from album art if available
  let (vibrant_color, dark_color) = if let Some(art) = &app.current_album_art {
    get_album_art_colors(art)
  } else {
    (Color::Cyan, Color::DarkGray)
  };
  
  // Calculate square album art size based on playbar height
  // The album art should fill the entire height minus borders
  let inner_height = layout_chunk.height.saturating_sub(2); // Account for borders
  // Since terminal characters are ~2:1 ratio, we need double the width for square appearance
  // Add a bit more width to ensure the art can fill the full height
  let album_art_width = (inner_height * 2) + 2;
  
  // First split horizontally to make room for album art
  let constraints: &[Constraint] = if app.current_album_art.is_some() {
    &[Constraint::Length(album_art_width), Constraint::Min(1)]
  } else {
    &[Constraint::Min(1)]
  };
  
  let horizontal_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints(constraints)
    .split(layout_chunk);

  // If we have album art, draw it in the left chunk
  if app.current_album_art.is_some() {
    draw_album_art_dynamic(f, app, horizontal_chunks[0]);
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
        Constraint::Min(1),          // Track info takes remaining space
        Constraint::Length(3),       // Progress bar is 3 units tall
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

      let title_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
          &title,
          Style::default().fg(app.user_config.theme.inactive),
        ))
        .border_style(Style::default().fg(app.user_config.theme.inactive));

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

      // Create play control buttons layout - two rows
      let control_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
          Constraint::Percentage(50), // Main controls
          Constraint::Percentage(50), // Secondary controls
        ].as_ref())
        .split(chunks[0]);

      // Top row: Previous, Play/Pause, Next
      let top_controls = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
          Constraint::Percentage(33), // Previous
          Constraint::Percentage(34), // Play/Pause
          Constraint::Percentage(33), // Next
        ].as_ref())
        .split(control_rows[0]);

      // Bottom row: Seek Back, Shuffle, Repeat, Seek Forward
      let bottom_controls = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
          Constraint::Percentage(25), // Seek back
          Constraint::Percentage(25), // Shuffle
          Constraint::Percentage(25), // Repeat
          Constraint::Percentage(25), // Seek forward
        ].as_ref())
        .split(control_rows[1]);

      // Previous button
      let prev_button = Paragraph::new("⏮")
        .style(Style::default().fg(app.user_config.theme.playbar_text))
        .alignment(Alignment::Center)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
              "B",
              Style::default().fg(app.user_config.theme.inactive),
            ))
        );
      f.render_widget(prev_button, top_controls[0]);

      // Play/Pause button
      let play_pause_icon = if current_playback_context.is_playing { "⏸" } else { "▶" };
      let play_pause_color = if current_playback_context.is_playing {
        app.user_config.theme.active
      } else {
        app.user_config.theme.playbar_text
      };
      let play_pause_button = Paragraph::new(play_pause_icon)
        .style(Style::default().fg(play_pause_color))
        .alignment(Alignment::Center)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
              "SPACE",
              Style::default().fg(app.user_config.theme.inactive),
            ))
            .border_style(Style::default().fg(app.user_config.theme.inactive))
        );
      f.render_widget(play_pause_button, top_controls[1]);

      // Next button
      let next_button = Paragraph::new("⏭")
        .style(Style::default().fg(app.user_config.theme.playbar_text))
        .alignment(Alignment::Center)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
              "N",
              Style::default().fg(app.user_config.theme.inactive),
            ))
        );
      f.render_widget(next_button, top_controls[2]);

      // Shuffle button
      let shuffle_active = current_playback_context.shuffle_state;
      let shuffle_color = if shuffle_active {
        app.user_config.theme.active
      } else {
        app.user_config.theme.playbar_text
      };
      let shuffle_border_color = if shuffle_active {
        app.user_config.theme.active
      } else {
        app.user_config.theme.inactive
      };
      let shuffle_button = Paragraph::new("🔀")
        .style(Style::default().fg(shuffle_color))
        .alignment(Alignment::Center)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .border_type(if shuffle_active { BorderType::Double } else { BorderType::Rounded })
            .title(Span::styled(
              "CTRL+S",
              Style::default().fg(app.user_config.theme.inactive),
            ))
            .border_style(Style::default().fg(shuffle_border_color))
        );
      f.render_widget(shuffle_button, bottom_controls[1]);

      // Repeat button
      let repeat_icon = match current_playback_context.repeat_state {
        SpotifyRepeatState::Track => "🔂",
        SpotifyRepeatState::Context => "🔁",
        SpotifyRepeatState::Off => "🔁",
      };
      let repeat_active = !matches!(current_playback_context.repeat_state, SpotifyRepeatState::Off);
      let repeat_color = if repeat_active {
        app.user_config.theme.active
      } else {
        app.user_config.theme.playbar_text
      };
      let repeat_border_color = if repeat_active {
        app.user_config.theme.active
      } else {
        app.user_config.theme.inactive
      };
      let repeat_button = Paragraph::new(repeat_icon)
        .style(Style::default().fg(repeat_color))
        .alignment(Alignment::Center)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .border_type(if repeat_active { BorderType::Double } else { BorderType::Rounded })
            .title(Span::styled(
              "CTRL+R",
              Style::default().fg(app.user_config.theme.inactive),
            ))
            .border_style(Style::default().fg(repeat_border_color))
        );
      f.render_widget(repeat_button, bottom_controls[2]);

      // Seek backward button
      let seek_back_button = Paragraph::new("◀◀")
        .style(Style::default().fg(app.user_config.theme.playbar_text))
        .alignment(Alignment::Center)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
              "<",
              Style::default().fg(app.user_config.theme.inactive),
            ))
        );
      f.render_widget(seek_back_button, bottom_controls[0]);

      // Seek forward button
      let seek_forward_button = Paragraph::new("▶▶")
        .style(Style::default().fg(app.user_config.theme.playbar_text))
        .alignment(Alignment::Center)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
              ">",
              Style::default().fg(app.user_config.theme.inactive),
            ))
        );
      f.render_widget(seek_forward_button, bottom_controls[3]);

      let progress_ms = match app.seek_ms {
        Some(seek_ms) => seek_ms,
        None => app.song_progress_ms,
      };

      let perc = get_track_progress_percentage(progress_ms, duration_ms.num_milliseconds() as u32);

      // Create the label text with track name and artist, similar to fullscreen mode
      let progress_label = format!("{} - {}", 
        track_name, 
        play_bar_text
      );
      
      // Calculate progress ratio for the gauge
      let progress_ratio = f64::from(perc) / 100.0;
      
      // Calculate text color with good contrast against the progress bar
      let text_color = calculate_text_color_for_progress(vibrant_color, dark_color);
      
      let song_progress = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default()
          .fg(vibrant_color)
          .bg(dark_color))
        .ratio(progress_ratio)
        .label(Span::styled(
          progress_label,
          Style::default().fg(text_color).add_modifier(Modifier::BOLD),
        ));
      // Add horizontal margin to the progress bar
      let progress_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
          Constraint::Min(0),      // Left side takes remaining space
          Constraint::Length(1),   // Right margin of 1 unit
        ].as_ref())
        .split(chunks[1]);
      
      f.render_widget(song_progress, progress_area[0]);
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

/// Darken a color by reducing its brightness
fn darken_color(color: Color, factor: f32) -> Color {
  match color {
    Color::Rgb(r, g, b) => {
      let darkened_r = (r as f32 * factor).max(0.0).min(255.0) as u8;
      let darkened_g = (g as f32 * factor).max(0.0).min(255.0) as u8;
      let darkened_b = (b as f32 * factor).max(0.0).min(255.0) as u8;
      Color::Rgb(darkened_r, darkened_g, darkened_b)
    }
    _ => color,
  }
}

/// Lighten a color by increasing its brightness
fn lighten_color(color: Color, factor: f32) -> Color {
  match color {
    Color::Rgb(r, g, b) => {
      let lightened_r = (r as f32 * factor).max(0.0).min(255.0) as u8;
      let lightened_g = (g as f32 * factor).max(0.0).min(255.0) as u8;
      let lightened_b = (b as f32 * factor).max(0.0).min(255.0) as u8;
      Color::Rgb(lightened_r, lightened_g, lightened_b)
    }
    _ => color,
  }
}

/// Convert HSL to RGB color
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
  let h = h / 360.0;
  let r;
  let g;
  let b;

  if s == 0.0 {
    r = l;
    g = l;
    b = l;
  } else {
    let hue2rgb = |p: f32, q: f32, t: f32| -> f32 {
      let mut t = t;
      if t < 0.0 { t += 1.0; }
      if t > 1.0 { t -= 1.0; }
      if t < 1.0/6.0 { return p + (q - p) * 6.0 * t; }
      if t < 1.0/2.0 { return q; }
      if t < 2.0/3.0 { return p + (q - p) * (2.0/3.0 - t) * 6.0; }
      p
    };

    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    r = hue2rgb(p, q, h + 1.0/3.0);
    g = hue2rgb(p, q, h);
    b = hue2rgb(p, q, h - 1.0/3.0);
  }

  ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Blend two colors together
fn blend_colors(color1: Color, color2: Color, factor: f32) -> Color {
  match (color1, color2) {
    (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
      let r = (r1 as f32 * (1.0 - factor) + r2 as f32 * factor) as u8;
      let g = (g1 as f32 * (1.0 - factor) + g2 as f32 * factor) as u8;
      let b = (b1 as f32 * (1.0 - factor) + b2 as f32 * factor) as u8;
      Color::Rgb(r, g, b)
    }
    _ => color1,
  }
}

/// Extract vibrant and dark colors from album art
fn get_album_art_colors(art: &crate::album_art::PixelatedAlbumArt) -> (Color, Color) {
  let mut darkest_color = art.pixels[0][0].to_ratatui_color();
  let mut min_brightness = u32::MAX;
  let mut vibrant_color = art.pixels[0][0].to_ratatui_color();
  let mut max_vibrancy = 0.0;
  
  for row in &art.pixels {
    for pixel in row {
      // Calculate brightness (simple sum of RGB values)
      let brightness = pixel.r as u32 + pixel.g as u32 + pixel.b as u32;
      if brightness < min_brightness {
        min_brightness = brightness;
        darkest_color = pixel.to_ratatui_color();
      }
      
      // Calculate vibrancy (saturation * brightness)
      let r = pixel.r as f32 / 255.0;
      let g = pixel.g as f32 / 255.0;
      let b = pixel.b as f32 / 255.0;
      
      let max_component = r.max(g).max(b);
      let min_component = r.min(g).min(b);
      let saturation = if max_component > 0.0 {
        (max_component - min_component) / max_component
      } else {
        0.0
      };
      
      // Vibrancy is a combination of saturation and brightness
      // We want colors that are both bright and saturated
      let vibrancy = saturation * max_component;
      
      if vibrancy > max_vibrancy && brightness > 100 { // Ensure it's not too dark
        max_vibrancy = vibrancy;
        vibrant_color = pixel.to_ratatui_color();
      }
    }
  }
  
  // Ensure good contrast between foreground and background colors
  let (vibrant_color, darkest_color) = ensure_color_contrast(vibrant_color, darkest_color);
  
  (vibrant_color, darkest_color)
}

/// Ensure sufficient contrast between two colors for progress bar visibility
fn ensure_color_contrast(fg: Color, bg: Color) -> (Color, Color) {
  // Get RGB values for both colors
  let (fg_r, fg_g, fg_b) = match fg {
    Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
    _ => (128.0, 128.0, 128.0), // Default fallback
  };
  
  let (bg_r, bg_g, bg_b) = match bg {
    Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
    _ => (64.0, 64.0, 64.0), // Default fallback
  };
  
  // Calculate relative luminance using WCAG formula
  let fg_lum = (0.2126 * fg_r + 0.7152 * fg_g + 0.0722 * fg_b) / 255.0;
  let bg_lum = (0.2126 * bg_r + 0.7152 * bg_g + 0.0722 * bg_b) / 255.0;
  
  // Calculate contrast ratio
  let lighter = fg_lum.max(bg_lum);
  let darker = fg_lum.min(bg_lum);
  let contrast_ratio = (lighter + 0.05) / (darker + 0.05);
  
  // If contrast is insufficient (less than 3:1), adjust colors
  if contrast_ratio < 3.0 {
    // If foreground is lighter, make it even lighter; if darker, make it darker
    let adjusted_fg = if fg_lum > bg_lum {
      // Lighten the foreground
      Color::Rgb(
        ((fg_r * 1.5).min(255.0)) as u8,
        ((fg_g * 1.5).min(255.0)) as u8,
        ((fg_b * 1.5).min(255.0)) as u8,
      )
    } else {
      // Darken the background instead and lighten foreground
      let adjusted_bg = Color::Rgb(
        ((bg_r * 0.5).max(0.0)) as u8,
        ((bg_g * 0.5).max(0.0)) as u8,
        ((bg_b * 0.5).max(0.0)) as u8,
      );
      return (fg, adjusted_bg);
    };
    (adjusted_fg, bg)
  } else {
    (fg, bg)
  }
}

/// Calculate optimal text color for progress bar that contrasts with both filled and unfilled portions
fn calculate_text_color_for_progress(fg_color: Color, bg_color: Color) -> Color {
  // Get luminance of both colors
  let fg_lum = match fg_color {
    Color::Rgb(r, g, b) => {
      (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0
    }
    _ => 0.5,
  };
  
  let bg_lum = match bg_color {
    Color::Rgb(r, g, b) => {
      (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0
    }
    _ => 0.2,
  };
  
  // Calculate average luminance since text will appear over both
  let avg_lum = (fg_lum + bg_lum) / 2.0;
  
  // Choose white or black based on which provides better contrast
  if avg_lum < 0.5 {
    Color::White
  } else {
    Color::Black
  }
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

/// Draw album art with dynamic sizing to fill available space
fn draw_album_art_dynamic(f: &mut Frame, app: &App, layout_chunk: Rect) {
  if let Some(art) = &app.current_album_art {
    // Create a block for the album art
    let block = Block::default()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .border_style(Style::default().fg(app.user_config.theme.inactive));
    
    let inner_area = block.inner(layout_chunk);
    f.render_widget(block, layout_chunk);
    
    // Calculate the maximum size that maintains square aspect ratio
    // For the playbar, we want to use all available height
    let available_height = inner_area.height;
    let available_width = inner_area.width / 2; // Divide by 2 for double-width chars
    
    // Use the full height available, constrained by width for square aspect
    let display_size = available_height.min(available_width);
    
    // Center horizontally only, align to top to fill vertical space
    let x_offset = (inner_area.width.saturating_sub(display_size * 2)) / 2;
    let y_offset = 0; // No vertical offset - fill from top to bottom
    
    // Scale factor from source to display
    let scale_x = art.width as f32 / display_size as f32;
    let scale_y = art.height as f32 / display_size as f32;
    
    // Render scaled pixels
    for dy in 0..display_size {
      let y_pos = inner_area.y + y_offset + dy;
      if y_pos >= inner_area.y + inner_area.height {
        break;
      }
      
      // Map display y to source y
      let src_y = (dy as f32 * scale_y) as usize;
      if src_y >= art.pixels.len() {
        continue;
      }
      
      for dx in 0..display_size {
        let x_pos = inner_area.x + x_offset + (dx * 2); // Double width for square appearance
        if x_pos + 1 >= inner_area.x + inner_area.width {
          break;
        }
        
        // Map display x to source x
        let src_x = (dx as f32 * scale_x) as usize;
        if src_x >= art.pixels[src_y].len() {
          continue;
        }
        
        let pixel = &art.pixels[src_y][src_x];
        let color = pixel.to_ratatui_color();
        
        // Use double-width block for square appearance
        let pixel_span = Span::styled("██", Style::default().fg(color));
        let paragraph = Paragraph::new(pixel_span);
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

/// Draw the idle mode screensaver with large album art
pub fn draw_idle_mode(f: &mut Frame, app: &App) {
  // No border in fullscreen mode - use the entire area
  let area = f.area();
  
  // Reserve bottom space for track info and progress
  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Min(1),      // Album art area (takes remaining space)
      Constraint::Length(3),   // Progress bar with track info (3 lines tall)
    ].as_ref())
    .split(area);

  // Draw fullscreen album art and get dynamic colors
  let (vibrant_color, dark_color) = if app.current_album_art.is_some() {
    match app.idle_animation {
      crate::app::IdleAnimation::SpinningRecord => draw_fullscreen_album_art(f, app, chunks[0]),
      crate::app::IdleAnimation::CoinFlip => draw_coin_flip_album_art(f, app, chunks[0]),
    }
  } else {
    (Color::Cyan, Color::DarkGray)
  };

  // Draw track info and progress bar at the bottom
  if let Some(context) = &app.current_playback_context {
    if let Some(item) = &context.item {
      // Get track info
      let track_info = match item {
        PlayableItem::Track(track) => {
          format!("{} - {}", track.name, create_artist_string(&track.artists))
        }
        PlayableItem::Episode(episode) => episode.name.clone(),
      };

      // Calculate progress
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
      
      // Use a single 3-line tall progress bar that spans the full width
      let progress_area = chunks[1];
      
      // Calculate text color with good contrast against the progress bar
      // We need to consider both filled and unfilled portions
      let text_color = calculate_text_color_for_progress(vibrant_color, dark_color);
      
      let progress_bar = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default()
          .fg(vibrant_color)
          .bg(lighten_color(dark_color, 1.3)))
        .ratio(progress_ratio)
        .label(Span::styled(
          track_info,
          Style::default().fg(text_color).add_modifier(Modifier::BOLD),
        ));
      f.render_widget(progress_bar, progress_area);
    }
  }
}

/// Draw fullscreen album art that fills the available space
fn draw_fullscreen_album_art(f: &mut Frame, app: &App, layout_chunk: Rect) -> (Color, Color) {
  if let Some(art) = &app.current_album_art {
    // Get dynamic colors from the album art
    let (vibrant_color, darkest_color) = get_album_art_colors(art);
    
    // Make the background darker than the picked color but not too dark
    let darker_background = darken_color(darkest_color, 0.5); // 50% brightness - not too dark
    
    // Fill the entire background with the darker color
    let background = Block::default()
      .style(Style::default().bg(darker_background));
    f.render_widget(background, layout_chunk);
    
    // Calculate the maximum size we can display
    // Account for double-width characters (2:1 aspect ratio)
    // Reduce size to make room for shadow and spacing from playbar
    let shadow_space = 4; // Space needed for shadow (2 pixels) + gap from playbar (2 pixels)
    let available_width = (layout_chunk.width / 2).saturating_sub(shadow_space);  // Divide by 2 for double-width chars
    let available_height = layout_chunk.height.saturating_sub(shadow_space);
    
    // Determine the size to use (maintain square aspect ratio)
    // Cap at 100x100 to prevent performance issues on large terminals
    const MAX_RENDER_SIZE: u32 = 100;
    let display_size = available_width.min(available_height).min(MAX_RENDER_SIZE as u16) as u32;
    
    // Calculate actual pixel size based on the original art size
    let scale_factor = display_size as f32 / art.width as f32;
    
    // Center the art in the available space (accounting for shadow)
    let total_width = (display_size as u16 + 2) * 2; // +2 for shadow offset
    let total_height = display_size as u16 + 2; // +2 for shadow offset
    let x_offset = (layout_chunk.width.saturating_sub(total_width)) / 2;
    let y_offset = (layout_chunk.height.saturating_sub(total_height)) / 2;
    
    // The art itself doesn't need additional offset since we've already accounted for shadow
    let inset_x_offset = x_offset;
    let inset_y_offset = y_offset;
    
    // Get rotation angle based on real-time for smooth animation - always spinning
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let time_ms = now.as_millis();
    
    // Fast rotation for visual effect - 1 rotation per 3 seconds (20 RPM)
    // Use modulo to keep the value manageable
    let rotation_phase = (time_ms % 3000) as f32 / 3000.0;
    let rotation_angle = rotation_phase * 2.0 * std::f32::consts::PI;
    
    // Calculate center point
    let center_x = display_size as f32 / 2.0;
    let center_y = display_size as f32 / 2.0;
    let radius = display_size as f32 / 2.0;
    
    // Pre-render the entire album art into a text buffer for better performance
    let shadow_color = darken_color(darker_background, 0.7); // 70% of background brightness
    
    // Build the album art in a single buffer
    let mut lines: Vec<Vec<Span>> = Vec::with_capacity(display_size as usize);
    
    // First, pre-calculate rotations
    let cos_angle = rotation_angle.cos();
    let sin_angle = rotation_angle.sin();
    
    for y in 0..display_size {
      let mut line_spans = Vec::with_capacity(display_size as usize);
      
      for x in 0..display_size {
        // Check if pixel is within circle
        let dx = x as f32 - center_x;
        let dy = y as f32 - center_y;
        let distance = (dx * dx + dy * dy).sqrt();
        
        if distance <= radius {
          // Apply inverse rotation to find which pixel from the source should be here
          let rotated_dx = cos_angle * dx - sin_angle * dy;
          let rotated_dy = sin_angle * dx + cos_angle * dy;
          
          // Map rotated coordinates back to source image
          let src_x = ((rotated_dx + center_x) / scale_factor) as i32;
          let src_y = ((rotated_dy + center_y) / scale_factor) as i32;
          
          // Get the pixel color from the original art
          let mut color = if src_x >= 0 && src_y >= 0 && (src_y as usize) < art.pixels.len() && (src_x as usize) < art.pixels[src_y as usize].len() {
            art.pixels[src_y as usize][src_x as usize].to_ratatui_color()
          } else {
            darker_background
          };
          
          // Add center hole
          if distance < radius * 0.15 {
            color = darkest_color;
          }
          
          // Add label area (lighter circle in center)
          if distance < radius * 0.4 && distance > radius * 0.15 {
            color = match (color, vibrant_color) {
              (Color::Rgb(r, g, b), Color::Rgb(vr, vg, vb)) => Color::Rgb(
                ((r as f32 * 0.7 + vr as f32 * 0.3)) as u8,
                ((g as f32 * 0.7 + vg as f32 * 0.3)) as u8,
                ((b as f32 * 0.7 + vb as f32 * 0.3)) as u8,
              ),
              _ => color,
            };
          }
          
          // Add a visual mark to show rotation (a line from center to edge)
          let angle_to_point = dy.atan2(dx);
          // Create a thick line by checking angle difference
          let angle_diff = ((angle_to_point - rotation_angle + std::f32::consts::PI) % (2.0 * std::f32::consts::PI)) - std::f32::consts::PI;
          if angle_diff.abs() < 0.1 && distance > radius * 0.4 {
            color = Color::Red; // Red mark for visibility
          }
          
          line_spans.push(Span::styled("██", Style::default().fg(color)));
        } else {
          // Outside the circle - transparent
          line_spans.push(Span::raw("  "));
        }
      }
      
      lines.push(line_spans);
    }
    
    // Draw shadow first as a single widget
    let mut shadow_lines = Vec::new();
    for y in 0..display_size {
      let mut shadow_line = String::new();
      for x in 0..display_size {
        let dx = x as f32 - center_x;
        let dy = y as f32 - center_y;
        let distance = (dx * dx + dy * dy).sqrt();
        
        if distance <= radius {
          shadow_line.push_str("██");
        } else {
          shadow_line.push_str("  ");
        }
      }
      shadow_lines.push(Line::from(Span::styled(shadow_line, Style::default().fg(shadow_color))));
    }
    
    // Render shadow
    let shadow_paragraph = Paragraph::new(shadow_lines);
    let shadow_area = Rect {
      x: layout_chunk.x + inset_x_offset + 2,
      y: layout_chunk.y + inset_y_offset + 2,
      width: display_size as u16 * 2,
      height: display_size as u16,
    };
    f.render_widget(shadow_paragraph, shadow_area);
    
    // Convert lines to text for the album art
    let album_text: Vec<Line> = lines.into_iter()
      .map(|spans| Line::from(spans))
      .collect();
    
    // Render the entire album art as a single widget
    let album_paragraph = Paragraph::new(album_text);
    let album_area = Rect {
      x: layout_chunk.x + inset_x_offset,
      y: layout_chunk.y + inset_y_offset,
      width: display_size as u16 * 2,
      height: display_size as u16,
    };
    f.render_widget(album_paragraph, album_area);
    
    // Return the colors for progress bar theming
    (vibrant_color, darker_background)
  } else {
    // Default colors if no album art
    (Color::Cyan, Color::DarkGray)
  }
}

/// Draw coin-flip rotation album art that rotates on Y-axis
fn draw_coin_flip_album_art(f: &mut Frame, app: &App, layout_chunk: Rect) -> (Color, Color) {
  if let Some(art) = &app.current_album_art {
    // Get dynamic colors from the album art
    let (vibrant_color, darkest_color) = get_album_art_colors(art);
    
    // Make the background darker than the picked color but not too dark
    let darker_background = darken_color(darkest_color, 0.5); // 50% brightness - not too dark
    
    // Fill the entire background with the darker color
    let background = Block::default()
      .style(Style::default().bg(darker_background));
    f.render_widget(background, layout_chunk);
    
    // Calculate the maximum size we can display
    // Account for double-width characters (2:1 aspect ratio)
    // No shadow space needed for coin flip
    let available_width = layout_chunk.width / 2;  // Divide by 2 for double-width chars
    let available_height = layout_chunk.height;
    
    // Determine the size to use (maintain square aspect ratio)
    // Cap at 100x100 to prevent performance issues on large terminals
    const MAX_RENDER_SIZE: u32 = 100;
    let display_size = available_width.min(available_height).min(MAX_RENDER_SIZE as u16) as u32;
    
    // Calculate actual pixel size based on the original art size
    let scale_factor = display_size as f32 / art.width as f32;
    
    // Center the art in the available space (no shadow offset)
    let total_width = display_size as u16 * 2; // No shadow offset
    let total_height = display_size as u16; // No shadow offset
    let x_offset = (layout_chunk.width.saturating_sub(total_width)) / 2;
    let y_offset = (layout_chunk.height.saturating_sub(total_height)) / 2;
    
    // The art itself doesn't need additional offset
    let inset_x_offset = x_offset;
    let inset_y_offset = y_offset;
    
    // Get rotation angle based on real-time for smooth animation
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let time_ms = now.as_millis();
    
    // Coin flip rotation - 1 rotation per 4 seconds (15 RPM) for a more dramatic effect
    let rotation_phase = (time_ms % 4000) as f32 / 4000.0;
    let rotation_angle = rotation_phase * 2.0 * std::f32::consts::PI;
    
    // Calculate center point
    let center_x = display_size as f32 / 2.0;
    let center_y = display_size as f32 / 2.0;
    let radius = display_size as f32 / 2.0;
    
    // Pre-render the entire coin flip animation into a text buffer for better performance
    let mut lines: Vec<Vec<Span>> = Vec::with_capacity(display_size as usize);
    
    // Pre-calculate compression factor
    let compression_factor = rotation_angle.cos();
    // Show CD side when compression is negative (back half of rotation)
    let show_cd_side = compression_factor < 0.0;
    
    for y in 0..display_size {
      let mut line_spans = Vec::with_capacity(display_size as usize);
      
      for x in 0..display_size {
        // Calculate position relative to center for the compressed view
        let dx_from_center = x as f32 - center_x;
        let dy_from_center = y as f32 - center_y;
        
        // For coin flip, we need to reverse-map from screen position to disc position
        // Screen X maps to disc X through compression
        let disc_dx = dx_from_center / compression_factor.abs().max(0.01);
        let distance_from_center = (disc_dx * disc_dx + dy_from_center * dy_from_center).sqrt();
        
        // Check if this screen position maps to a point on the disc
        if distance_from_center > radius || compression_factor.abs() < 0.01 {
          // Outside the disc or edge-on
          line_spans.push(Span::raw("  "));
          continue;
        }
        
        // We're within the circle, so proceed with rendering
        let mut color = if show_cd_side {
          // Show CD back side
          let normalized_y = dy_from_center / radius;
          let normalized_x = disc_dx / radius;
          let angle_from_center = normalized_y.atan2(normalized_x);
          
          // CD base color (silver/gray)
          let base_color = Color::Rgb(205, 205, 215);
          
          // Add rainbow shimmer effect based on angle and distance
          let radial_factor = distance_from_center / radius;
          let angular_factor = angle_from_center / (2.0 * std::f32::consts::PI);
          
          // Create a rainbow that shifts with rotation angle
          let animation_intensity = (1.0_f32 + compression_factor).abs(); // 0 when flush, 1 when edge-on
          
          // Less angular influence when facing forward to reduce spiral effect
          let rotation_offset = rotation_angle / (2.0 * std::f32::consts::PI);
          let angular_influence = angular_factor * 0.5;
          let radial_influence = radial_factor * 4.0;
          
          // Base hue primarily from radial distance with subtle angular variation
          let base_hue = (radial_influence + angular_influence) * 360.0;
          
          // Add animation only when not flush - scale by animation_intensity
          let animated_offset = rotation_offset * 3.0 * animation_intensity;
          let shimmer_wave = (time_ms as f32 / 1500.0 + radial_factor * 2.0).sin() * 20.0 * animation_intensity;
          
          let hue = (base_hue + animated_offset * 360.0 + shimmer_wave) % 360.0;
          
          // Higher saturation for more vivid colors
          let saturation = 0.85 + 0.15 * (time_ms as f32 / 2000.0).sin() * animation_intensity;
          // Darker inner ring - reduce lightness near the hole
          let lightness = if radial_factor < 0.3 {
            0.35 + 0.15 * radial_factor // Darker near hole
          } else {
            0.55 + 0.15 * radial_factor // Normal brightness
          };
          
          let (r, g, b) = hsl_to_rgb(hue, saturation, lightness);
          let shimmer_color = Color::Rgb(r, g, b);
          
          // Mix base color with shimmer, stronger effect on outer edge
          let shimmer_intensity = (radial_factor * 0.7).min(1.0);
          blend_colors(base_color, shimmer_color, shimmer_intensity)
        } else {
          // Show album art side
          // We need to map from compressed screen coordinates back to original album art
          // The disc_dx already represents the uncompressed X position
          let uncompressed_x = (disc_dx + center_x) / scale_factor;
          let src_x = uncompressed_x as i32;
          let src_y = (y as f32 / scale_factor) as i32;
          
          // Get the pixel color from the original art
          if src_x >= 0 && src_y >= 0 && (src_y as usize) < art.pixels.len() && (src_x as usize) < art.pixels[src_y as usize].len() {
            art.pixels[src_y as usize][src_x as usize].to_ratatui_color()
          } else {
            darker_background
          }
        };
        
        // Add label area - lighten on both sides for consistency
        if distance_from_center < radius * 0.4 && distance_from_center > radius * 0.15 {
          // Lighten the label area on both album art and CD sides
          color = lighten_color(color, 1.2);
        }
        
        // Add tracks/grooves effect for CD side
        if show_cd_side && distance_from_center > radius * 0.4 {
          let track_pattern = ((distance_from_center - radius * 0.4) * 50.0).sin();
          if track_pattern > 0.7 {
            color = darken_color(color, 0.95);
          }
        }
        
        // Add subtle edge darkening for depth
        let edge_factor = 1.0 - (distance_from_center / radius).powf(3.0);
        color = darken_color(color, 0.9 + 0.1 * edge_factor);
        
        // Apply fresnel effect - surfaces are more reflective at glancing angles
        let fresnel_factor = 1.0 - compression_factor.abs(); // Inverted: 0 when flat, 1 when edge-on
        
        // Don't apply fresnel to the center hole area
        let apply_fresnel = distance_from_center > radius * 0.15;
        
        if apply_fresnel {
          // Apply fresnel effect differently for CD and album art sides
          if show_cd_side {
            // CD side: Increase brightness as it rotates edge-on
            let fresnel_brightness = 1.0 + (fresnel_factor * 0.4);
            color = lighten_color(color, fresnel_brightness);
            
            // Add extra shimmer when nearly edge-on
            if fresnel_factor > 0.8 {
              let edge_shimmer = ((time_ms as f32 / 500.0 + distance_from_center * 0.1).sin() + 1.0) * 0.1;
              color = lighten_color(color, 1.0 + edge_shimmer);
            }
          } else {
            // Album art side: Subtle brightening when edge-on, like a glossy surface
            let fresnel_brightness = 1.0 + (fresnel_factor * 0.2);
            color = lighten_color(color, fresnel_brightness);
          }
        }
        
        // Apply gradient that flips at edge-on position for continuous rotation
        if distance_from_center <= radius && compression_factor.abs() < 0.95 {
          // Gradient strength fades as we approach edge-on or flush
          let gradient_strength = 1.0 - compression_factor.abs().powf(2.0);
          
          if gradient_strength > 0.05 {
            let normalized_x = disc_dx / radius; // -1 to 1
            
            // Determine gradient direction based on rotation phase
            // Flip gradient at both edge-on AND flush positions
            // rotation_angle goes 0 → π → 2π
            // 0: flush (flip)
            // π/2: edge-on (flip)
            // π: flush (flip)
            // 3π/2: edge-on (flip)
            // This creates 4 quadrants with alternating gradients
            let quadrant = (rotation_angle / (std::f32::consts::PI * 0.5)) as i32;
            let flip_gradient = quadrant % 2 == 1;
            
            let shading = if flip_gradient {
              // Flipped: dark on left, light on right
              0.3 + (normalized_x + 1.0) * 0.35  // 0.3 to 1.0
            } else {
              // Normal: light on left, dark on right
              1.0 - (normalized_x + 1.0) * 0.35  // 1.0 to 0.3
            };
            
            let darkness = 1.0 - ((1.0 - shading) * gradient_strength);
            color = darken_color(color, darkness);
          }
        }
        
        // Apply lighting based on rotation angle for 3D effect
        // Commented out to make gradient more visible
        // let lighting = (rotation_angle.cos().abs() * 0.3 + 0.7).max(0.4);
        // color = darken_color(color, lighting);
        
        // Rainbow edge effect - applies to both sides
        if distance_from_center > radius * 0.92 && distance_from_center <= radius {
          let edge_intensity = (distance_from_center - radius * 0.92) / (radius * 0.08);
          // Entire edge cycles through rainbow - no position dependency
          let time_factor = (time_ms as f32 / 1000.0) % 1.0; // Cycle every second
          let edge_hue = time_factor * 360.0;
          // Add slight variation based on angle for shimmer
          let angle_variation = (dy_from_center.atan2(disc_dx) * 2.0).sin() * 30.0;
          let final_hue = (edge_hue + angle_variation) % 360.0;
          let (r, g, b) = hsl_to_rgb(final_hue, 0.9, 0.7);
          let edge_color = Color::Rgb(r, g, b);
          color = blend_colors(color, edge_color, edge_intensity * 0.9);
        }
        
        // Rainbow inner ring around the hole - back to original size but more transparent
        if distance_from_center > radius * 0.15 && distance_from_center < radius * 0.25 {
          let ring_intensity = 1.0 - ((distance_from_center - radius * 0.20) / (radius * 0.05)).abs();
          
          // Match the outer edge timing for consistency
          let time_factor = (time_ms as f32 / 800.0) % 1.0; // Faster cycle for visibility
          let ring_hue = time_factor * 360.0;
          
          // Simple clean rainbow without too much variation
          let angle = dy_from_center.atan2(disc_dx);
          let subtle_variation = (angle * 2.0).sin() * 15.0;
          let final_hue = (ring_hue + subtle_variation) % 360.0;
          
          // More visible but still transparent on CD side
          let blend_strength = if show_cd_side { 0.4 } else { 0.7 };
          let lightness = if show_cd_side { 0.5 } else { 0.65 }; // Brighter on CD side
          
          let (r, g, b) = hsl_to_rgb(final_hue, 0.9, lightness);
          let ring_color = Color::Rgb(r, g, b);
          color = blend_colors(color, ring_color, ring_intensity * blend_strength);
        }
        
        // Override with hole color AFTER all effects (should match background)
        if distance_from_center < radius * 0.15 {
          color = darker_background;
        }
        
        line_spans.push(Span::styled("██", Style::default().fg(color)));
      }
      
      lines.push(line_spans);
    }
    
    // Convert lines to text for the album art
    let album_text: Vec<Line> = lines.into_iter()
      .map(|spans| Line::from(spans))
      .collect();
    
    // Render the entire coin flip as a single widget
    let coin_paragraph = Paragraph::new(album_text);
    let coin_area = Rect {
      x: layout_chunk.x + inset_x_offset,
      y: layout_chunk.y + inset_y_offset,
      width: display_size as u16 * 2,
      height: display_size as u16,
    };
    f.render_widget(coin_paragraph, coin_area);
    
    // Return the colors for progress bar theming
    (vibrant_color, darker_background)
  } else {
    // Default colors if no album art
    (Color::Cyan, Color::DarkGray)
  }
}
