# spotui

A Spotify terminal user interface (TUI) client written in Rust.

**⚠️ Spotify Premium is required to use this application.**

## About

spotui is a fork of the original [spotify-tui](https://github.com/Rigellute/spotify-tui) project by Alexander Keliris (Rigellute) and its contributors. The original project stopped compiling with newer Rust versions, so spotui was created to:

- Update Rust dependencies and ensure compatibility with modern toolchains
- Add missing features for a better user experience
- Fix existing bugs and improve stability

We are grateful to all the developers who contributed to spotify-tui - this project wouldn't exist without their excellent foundation.

![Demo](https://user-images.githubusercontent.com/12150276/75177190-91d4ab00-572d-11ea-80bd-c5e28c7b17ad.gif)

## Features

- Control playback (play/pause, next/previous, seek)
- Browse playlists, albums, artists, and tracks
- Search for music
- Manage devices
- View audio analysis visualizations
- Like/unlike tracks
- Follow/unfollow artists and playlists
- And much more!

## Installation

### Prerequisites

- Spotify Premium account (required for playback control)
- For a complete terminal-only experience without the official Spotify client, consider using [spotifyd](https://github.com/Spotifyd/spotifyd)

### From Source

First, install [Rust](https://www.rust-lang.org/tools/install) (using the recommended `rustup` installation method) and then:

```bash
cargo install --git https://github.com/yourusername/spotui
```

#### Note on Linux

For compilation on Linux, the development packages for `libssl` are required. See [OpenSSL installation instructions](https://docs.rs/openssl/0.10.25/openssl/#automatic). The compilation also requires `pkg-config` to be installed.

### Package Managers

Package manager support coming soon.

## Connecting to Spotify's API

spotui needs to connect to Spotify's API to function. Instructions will be shown when you first run the app, but here they are for reference:

1. Go to the [Spotify dashboard](https://developer.spotify.com/dashboard/applications)
2. Click `Create an app`
   - You now can see your `Client ID` and `Client Secret`
3. Click `Edit Settings`
4. Add `http://localhost:8888/callback` to the Redirect URIs
5. Scroll down and click `Save`
6. Run `spotui`
7. Enter your `Client ID` and `Client Secret`
8. Press enter to confirm the default port (8888) or enter a custom port
9. You will be redirected to an official Spotify webpage to ask you for permissions
10. After accepting, you'll be redirected to localhost. The URL will be parsed automatically and you're ready to go!

## Usage

The binary is named `spotui`.

Running `spotui` with no arguments will bring up the UI. Press `?` to bring up a help menu that shows currently implemented key events and their actions.

### Using with spotifyd

[spotifyd](https://github.com/Spotifyd/spotifyd) is a lightweight Spotify daemon that allows you to use spotui without having the official Spotify client running.

1. Install and start spotifyd following their documentation
2. Start spotui
3. Press `d` to go to the device selection menu - spotifyd should appear as an available device

## Configuration

Configuration files are located at:
- `${HOME}/.config/spotify-tui/config.yml` - UI configuration
- `${HOME}/.config/spotify-tui/client.yml` - Spotify authentication

### Example Configuration

```yaml
# Sample config.yml
theme:
  active: Cyan
  banner: LightCyan
  error_border: Red
  error_text: LightRed
  hint: Yellow
  hovered: Magenta
  inactive: Gray
  playbar_background: Black
  playbar_progress: LightCyan
  playbar_progress_text: Cyan
  playbar_text: White
  selected: LightCyan
  text: "255, 255, 255"
  header: White

behavior:
  seek_milliseconds: 5000
  volume_increment: 10
  tick_rate_milliseconds: 250
  enable_text_emphasis: true
  show_loading_indicator: true
  enforce_wide_search_bar: false
  liked_icon: ♥
  shuffle_icon: 🔀
  repeat_track_icon: 🔂
  repeat_context_icon: 🔁
  playing_icon: ▶
  paused_icon: ⏸
  set_window_title: true

keybindings:
  back: "ctrl-q"
  jump_to_album: "a"
  jump_to_artist_album: "A"
  manage_devices: "d"
  decrease_volume: "-"
  increase_volume: "+"
  toggle_playback: " "
  seek_backwards: "<"
  seek_forwards: ">"
  next_track: "n"
  previous_track: "p"
  copy_song_url: "c"
  copy_album_url: "C"
  help: "?"
  shuffle: "ctrl-s"
  repeat: "r"
  search: "/"
  audio_analysis: "v"
  jump_to_context: "o"
  basic_view: "B"
  add_item_to_queue: "z"
```

## Libraries Used

- [tui-rs](https://github.com/fdehau/tui-rs) - Terminal UI framework
- [rspotify](https://github.com/ramsayleung/rspotify) - Spotify Web API wrapper

## Development

1. [Install Rust](https://www.rust-lang.org/tools/install)
2. Clone this repository
3. Run `cargo run` to start the application in development mode

## License

MIT - See LICENSE file for details

## Acknowledgments

This project is based on the excellent work done by the spotify-tui team. Special thanks to Alexander Keliris (Rigellute) and all contributors to the original project.