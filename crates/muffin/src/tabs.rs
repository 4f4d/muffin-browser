use serde::{Deserialize, Serialize};

const MAX_TABS: usize = 100;
const MAX_TAB_ID: u32 = u32::MAX - MAX_TABS as u32;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct TabState {
    pub id: u32,
    pub title: String,
    pub url: String,
    pub pinned: bool,
    pub loading: bool,
    pub discarded: bool,
    pub generation: u64,
}

impl TabState {
    fn home(id: u32) -> Self {
        Self {
            id,
            title: "New Tab".into(),
            url: "about:blank".into(),
            pinned: false,
            loading: false,
            discarded: false,
            generation: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SavedWindow {
    pub tabs: Vec<SavedTab>,
    pub selected_tab: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SavedTab {
    pub id: u32,
    pub title: String,
    pub url: String,
    pub pinned: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SessionDocument {
    pub version: u8,
    pub windows: Vec<SavedWindow>,
}

#[derive(Debug)]
pub(crate) struct TabSet {
    tabs: Vec<TabState>,
    selected: u32,
    next_id: u32,
    closed: Vec<SavedTab>,
}

impl TabSet {
    pub fn new() -> Self {
        let home = TabState::home(1);
        Self {
            tabs: vec![home],
            selected: 1,
            next_id: 2,
            closed: Vec::new(),
        }
    }

    pub fn from_saved(saved: SavedWindow) -> Self {
        let mut next_id = 1;
        let tabs = saved
            .tabs
            .into_iter()
            .map(|tab| {
                let id = tab.id;
                next_id = next_id.max(id.saturating_add(1));
                TabState {
                    id,
                    title: tab.title,
                    url: tab.url,
                    pinned: tab.pinned,
                    loading: false,
                    discarded: true,
                    generation: 0,
                }
            })
            .collect::<Vec<_>>();
        let mut result = Self {
            tabs: if tabs.is_empty() {
                vec![TabState::home(next_id)]
            } else {
                tabs
            },
            selected: saved.selected_tab,
            next_id: next_id + 1,
            closed: Vec::new(),
        };
        if !result.tabs.iter().any(|tab| tab.id == result.selected) {
            result.selected = result.tabs[0].id;
        }
        result
    }

    pub fn tabs(&self) -> &[TabState] {
        &self.tabs
    }

    pub fn can_create(&self) -> bool {
        self.tabs.len() < MAX_TABS && self.next_id < MAX_TAB_ID
    }

    pub fn selected(&self) -> u32 {
        self.selected
    }

    pub fn get(&self, id: u32) -> Option<&TabState> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut TabState> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    pub fn select(&mut self, id: u32) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        self.selected = id;
        true
    }

    pub fn create(&mut self, saved: Option<SavedTab>) -> u32 {
        assert!(self.can_create(), "tab limit reached");
        let id = self.allocate_id();
        let mut tab = saved
            .map(|saved| TabState {
                id,
                title: saved.title,
                url: saved.url,
                pinned: saved.pinned,
                loading: false,
                discarded: true,
                generation: 0,
            })
            .unwrap_or_else(|| TabState::home(id));
        tab.loading = false;
        let index = if tab.pinned {
            self.pinned_count()
        } else {
            self.tabs.len()
        };
        self.tabs.insert(index, tab);
        self.selected = id;
        id
    }

    pub fn duplicate(&mut self, id: u32) -> Option<u32> {
        if !self.can_create() {
            return None;
        }
        let mut copy = self.get(id)?.clone();
        copy.id = self.allocate_id();
        copy.loading = false;
        copy.discarded = true;
        copy.generation = 0;
        let new_id = copy.id;
        let index = if copy.pinned {
            self.pinned_count()
        } else {
            self.tabs.len()
        };
        self.tabs.insert(index, copy);
        self.selected = new_id;
        Some(new_id)
    }

    pub fn close(&mut self, id: u32) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        let tab = self.tabs.remove(index);
        self.closed.push(SavedTab {
            id: tab.id,
            title: tab.title,
            url: tab.url,
            pinned: tab.pinned,
        });
        if self.tabs.is_empty() {
            self.create(None);
        } else if self.selected == id {
            self.selected = self.tabs[index.min(self.tabs.len() - 1)].id;
        }
        true
    }

    pub fn reopen(&mut self) -> Option<u32> {
        if !self.can_create() {
            return None;
        }
        let closed = self.closed.pop()?;
        Some(self.create(Some(closed)))
    }

    pub fn can_reopen(&self) -> bool {
        !self.closed.is_empty()
    }

    pub fn set_pinned(&mut self, id: u32, pinned: bool) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        if self.tabs[index].pinned == pinned {
            return true;
        }
        let mut tab = self.tabs.remove(index);
        tab.pinned = pinned;
        let target = self.pinned_count();
        self.tabs.insert(target, tab);
        true
    }

    pub fn move_to(&mut self, id: u32, index: usize) -> bool {
        let Some(old_index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        if index >= self.tabs.len() {
            return false;
        }
        let pinned_count = self.pinned_count();
        let pinned = self.tabs[old_index].pinned;
        if (pinned && index >= pinned_count) || (!pinned && index < pinned_count) {
            return false;
        }
        let tab = self.tabs.remove(old_index);
        self.tabs.insert(index.min(self.tabs.len()), tab);
        true
    }

    pub fn discard(&mut self, id: u32) -> bool {
        if id == self.selected {
            return false;
        }
        let Some(tab) = self.get_mut(id) else {
            return false;
        };
        if tab.loading {
            return false;
        }
        tab.discarded = true;
        tab.generation += 1;
        true
    }

    pub fn saved(&self) -> SavedWindow {
        SavedWindow {
            tabs: self
                .tabs
                .iter()
                .map(|tab| SavedTab {
                    id: tab.id,
                    title: tab.title.clone(),
                    url: tab.url.clone(),
                    pinned: tab.pinned,
                })
                .collect(),
            selected_tab: self.selected,
        }
    }

    fn pinned_count(&self) -> usize {
        self.tabs.iter().take_while(|tab| tab.pinned).count()
    }

    fn allocate_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_tabs_stay_in_the_pinned_segment_and_reject_crossing_moves() {
        let mut tabs = TabSet::new();
        let second = tabs.create(None);
        let third = tabs.create(None);
        assert!(tabs.set_pinned(third, true));
        assert_eq!(
            tabs.tabs().iter().map(|tab| tab.id).collect::<Vec<_>>(),
            [third, 1, second]
        );
        assert!(!tabs.move_to(third, 2));
        assert!(!tabs.move_to(second, 0));
        assert!(tabs.move_to(second, 2));
    }

    #[test]
    fn closing_and_reopening_restores_only_navigation_metadata_with_new_identity() {
        let mut tabs = TabSet::new();
        let original = tabs.selected();
        tabs.get_mut(original).unwrap().url = "https://example.test/".into();
        tabs.get_mut(original).unwrap().title = "Example".into();
        assert!(tabs.close(original));
        let reopened = tabs.reopen().unwrap();
        let restored = tabs.get(reopened).unwrap();
        assert_ne!(reopened, original);
        assert_eq!(restored.url, "https://example.test/");
        assert!(restored.discarded);
        assert!(!restored.loading);
    }

    #[test]
    fn discard_is_explicit_inactive_and_never_drops_a_loading_tab() {
        let mut tabs = TabSet::new();
        let inactive = tabs.create(None);
        tabs.select(1);
        assert!(tabs.discard(inactive));
        assert!(tabs.get(inactive).unwrap().discarded);
        tabs.get_mut(inactive).unwrap().loading = true;
        assert!(!tabs.discard(inactive));
        assert!(!tabs.discard(1));
    }

    #[test]
    fn duplicate_gets_new_identity_and_preserves_url_and_pin() {
        let mut tabs = TabSet::new();
        let original = tabs.selected();
        let state = tabs.get_mut(original).unwrap();
        state.url = "https://example.test/path".into();
        state.pinned = true;
        let duplicate = tabs.duplicate(original).unwrap();
        assert_ne!(duplicate, original);
        assert_eq!(
            tabs.get(duplicate).unwrap().url,
            "https://example.test/path"
        );
        assert!(tabs.get(duplicate).unwrap().pinned);
    }

    #[test]
    fn session_round_trip_marks_webviews_for_lazy_recreation() {
        let mut tabs = TabSet::new();
        let second = tabs.create(None);
        tabs.get_mut(second).unwrap().url = "https://example.test".into();
        let saved = tabs.saved();
        let restored = TabSet::from_saved(saved);
        assert_eq!(
            restored.get(restored.selected()).unwrap().url,
            "https://example.test"
        );
        assert!(restored
            .tabs()
            .iter()
            .all(|tab| tab.discarded && !tab.loading));
    }
}
