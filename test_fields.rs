use rspotify::model::{FullTrack, SimplifiedPlaylist};

fn main() {
    // This will show us compilation errors with available fields
    let track: FullTrack = todo!();
    println!("{}", track.uri); // This will fail and show available fields
    
    let playlist: SimplifiedPlaylist = todo!();
    println!("{}", playlist.uri); // This will fail and show available fields
}