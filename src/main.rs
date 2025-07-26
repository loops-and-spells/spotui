mod app;
mod banner;
// mod cli;  // TODO: Re-enable after fixing clap compatibility
mod config;
mod event;
mod focus_manager;
mod handlers;
mod network;  // Temporary minimal network module
mod redirect_uri;
mod ui;
mod user_config;

use crate::app::RouteId;
use crate::event::Key;
use anyhow::{anyhow, Result};
use app::{ActiveBlock, App};
use backtrace::Backtrace;
use banner::BANNER;
use clap::{Arg, Command};
use config::ClientConfig;
use crossterm::{
  cursor::MoveTo,
  event::{DisableMouseCapture, EnableMouseCapture},
  execute,
  style::Print,
  terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
  },
  ExecutableCommand,
};
use network::{IoEvent, Network};
// use redirect_uri::redirect_uri_web_server;  // TODO: Fix redirect_uri module
use rspotify::{
  AuthCodeSpotify, Credentials, OAuth, Token,
  prelude::*,
};
use webbrowser;
use std::{
  cmp::{max, min},
  io::{self, stdout},
  panic::{self, PanicInfo},
  path::PathBuf,
  sync::Arc,
  time::SystemTime,
};
use tokio::sync::Mutex;
use ratatui::{
  backend::{Backend, CrosstermBackend},
  layout::Rect,
  Terminal,
};
use user_config::{UserConfig, UserConfigPaths};

fn get_scopes() -> std::collections::HashSet<String> {
  [
    "playlist-read-collaborative",
    "playlist-read-private",
    "playlist-modify-private", 
    "playlist-modify-public",
    "user-follow-read",
    "user-follow-modify",
    "user-library-modify",
    "user-library-read",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "user-read-playback-state",
    "user-read-playback-position",
    "user-read-private",
    "user-read-recently-played",
    "user-top-read",
  ].iter().map(|s| s.to_string()).collect()
}

/// Create Spotify client with rspotify 0.15 API
pub async fn create_spotify_client(client_config: &ClientConfig) -> Result<AuthCodeSpotify> {
  let creds = Credentials::new(&client_config.client_id, &client_config.client_secret);
  
  let oauth = OAuth {
    redirect_uri: client_config.get_redirect_uri(),
    scopes: get_scopes(),
    ..Default::default()
  };
  
  let paths = client_config.get_or_build_paths()?;
  let cache_path = paths.token_cache_path.clone();
  
  // Set the environment variable for rspotify's token cache
  std::env::set_var("RSPOTIFY_CACHE_PATH", cache_path.to_str().unwrap_or(""));
  
  let mut spotify = AuthCodeSpotify::new(creds, oauth);
  
  // Try to load cached token first
  if cache_path.exists() {
    println!("Checking for cached token at: {:?}", cache_path);
    // Read the token file manually
    match std::fs::read_to_string(&cache_path) {
      Ok(token_json) => {
        use rspotify::Token;
        match serde_json::from_str::<Token>(&token_json) {
          Ok(token) => {
            *spotify.token.lock().await.unwrap() = Some(token);
            println!("Loaded cached authentication token");
            // Verify token is still valid
            match spotify.current_user().await {
              Ok(_) => {
                println!("Token is valid, skipping authentication");
                return Ok(spotify);
              }
              Err(_) => {
                println!("Token expired, need to re-authenticate");
              }
            }
          }
          Err(e) => {
            println!("Failed to parse cached token: {}", e);
          }
        }
      }
      Err(e) => {
        println!("Failed to read token cache file: {}", e);
      }
    }
  } else {
    println!("No token cache file found at: {:?}", cache_path);
  }
  
  // Perform OAuth flow
  println!("Opening Spotify authorization page in your browser...");
  
  // Get authorization URL
  let auth_url = spotify.get_authorize_url(false).unwrap();
  
  // Try to open the URL in browser
  if let Err(_) = webbrowser::open(&auth_url) {
    println!("Failed to open browser automatically.");
    println!("Please open this URL manually: {}", auth_url);
  }
  
  // Start local server to capture redirect
  use crate::redirect_uri::redirect_uri_web_server_modern;
  let redirect_url = redirect_uri_web_server_modern(client_config.get_port())?;
  
  // Extract authorization code from redirect URL
  let code = extract_code_from_url(&redirect_url)?;
  
  // Exchange code for token
  spotify.request_token(&code).await?;
  
  // Cache the token
  println!("Caching token to: {:?}", paths.token_cache_path);
  
  // Get the token and write it manually
  if let Ok(token_guard) = spotify.token.lock().await {
    if let Some(token) = token_guard.as_ref() {
      match serde_json::to_string_pretty(token) {
        Ok(token_json) => {
          match std::fs::write(&paths.token_cache_path, token_json) {
            Ok(_) => {
              println!("Authentication successful! Token cached for future use.");
            }
            Err(e) => {
              println!("Warning: Failed to write token cache file: {}", e);
            }
          }
        }
        Err(e) => {
          println!("Warning: Failed to serialize token: {}", e);
        }
      }
    } else {
      println!("Warning: No token to cache");
    }
  }
  
  Ok(spotify)
}

/// Extract authorization code from Spotify redirect URL
fn extract_code_from_url(url: &str) -> Result<String> {
  if let Some(code_start) = url.find("code=") {
    let code_start = code_start + 5; // Skip "code="
    let code_end = url[code_start..]
      .find('&')
      .map(|i| code_start + i)
      .unwrap_or(url.len());
    
    let code = &url[code_start..code_end];
    if code.is_empty() {
      return Err(anyhow!("Empty authorization code in URL"));
    }
    
    Ok(code.to_string())
  } else {
    Err(anyhow!("No authorization code found in redirect URL: {}", url))
  }
}

fn close_application() -> Result<()> {
  disable_raw_mode()?;
  let mut stdout = io::stdout();
  execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
  Ok(())
}

fn panic_hook(info: &PanicInfo<'_>) {
  if cfg!(debug_assertions) {
    let location = info.location().unwrap();

    let msg = match info.payload().downcast_ref::<&'static str>() {
      Some(s) => *s,
      None => match info.payload().downcast_ref::<String>() {
        Some(s) => &s[..],
        None => "Box<Any>",
      },
    };

    let stacktrace: String = format!("{:?}", Backtrace::new()).replace('\n', "\n\r");

    disable_raw_mode().unwrap();
    execute!(
      io::stdout(),
      LeaveAlternateScreen,
      Print(format!(
        "thread '<unnamed>' panicked at '{}', {}\n\r{}",
        msg, location, stacktrace
      )),
      DisableMouseCapture
    )
    .unwrap();
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  panic::set_hook(Box::new(|info| {
    panic_hook(info);
  }));

  let mut clap_app = Command::new(env!("CARGO_PKG_NAME"))
    .version(env!("CARGO_PKG_VERSION"))
    .author(env!("CARGO_PKG_AUTHORS"))
    .about(env!("CARGO_PKG_DESCRIPTION"))
    .after_help("Press `?` while running the app to see keybindings")
    .before_help(BANNER)
    .after_help(
      "Your spotify Client ID and Client Secret are stored in $HOME/.config/spotify-tui/client.yml",
    )
    .arg(
      Arg::new("tick-rate")
        .short('t')
        .long("tick-rate")
        .help("Set the tick rate (milliseconds): the lower the number the higher the FPS.")
        .long_help(
          "Specify the tick rate in milliseconds: the lower the number the \
higher the FPS. It can be nicer to have a lower value when you want to use the audio analysis view \
of the app. Beware that this comes at a CPU cost!",
        )
        .takes_value(true),
    )
    .arg(
      Arg::new("config")
        .short('c')
        .long("config")
        .help("Specify configuration file path.")
        .takes_value(true),
    )
    .arg(
      Arg::new("completions")
        .long("completions")
        .help("Generates completions for your preferred shell")
        .takes_value(true)
        .possible_values(&["bash", "zsh", "fish", "power-shell", "elvish"])
        .value_name("SHELL"),
    )
    // Control spotify from the command line
    // TODO: Re-enable CLI commands after fixing clap compatibility
    // .subcommand(cli::playback_subcommand())
    // .subcommand(cli::play_subcommand())
    // .subcommand(cli::list_subcommand())
    // .subcommand(cli::search_subcommand())
    ;

  let matches = clap_app.clone().get_matches();

  // Shell completions don't need any spotify work
  // TODO: Fix shell completions with proper clap_generate integration
  // if let Some(s) = matches.value_of("completions") {
  //   let shell = match s {
  //     "fish" => Shell::Fish,
  //     "bash" => Shell::Bash,
  //     "zsh" => Shell::Zsh,
  //     "power-shell" => Shell::PowerShell,
  //     "elvish" => Shell::Elvish,
  //     _ => return Err(anyhow!("no completions avaible for '{}'", s);
  //   };
  //   clap_app.gen_completions_to("spt", shell, &mut io::stdout());
  //   return Ok(());
  // }

  let mut user_config = UserConfig::new();
  if let Some(config_file_path) = matches.get_one::<String>("config") {
    let config_file_path = PathBuf::from(config_file_path);
    let path = UserConfigPaths { config_file_path };
    user_config.path_to_config.replace(path);
  }
  user_config.load_config()?;

  if let Some(tick_rate) = matches
    .get_one::<String>("tick-rate")
    .and_then(|tick_rate| tick_rate.parse().ok())
  {
    if tick_rate >= 1000 {
      panic!("Tick rate must be below 1000");
    } else {
      user_config.behavior.tick_rate_milliseconds = tick_rate;
    }
  }

  let mut client_config = ClientConfig::new();
  client_config.load_config()?;

  let config_paths = client_config.get_or_build_paths()?;

  // Start authorization with spotify
  match create_spotify_client(&client_config).await {
    Ok(spotify) => {
      let (sync_io_tx, sync_io_rx) = std::sync::mpsc::channel::<IoEvent>();

      // Get token expiry from the authenticated client
      let token_expiry = if let Ok(token_guard) = spotify.token.lock().await {
        if let Some(token) = token_guard.as_ref() {
          if let Some(expires_at) = token.expires_at {
            expires_at.into()
          } else {
            SystemTime::now() + std::time::Duration::from_secs(3600)
          }
        } else {
          SystemTime::now() + std::time::Duration::from_secs(3600)
        }
      } else {
        SystemTime::now() + std::time::Duration::from_secs(3600)
      };

      // Initialise app state
      let app = Arc::new(Mutex::new(App::new(
        sync_io_tx.clone(),
        user_config.clone(),
        token_expiry,
      )));

      // Add startup log message
      {
        let mut app_lock = app.lock().await;
        app_lock.add_log_message("Spotify TUI started - checking current device...".to_string());
        app_lock.add_log_message("Tip: Press 'd' to select a playback device".to_string());
      }

      // Check current playback context on startup
      if let Err(_) = sync_io_tx.send(IoEvent::GetCurrentPlayback) {
        // Failed to dispatch initial playback check
      }

      // Start network handler in background thread  
      let app_clone = Arc::clone(&app);
      let spotify_clone = spotify.clone();
      std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
          let mut network = Network::new(spotify_clone, client_config, &app_clone);
          start_tokio(sync_io_rx, &mut network).await;
        });
      });

      // Launch the UI
      start_ui(user_config, &app).await?;
    }
    Err(e) => {
      println!("\nSpotify authentication failed: {}", e);
      return Err(e);
    }
  }

  Ok(())
}

async fn start_tokio(io_rx: std::sync::mpsc::Receiver<IoEvent>, network: &mut Network) {
  while let Ok(io_event) = io_rx.recv() {
    network.handle_network_event(io_event).await;
  }
}

async fn start_ui(user_config: UserConfig, app: &Arc<Mutex<App>>) -> Result<()> {
  // Terminal initialization
  let mut stdout = stdout();
  execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
  enable_raw_mode()?;

  let mut backend = CrosstermBackend::new(stdout);

  if user_config.behavior.set_window_title {
    backend.execute(SetTitle("spt - Spotify TUI"))?;
  }

  let mut terminal = Terminal::new(backend)?;
  terminal.hide_cursor()?;

  let events = event::Events::new(user_config.behavior.tick_rate_milliseconds);

  // play music on, if not send them to the device selection view

  let mut is_first_render = true;

  loop {
    let mut app = app.lock().await;
    // Get the size of the screen on each loop to account for resize event
    if let Ok(size) = terminal.backend().size() {
      let size_rect = Rect::new(0, 0, size.width, size.height);
      // Reset the help menu is the terminal was resized
      if is_first_render || app.size != size_rect {

        app.size = size_rect;

        // Based on the size of the terminal, adjust the search limit.
        let potential_limit = max((app.size.height as i32) - 13, 0) as u32;
        let max_limit = min(potential_limit, 50);
        let large_search_limit = min((f32::from(size.height) / 1.4) as u32, max_limit);
        let small_search_limit = min((f32::from(size.height) / 2.85) as u32, max_limit / 2);

        app.dispatch(IoEvent::UpdateSearchLimits(
          large_search_limit,
          small_search_limit,
        ));

      }
    };

    let current_route = app.get_current_route();
    terminal.draw(|mut f| match current_route.active_block {
      ActiveBlock::SelectDevice => {
        ui::draw_device_list(&mut f, &app);
      }
      ActiveBlock::Analysis => {
        ui::audio_analysis::draw(&mut f, &app);
      }
      ActiveBlock::BasicView => {
        ui::draw_basic_view(&mut f, &app);
      }
      ActiveBlock::LogStream => {
        ui::draw_log_stream_full_screen(&mut f, &app);
      }
      _ => {
        ui::draw_main_layout(&mut f, &app);
      }
    })?;

    if current_route.active_block == ActiveBlock::Input {
      terminal.show_cursor()?;
    } else {
      terminal.hide_cursor()?;
    }

    let cursor_offset = if app.size.height > ui::util::SMALL_TERMINAL_HEIGHT {
      2
    } else {
      1
    };

    // Put the cursor back inside the input box only if Input is active
    if app.get_current_route().active_block == ActiveBlock::Input {
      terminal.backend_mut().execute(MoveTo(
        cursor_offset + app.input_cursor_position,
        cursor_offset,
      ))?;
    }

    // Handle authentication refresh
    if SystemTime::now() > app.spotify_token_expiry {
      app.dispatch(IoEvent::RefreshAuthentication);
    }

    match events.next()? {
      event::Event::Input(key) => {
        if key == Key::Ctrl('c') {
          break;
        }

        let current_active_block = app.get_current_route().active_block;

        // To avoid swallowing global key presses make a special
        // case for the input handler
        if current_active_block == ActiveBlock::Input {
          handlers::input_handler(key, &mut app);
        } else if key == app.user_config.keys.back {
          if app.get_current_route().active_block != ActiveBlock::Input {
            // Go back through navigation stack when not in search input mode
            // NOTE: Unlike before, we do NOT exit the app - only Ctrl-C should do that
            let _pop_result = match app.pop_navigation_stack() {
              Some(ref x) if x.id == RouteId::Search => app.pop_navigation_stack(),
              Some(x) => Some(x),
              None => None,
            };
            // Removed: if pop_result.is_none() { break; } - no longer exit on 'q'
          }
        } else {
          handlers::handle_app(key, &mut app);
        }
      }
      event::Event::Tick => {
        app.update_on_tick();
      }
    }

    // Delay spotify request until first render, will have the effect of improving
    // startup speed
    if is_first_render {
      app.dispatch(IoEvent::GetPlaylists);
      app.dispatch(IoEvent::GetUser);
      app.dispatch(IoEvent::GetCurrentPlayback);
      app.dispatch(IoEvent::GetDevices);

      is_first_render = false;
    }
  }

  terminal.show_cursor()?;
  close_application()?;

  Ok(())
}
