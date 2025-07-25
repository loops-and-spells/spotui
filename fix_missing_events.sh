#!/bin/bash

# Script to comment out missing IoEvent calls temporarily
# This helps get the core functionality building first

sed -i 's/IoEvent::SetTracksToTable/\/\/ IoEvent::SetTracksToTable/g' src/app.rs
sed -i 's/IoEvent::SetArtistsToTable/\/\/ IoEvent::SetArtistsToTable/g' src/app.rs
sed -i 's/IoEvent::GetFollowedArtists/\/\/ IoEvent::GetFollowedArtists/g' src/app.rs
sed -i 's/IoEvent::GetCurrentSavedTracks/\/\/ IoEvent::GetCurrentSavedTracks/g' src/app.rs
sed -i 's/IoEvent::GetCurrentUserSavedAlbums/\/\/ IoEvent::GetCurrentUserSavedAlbums/g' src/app.rs
sed -i 's/IoEvent::CurrentUserSavedAlbumDelete/\/\/ IoEvent::CurrentUserSavedAlbumDelete/g' src/app.rs
sed -i 's/IoEvent::CurrentUserSavedAlbumAdd/\/\/ IoEvent::CurrentUserSavedAlbumAdd/g' src/app.rs
sed -i 's/IoEvent::GetCurrentUserSavedShows/\/\/ IoEvent::GetCurrentUserSavedShows/g' src/app.rs
sed -i 's/IoEvent::GetCurrentShowEpisodes/\/\/ IoEvent::GetCurrentShowEpisodes/g' src/app.rs
sed -i 's/IoEvent::UserUnfollowArtists/\/\/ IoEvent::UserUnfollowArtists/g' src/app.rs
sed -i 's/IoEvent::UserFollowArtists/\/\/ IoEvent::UserFollowArtists/g' src/app.rs
sed -i 's/IoEvent::UserFollowPlaylist/\/\/ IoEvent::UserFollowPlaylist/g' src/app.rs
sed -i 's/IoEvent::UserUnfollowPlaylist/\/\/ IoEvent::UserUnfollowPlaylist/g' src/app.rs
sed -i 's/IoEvent::CurrentUserSavedShowAdd/\/\/ IoEvent::CurrentUserSavedShowAdd/g' src/app.rs
sed -i 's/IoEvent::CurrentUserSavedShowDelete/\/\/ IoEvent::CurrentUserSavedShowDelete/g' src/app.rs
sed -i 's/IoEvent::MadeForYouSearchAndAdd/\/\/ IoEvent::MadeForYouSearchAndAdd/g' src/app.rs

echo "Commented out missing IoEvent calls in app.rs"