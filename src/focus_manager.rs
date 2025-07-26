use crate::app::{ActiveBlock, SearchResultBlock, ArtistBlock};

#[derive(Debug, Clone, PartialEq)]
pub enum FocusState {
    /// Component is unfocused - no interaction possible
    Unfocused,
    /// Component is hovered - can navigate with arrows, highlighted
    Hovered,
    /// Component is focused - actively receiving input, can perform actions
    Focused,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComponentId {
    // Main components
    Library,
    MyPlaylists,
    SearchInput,
    SearchResults(SearchResultBlock),
    Artist(ArtistBlock),
    TrackTable,
    EpisodeTable,
    AlbumList,
    AlbumTracks,
    RecentlyPlayed,
    Artists,
    Podcasts,
    Home,
    SelectDevice,
    PlayBar,
    BasicView,
    LogStream,
    Analysis,
    Dialog,
    Empty,
}

pub struct FocusManager {
    /// Currently focused component (only one can be focused at a time)
    focused_component: Option<ComponentId>,
    /// Currently hovered component (only one can be hovered at a time)
    hovered_component: Option<ComponentId>,
}

impl Default for FocusManager {
    fn default() -> Self {
        Self {
            focused_component: None,
            hovered_component: None,
        }
    }
}

impl FocusManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set focus to a component, clearing any previous focus
    pub fn set_focus(&mut self, component: ComponentId) {
        self.focused_component = Some(component);
        // When we focus a component, it should also be hovered
        self.hovered_component = Some(component);
    }

    /// Set hover to a component, clearing any previous hover
    /// Does not affect focus state
    pub fn set_hover(&mut self, component: ComponentId) {
        self.hovered_component = Some(component);
    }

    /// Clear focus, keeping hover state
    pub fn clear_focus(&mut self) {
        self.focused_component = None;
    }

    /// Clear hover state
    pub fn clear_hover(&mut self) {
        self.hovered_component = None;
    }

    /// Clear both focus and hover
    pub fn clear_all(&mut self) {
        self.focused_component = None;
        self.hovered_component = None;
    }

    /// Get the focus state of a component
    pub fn get_focus_state(&self, component: &ComponentId) -> FocusState {
        if self.focused_component.as_ref() == Some(component) {
            FocusState::Focused
        } else if self.hovered_component.as_ref() == Some(component) {
            FocusState::Hovered
        } else {
            FocusState::Unfocused
        }
    }

    /// Check if a component is focused
    pub fn is_focused(&self, component: &ComponentId) -> bool {
        self.focused_component.as_ref() == Some(component)
    }

    /// Check if a component is hovered
    pub fn is_hovered(&self, component: &ComponentId) -> bool {
        self.hovered_component.as_ref() == Some(component)
    }

    /// Get currently focused component
    pub fn get_focused(&self) -> Option<&ComponentId> {
        self.focused_component.as_ref()
    }

    /// Get currently hovered component
    pub fn get_hovered(&self) -> Option<&ComponentId> {
        self.hovered_component.as_ref()
    }

    /// Navigate between components (for arrow key navigation)
    /// This affects hover state, not focus
    pub fn navigate_to(&mut self, component: ComponentId) {
        self.set_hover(component);
    }

    /// Enter a component (for direct key shortcuts like P, L, S)
    /// This sets both focus and hover
    pub fn enter_component(&mut self, component: ComponentId) {
        self.set_focus(component);
    }

    /// Convert from legacy ActiveBlock to ComponentId
    pub fn from_active_block(block: ActiveBlock) -> ComponentId {
        match block {
            ActiveBlock::Library => ComponentId::Library,
            ActiveBlock::MyPlaylists => ComponentId::MyPlaylists,
            ActiveBlock::Input => ComponentId::SearchInput,
            ActiveBlock::SearchResultBlock => ComponentId::SearchResults(SearchResultBlock::Empty),
            ActiveBlock::ArtistBlock => ComponentId::Artist(ArtistBlock::Empty),
            ActiveBlock::TrackTable => ComponentId::TrackTable,
            ActiveBlock::EpisodeTable => ComponentId::EpisodeTable,
            ActiveBlock::AlbumList => ComponentId::AlbumList,
            ActiveBlock::AlbumTracks => ComponentId::AlbumTracks,
            ActiveBlock::RecentlyPlayed => ComponentId::RecentlyPlayed,
            ActiveBlock::Artists => ComponentId::Artists,
            ActiveBlock::Podcasts => ComponentId::Podcasts,
            ActiveBlock::Home => ComponentId::Home,
            ActiveBlock::SelectDevice => ComponentId::SelectDevice,
            ActiveBlock::PlayBar => ComponentId::PlayBar,
            ActiveBlock::BasicView => ComponentId::BasicView,
            ActiveBlock::LogStream => ComponentId::LogStream,
            ActiveBlock::Analysis => ComponentId::Analysis,
            ActiveBlock::Dialog(_) => ComponentId::Dialog,
            ActiveBlock::Empty => ComponentId::Empty,
            ActiveBlock::Error => ComponentId::Empty, // Error is deprecated
        }
    }

    /// Convert ComponentId back to ActiveBlock for legacy compatibility
    pub fn to_active_block(&self, component: &ComponentId) -> ActiveBlock {
        match component {
            ComponentId::Library => ActiveBlock::Library,
            ComponentId::MyPlaylists => ActiveBlock::MyPlaylists,
            ComponentId::SearchInput => ActiveBlock::Input,
            ComponentId::SearchResults(_) => ActiveBlock::SearchResultBlock,
            ComponentId::Artist(_) => ActiveBlock::ArtistBlock,
            ComponentId::TrackTable => ActiveBlock::TrackTable,
            ComponentId::EpisodeTable => ActiveBlock::EpisodeTable,
            ComponentId::AlbumList => ActiveBlock::AlbumList,
            ComponentId::AlbumTracks => ActiveBlock::AlbumTracks,
            ComponentId::RecentlyPlayed => ActiveBlock::RecentlyPlayed,
            ComponentId::Artists => ActiveBlock::Artists,
            ComponentId::Podcasts => ActiveBlock::Podcasts,
            ComponentId::Home => ActiveBlock::Home,
            ComponentId::SelectDevice => ActiveBlock::SelectDevice,
            ComponentId::PlayBar => ActiveBlock::PlayBar,
            ComponentId::BasicView => ActiveBlock::BasicView,
            ComponentId::LogStream => ActiveBlock::LogStream,
            ComponentId::Analysis => ActiveBlock::Analysis,
            ComponentId::Dialog => ActiveBlock::Dialog(Default::default()),
            ComponentId::Empty => ActiveBlock::Empty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_states() {
        let mut fm = FocusManager::new();
        let comp = ComponentId::Library;

        // Initially unfocused
        assert_eq!(fm.get_focus_state(&comp), FocusState::Unfocused);
        assert!(!fm.is_focused(&comp));
        assert!(!fm.is_hovered(&comp));

        // Set hover
        fm.set_hover(comp.clone());
        assert_eq!(fm.get_focus_state(&comp), FocusState::Hovered);
        assert!(!fm.is_focused(&comp));
        assert!(fm.is_hovered(&comp));

        // Set focus (should also set hover)
        fm.clear_hover();
        fm.set_focus(comp.clone());
        assert_eq!(fm.get_focus_state(&comp), FocusState::Focused);
        assert!(fm.is_focused(&comp));
        assert!(fm.is_hovered(&comp));

        // Clear focus (should keep hover)
        fm.clear_focus();
        assert_eq!(fm.get_focus_state(&comp), FocusState::Hovered);
        assert!(!fm.is_focused(&comp));
        assert!(fm.is_hovered(&comp));

        // Clear all
        fm.clear_all();
        assert_eq!(fm.get_focus_state(&comp), FocusState::Unfocused);
        assert!(!fm.is_focused(&comp));
        assert!(!fm.is_hovered(&comp));
    }

    #[test]
    fn test_single_focus() {
        let mut fm = FocusManager::new();
        let comp1 = ComponentId::Library;
        let comp2 = ComponentId::MyPlaylists;

        // Focus first component
        fm.set_focus(comp1.clone());
        assert!(fm.is_focused(&comp1));
        assert!(!fm.is_focused(&comp2));

        // Focus second component (should clear first)
        fm.set_focus(comp2.clone());
        assert!(!fm.is_focused(&comp1));
        assert!(fm.is_focused(&comp2));
    }
}