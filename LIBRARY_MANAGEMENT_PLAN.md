# Library Management Features Plan

This document outlines the plan for implementing library management features in spotify-tui, allowing users to save/unsave tracks, albums, and manage their library directly from the TUI.

## Available API Methods in rspotify 0.15

### Track Management
- `current_user_saved_tracks_add()` - Add tracks to library
- `current_user_saved_tracks_delete()` - Remove tracks from library  
- `current_user_saved_tracks_contains()` - Check if tracks are saved

### Album Management
- `current_user_saved_albums_add()` - Add albums to library
- `current_user_saved_albums_delete()` - Remove albums from library
- `current_user_saved_albums_contains()` - Check if albums are saved

### Artist Management
- `user_follow_artists()` - Follow artists
- `user_unfollow_artists()` - Unfollow artists
- `user_artist_check_follow()` - Check if following artists

### User Management
- `user_follow_users()` - Follow other users
- `user_unfollow_users()` - Unfollow users

## Implementation Plan

### Phase 1: Track Saving/Unsaving
1. **Visual Indicator**
   - Add a heart icon (♥) next to saved tracks in track tables
   - Use different color/style for saved vs unsaved tracks
   
2. **Key Bindings**
   - Add 's' key to save/unsave current track (toggle)
   - Show confirmation in log stream when action completes
   
3. **Implementation**
   - Check saved status when displaying tracks
   - Batch check using `current_user_saved_tracks_contains()` for efficiency
   - Update visual state immediately, handle API response asynchronously

### Phase 2: Album Saving/Unsaving
1. **Visual Indicator**
   - Add saved indicator in album views
   - Show saved status in album list
   
2. **Key Bindings**
   - Add 'S' (shift+s) to save/unsave current album
   - Available in album view and album list
   
3. **Implementation**
   - Similar pattern to track saving
   - Update both album list and track table views

### Phase 3: Artist Following/Unfollowing
1. **Visual Indicator**
   - Show "Following" badge for followed artists
   - Different styling in artist lists
   
2. **Key Bindings**
   - Add 'f' to follow/unfollow artist
   - Available in artist view and search results
   
3. **Implementation**
   - Check follow status on artist load
   - Update artist list views

### Phase 4: Bulk Operations
1. **Multi-select Mode**
   - Add visual selection mode (space to select, enter to confirm)
   - Show selected count in status
   
2. **Bulk Actions**
   - Save/unsave multiple tracks
   - Save/unsave multiple albums
   - Follow/unfollow multiple artists

## Technical Considerations

### State Management
- Cache saved/followed status to avoid repeated API calls
- Implement optimistic updates for better UX
- Handle API failures gracefully

### Performance
- Batch API calls when checking multiple items
- Implement pagination for large operations
- Add loading indicators for long operations

### Error Handling
- Show errors in log stream
- Retry failed operations
- Handle rate limiting

## UI/UX Guidelines

### Visual Feedback
- Immediate visual update on action
- Loading state while API call in progress
- Success/failure indication

### Keyboard Shortcuts
- Consistent across different views
- Show hints in footer/help
- Avoid conflicts with existing shortcuts

### Confirmation
- No confirmation for single item toggle
- Confirm bulk operations
- Show undo option for destructive actions

## Future Enhancements

1. **Playlist Management**
   - Add/remove tracks from playlists
   - Create new playlists
   - Reorder playlist tracks

2. **Smart Actions**
   - Save all tracks from current album
   - Follow all artists from current playlist
   - Bulk operations based on filters

3. **Sync Features**
   - Export library to file
   - Import saved tracks/albums
   - Backup/restore functionality

## Implementation Priority

1. Track saving/unsaving (most common use case)
2. Album saving/unsaving
3. Artist following/unfollowing
4. Bulk operations
5. Advanced features

This plan provides a roadmap for making spotify-tui a fully-featured library management tool while maintaining good performance and user experience.