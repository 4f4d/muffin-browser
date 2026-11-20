//! Muffin's local, native-owned profile database.
//!
//! Remote page views must never receive this connection or its data directly.

use html5ever::{
    tendril::StrTendril,
    tokenizer::{BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer},
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

const SCHEMA_VERSION: i64 = 1;
const MAX_RESULTS: usize = 20;
const MAX_BOOKMARK_HTML_BYTES: usize = 4 * 1024 * 1024;
const MAX_BOOKMARK_NODES: usize = 10_000;
const MAX_FOLDER_DEPTH: usize = 64;

#[derive(Debug)]
pub enum ProfileError {
    Sql(rusqlite::Error),
    Url(String),
    Html(String),
}
impl From<rusqlite::Error> for ProfileError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sql(e)
    }
}
impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sql(e) => write!(f, "profile database error: {e}"),
            Self::Url(e) => write!(f, "invalid URL: {e}"),
            Self::Html(e) => write!(f, "invalid profile data: {e}"),
        }
    }
}
impl std::error::Error for ProfileError {}
pub type Result<T> = std::result::Result<T, ProfileError>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub theme: String,
    pub search_engine: String,
    pub restore_session: bool,
    pub clear_history_on_exit: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            search_engine: "duckduckgo".into(),
            restore_session: true,
            clear_history_on_exit: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Suggestion {
    pub url: String,
    pub title: String,
    pub bookmarked: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Visit {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visited_at_ms: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub url: Option<String>,
    pub title: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Shortcut {
    pub id: i64,
    pub url: String,
    pub title: String,
}

pub struct Profile {
    conn: Connection,
}
impl Profile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::default() | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        Self::initialize(conn)
    }
    pub fn in_memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }
    fn initialize(conn: Connection) -> Result<Self> {
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(ProfileError::Html(format!(
                "unsupported profile schema {version}"
            )));
        }
        conn.busy_timeout(std::time::Duration::from_millis(250))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        let mut p = Self { conn };
        p.migrate()?;
        Ok(p)
    }
    fn migrate(&mut self) -> Result<()> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(ProfileError::Html(format!(
                "unsupported profile schema {version}"
            )));
        }
        if version == 0 {
            let tx = self.conn.transaction()?;
            tx.execute_batch("\
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE visits (id INTEGER PRIMARY KEY, url TEXT NOT NULL, title TEXT NOT NULL DEFAULT '', visited_at_ms INTEGER NOT NULL);
            CREATE INDEX visits_time ON visits(visited_at_ms DESC);
            CREATE VIRTUAL TABLE visit_fts USING fts5(url, title, content='visits', content_rowid='id', tokenize='unicode61');
            CREATE TRIGGER visits_ai AFTER INSERT ON visits BEGIN INSERT INTO visit_fts(rowid,url,title) VALUES(new.id,new.url,new.title); END;
            CREATE TRIGGER visits_ad AFTER DELETE ON visits BEGIN INSERT INTO visit_fts(visit_fts,rowid,url,title) VALUES('delete',old.id,old.url,old.title); END;
            CREATE TRIGGER visits_au AFTER UPDATE ON visits BEGIN INSERT INTO visit_fts(visit_fts,rowid,url,title) VALUES('delete',old.id,old.url,old.title); INSERT INTO visit_fts(rowid,url,title) VALUES(new.id,new.url,new.title); END;
            CREATE TABLE bookmarks (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES bookmarks(id) ON DELETE CASCADE, url TEXT, title TEXT NOT NULL, position INTEGER NOT NULL DEFAULT 0, CHECK(url IS NULL OR (url LIKE 'http://%' OR url LIKE 'https://%')));
            CREATE TABLE shortcuts (id INTEGER PRIMARY KEY, url TEXT NOT NULL, title TEXT NOT NULL, position INTEGER NOT NULL DEFAULT 0, UNIQUE(url));
            PRAGMA user_version=1;")?;
            tx.commit()?;
        }
        Ok(())
    }
    pub fn settings(&self) -> Result<Settings> {
        let mut s = Settings::default();
        let mut q = self.conn.prepare("SELECT key,value FROM settings")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (k, v) = row?;
            match k.as_str() {
                "theme" if ["system", "light", "dark"].contains(&v.as_str()) => s.theme = v,
                "search_engine" if ["duckduckgo", "google", "bing"].contains(&v.as_str()) => {
                    s.search_engine = v
                }
                "restore_session" => s.restore_session = v == "true",
                "clear_history_on_exit" => s.clear_history_on_exit = v == "true",
                _ => (),
            }
        }
        Ok(s)
    }
    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        if !["system", "light", "dark"].contains(&s.theme.as_str())
            || !["duckduckgo", "google", "bing"].contains(&s.search_engine.as_str())
        {
            return Err(ProfileError::Html("invalid setting".into()));
        }
        let tx = self.conn.unchecked_transaction()?;
        for (k, v) in [
            ("theme", s.theme.as_str()),
            ("search_engine", s.search_engine.as_str()),
            (
                "restore_session",
                if s.restore_session { "true" } else { "false" },
            ),
            (
                "clear_history_on_exit",
                if s.clear_history_on_exit {
                    "true"
                } else {
                    "false"
                },
            ),
        ] {
            tx.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![k,v])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn add_visit(&self, url: &str, title: &str, at_ms: Option<i64>) -> Result<i64> {
        let u = web_url(url)?;
        let t = at_ms.unwrap_or_else(now_ms);
        self.conn.execute(
            "INSERT INTO visits(url,title,visited_at_ms) VALUES(?1,?2,?3)",
            params![u.as_str(), title, t],
        )?;
        Ok(self.conn.last_insert_rowid())
    }
    pub fn history(&self, limit: usize) -> Result<Vec<Visit>> {
        let mut q=self.conn.prepare("SELECT id,url,title,visited_at_ms FROM visits ORDER BY visited_at_ms DESC,id DESC LIMIT ?1")?;
        let rows = q.query_map([limit.min(100) as i64], |r| {
            Ok(Visit {
                id: r.get(0)?,
                url: r.get(1)?,
                title: r.get(2)?,
                visited_at_ms: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
    /// Search all locally retained visits, not only the newest history page.
    /// An empty query returns recent history, matching the address-bar history view.
    pub fn search_history(&self, query: &str, limit: usize) -> Result<Vec<Visit>> {
        let tokens = fts_query(query);
        let sql = if tokens.is_empty() {
            "SELECT id,url,title,visited_at_ms FROM visits ORDER BY visited_at_ms DESC,id DESC LIMIT ?1"
        } else {
            "SELECT v.id,v.url,v.title,v.visited_at_ms FROM visit_fts JOIN visits v ON v.id=visit_fts.rowid WHERE visit_fts MATCH ?1 ORDER BY bm25(visit_fts,1.0,2.5),v.visited_at_ms DESC LIMIT ?2"
        };
        let mut q = self.conn.prepare(sql)?;
        if tokens.is_empty() {
            let rows = q.query_map([limit.min(100) as i64], |r| {
                Ok(Visit {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    title: r.get(2)?,
                    visited_at_ms: r.get(3)?,
                })
            })?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        } else {
            let rows = q.query_map(params![tokens, limit.min(100) as i64], |r| {
                Ok(Visit {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    title: r.get(2)?,
                    visited_at_ms: r.get(3)?,
                })
            })?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        }
    }
    pub fn delete_visit(&self, id: i64) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM visits WHERE id=?1", [id])? != 0)
    }
    /// Delete visits in [start_ms, end_ms); None means unbounded on that side.
    pub fn clear_history_range(&self, start_ms: Option<i64>, end_ms: Option<i64>) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM visits WHERE (?1 IS NULL OR visited_at_ms>=?1) AND (?2 IS NULL OR visited_at_ms<?2)",params![start_ms,end_ms])?)
    }
    pub fn clear_history_on_exit(&self) -> Result<()> {
        if self.settings()?.clear_history_on_exit {
            self.clear_history_range(None, None)?;
        }
        Ok(())
    }
    pub fn suggest(&self, input: &str, limit: usize) -> Result<Vec<Suggestion>> {
        let tokens = fts_query(input);
        if tokens.is_empty() {
            return Ok(vec![]);
        }
        let query_terms: Vec<String> = input.split_whitespace().map(|s| s.to_lowercase()).collect();
        let mut candidates = std::collections::HashMap::<String, (Suggestion, f64)>::new();
        let mut q = self.conn.prepare("SELECT v.url,v.title,EXISTS(SELECT 1 FROM bookmarks b WHERE b.url=v.url),COUNT(*),MAX(v.visited_at_ms) FROM visit_fts JOIN visits v ON v.id=visit_fts.rowid WHERE visit_fts MATCH ?1 GROUP BY v.url")?;
        let rows = q.query_map([&tokens], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        for row in rows {
            let (url, title, bookmarked, visits, recent) = row?;
            let age_days =
                ((now_ms().saturating_sub(recent).max(0) as f64) / 86_400_000.0).min(10_000.0);
            let score = age_days - (visits as f64 * 2.0) - if bookmarked { 6.0 } else { 0.0 };
            candidates.insert(
                url.clone(),
                (
                    Suggestion {
                        url,
                        title,
                        bookmarked,
                    },
                    score,
                ),
            );
        }
        // Include bookmarked URLs without any history. Matching is local and
        // parameterized; URL and title values never become SQL syntax.
        let mut q = self.conn.prepare("SELECT url,title FROM bookmarks WHERE url IS NOT NULL ORDER BY position,id LIMIT 10000")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (url, title) = row?;
            if candidates.contains_key(&url) {
                continue;
            }
            let haystack = format!("{} {}", title.to_lowercase(), url.to_lowercase());
            if query_terms.iter().all(|term| haystack.contains(term)) {
                let normalized = title.to_lowercase();
                let score = if normalized == input.to_lowercase() {
                    -4.0
                } else if normalized.starts_with(&input.to_lowercase()) {
                    -2.0
                } else {
                    0.0
                };
                candidates.insert(
                    url.clone(),
                    (
                        Suggestion {
                            url,
                            title,
                            bookmarked: true,
                        },
                        score,
                    ),
                );
            }
        }
        let mut ranked: Vec<_> = candidates.into_values().collect();
        ranked.sort_by(|(a, sa), (b, sb)| sa.total_cmp(sb).then_with(|| a.url.cmp(&b.url)));
        Ok(ranked
            .into_iter()
            .take(limit.min(MAX_RESULTS))
            .map(|(item, _)| item)
            .collect())
    }
    pub fn add_bookmark(
        &self,
        parent_id: Option<i64>,
        url: Option<&str>,
        title: &str,
    ) -> Result<i64> {
        let u = url.map(web_url).transpose()?;
        self.conn.execute("INSERT INTO bookmarks(parent_id,url,title,position) VALUES(?1,?2,?3,(SELECT COALESCE(MAX(position)+1,0) FROM bookmarks WHERE parent_id IS ?1))",params![parent_id,u.as_ref().map(Url::as_str),title])?;
        Ok(self.conn.last_insert_rowid())
    }
    pub fn bookmark_for_url(&self, url: &str) -> Result<Option<i64>> {
        let url = web_url(url)?;
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM bookmarks WHERE url=?1 ORDER BY id LIMIT 1",
                [url.as_str()],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub fn bookmarks(&self) -> Result<Vec<Bookmark>> {
        let mut q = self.conn.prepare(
            "SELECT id,parent_id,url,title FROM bookmarks ORDER BY parent_id,position,id",
        )?;
        let rows = q.query_map([], |r| {
            Ok(Bookmark {
                id: r.get(0)?,
                parent_id: r.get(1)?,
                url: r.get(2)?,
                title: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
    pub fn delete_bookmark(&self, id: i64) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM bookmarks WHERE id=?1", [id])?
            != 0)
    }
    /// Move an item beneath a folder; reject self-parenting, missing parents,
    /// and moves that would make a folder its own descendant.
    pub fn move_bookmark(&self, id: i64, parent_id: Option<i64>) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        let is_folder: Option<bool> = tx
            .query_row("SELECT url IS NULL FROM bookmarks WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        let Some(is_folder) = is_folder else {
            tx.commit()?;
            return Ok(false);
        };
        if let Some(parent) = parent_id {
            let parent_folder: Option<bool> = tx
                .query_row(
                    "SELECT url IS NULL FROM bookmarks WHERE id=?1",
                    [parent],
                    |r| r.get(0),
                )
                .optional()?;
            if parent_folder != Some(true) || parent == id {
                return Err(ProfileError::Html(
                    "bookmark parent must be a different existing folder".into(),
                ));
            }
            if is_folder {
                let cycle: bool = tx.query_row("WITH RECURSIVE descendants(id) AS (SELECT id FROM bookmarks WHERE parent_id=?1 UNION ALL SELECT b.id FROM bookmarks b JOIN descendants d ON b.parent_id=d.id) SELECT EXISTS(SELECT 1 FROM descendants WHERE id=?2)", params![id,parent], |r| r.get(0))?;
                if cycle {
                    return Err(ProfileError::Html(
                        "cannot move a folder beneath its descendant".into(),
                    ));
                }
            }
        }
        let changed = tx.execute("UPDATE bookmarks SET parent_id=?1,position=(SELECT COALESCE(MAX(position)+1,0) FROM bookmarks WHERE parent_id IS ?1) WHERE id=?2", params![parent_id,id])? != 0;
        tx.commit()?;
        Ok(changed)
    }
    pub fn import_bookmarks_html(&mut self, html: &str) -> Result<usize> {
        if html.len() > MAX_BOOKMARK_HTML_BYTES {
            return Err(ProfileError::Html(
                "bookmark HTML exceeds 4 MiB limit".into(),
            ));
        }
        let entries = parse_bookmarks(html)?;
        let tx = self.conn.transaction()?;
        let mut ids = Vec::<i64>::with_capacity(entries.len());
        let mut count = 0;
        for e in entries {
            let parent = e.parent_index.map(|i| ids[i]);
            let position: i64 = tx.query_row(
                "SELECT COALESCE(MAX(position)+1,0) FROM bookmarks WHERE parent_id IS ?1",
                [parent],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO bookmarks(parent_id,url,title,position) VALUES(?1,?2,?3,?4)",
                params![parent, e.url, e.title, position],
            )?;
            ids.push(tx.last_insert_rowid());
            if ids.len() > MAX_BOOKMARK_NODES {
                return Err(ProfileError::Html(
                    "bookmark file contains more than 10,000 entries".into(),
                ));
            }
            if e.url.is_some() {
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }
    pub fn export_bookmarks_html(&self) -> Result<String> {
        let all = self.bookmarks()?;
        let mut s=String::from("<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Muffin Bookmarks</TITLE><H1>Bookmarks</H1><DL><p>\n");
        let mut children = std::collections::HashMap::<Option<i64>, Vec<Bookmark>>::new();
        for b in all {
            children.entry(b.parent_id).or_default().push(b);
        }
        write_bookmark_children(
            &mut s,
            &children,
            None,
            0,
            &mut std::collections::HashSet::new(),
        );
        s.push_str("</DL><p>\n");
        Ok(s)
    }
    pub fn shortcuts(&self) -> Result<Vec<Shortcut>> {
        let mut q = self
            .conn
            .prepare("SELECT id,url,title FROM shortcuts ORDER BY position,id LIMIT 12")?;
        let rows = q.query_map([], |r| {
            Ok(Shortcut {
                id: r.get(0)?,
                url: r.get(1)?,
                title: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
    pub fn set_shortcut(&self, id: Option<i64>, url: &str, title: &str) -> Result<i64> {
        let u = web_url(url)?;
        if let Some(id) = id {
            self.conn.execute(
                "UPDATE shortcuts SET url=?1,title=?2 WHERE id=?3",
                params![u.as_str(), title, id],
            )?;
            Ok(id)
        } else {
            self.conn.execute("INSERT INTO shortcuts(url,title,position) VALUES(?1,?2,(SELECT COALESCE(MAX(position)+1,0) FROM shortcuts)) ON CONFLICT(url) DO UPDATE SET title=excluded.title",params![u.as_str(),title])?;
            Ok(self.conn.last_insert_rowid())
        }
    }
    pub fn remove_shortcut(&self, id: i64) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM shortcuts WHERE id=?1", [id])?
            != 0)
    }
}
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
fn web_url(raw: &str) -> Result<Url> {
    let u = Url::parse(raw).map_err(|e| ProfileError::Url(e.to_string()))?;
    if !["http", "https"].contains(&u.scheme())
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
    {
        return Err(ProfileError::Url(
            "only credential-free HTTP(S) URLs are accepted".into(),
        ));
    }
    Ok(u)
}
fn fts_query(raw: &str) -> String {
    raw.split_whitespace()
        .filter_map(|w| {
            let t: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
            (!t.is_empty()).then(|| format!("\"{}\"*", t.replace('"', "")))
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}
fn write_bookmark_children(
    output: &mut String,
    children: &std::collections::HashMap<Option<i64>, Vec<Bookmark>>,
    parent: Option<i64>,
    depth: usize,
    seen: &mut std::collections::HashSet<i64>,
) {
    if depth > MAX_FOLDER_DEPTH {
        return;
    }
    if let Some(items) = children.get(&parent) {
        for item in items {
            if !seen.insert(item.id) {
                continue;
            }
            if let Some(url) = &item.url {
                output.push_str(&format!(
                    "<DT><A HREF=\"{}\">{}</A>\n",
                    html_escape(url),
                    html_escape(&item.title)
                ));
            } else {
                output.push_str(&format!(
                    "<DT><H3>{}</H3>\n<DL><p>\n",
                    html_escape(&item.title)
                ));
                write_bookmark_children(output, children, Some(item.id), depth + 1, seen);
                output.push_str("</DL><p>\n");
            }
        }
    }
}
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
#[derive(Debug)]
struct ImportEntry {
    parent_index: Option<usize>,
    url: Option<String>,
    title: String,
}
#[derive(Default)]
struct BookmarkHtmlState {
    entries: Vec<ImportEntry>,
    folder_stack: Vec<usize>,
    pending_folder: Option<usize>,
    heading: Option<String>,
    anchor: Option<(String, String)>,
    overflow: bool,
    raw_text_element: Option<String>,
}
struct BookmarkHtmlSink(std::cell::RefCell<BookmarkHtmlState>);
impl TokenSink for BookmarkHtmlSink {
    type Handle = ();
    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        let mut state = self.0.borrow_mut();
        if let Some(raw_name) = &state.raw_text_element {
            if matches!(&token, Token::TagToken(t) if t.kind == TagKind::EndTag && &*t.name == raw_name.as_str())
            {
                state.raw_text_element = None;
            }
            return TokenSinkResult::Continue;
        }
        match token {
            Token::TagToken(tag) => match (tag.kind, tag.name.to_string().as_str()) {
                (TagKind::StartTag, "script" | "style") => {
                    state.raw_text_element = Some(tag.name.to_string())
                }
                (TagKind::StartTag, "h3") => state.heading = Some(String::new()),
                (TagKind::EndTag, "h3") => {
                    if let Some(title) = state.heading.take() {
                        if state.entries.len() < MAX_BOOKMARK_NODES {
                            let index = state.entries.len();
                            let parent_index = state.folder_stack.last().copied();
                            state.entries.push(ImportEntry {
                                parent_index,
                                url: None,
                                title: title.trim().chars().take(4096).collect(),
                            });
                            state.pending_folder = Some(index);
                        } else {
                            state.overflow = true;
                        }
                    }
                }
                (TagKind::StartTag, "dl") => {
                    if let Some(folder) = state.pending_folder.take() {
                        if state.folder_stack.len() < MAX_FOLDER_DEPTH {
                            state.folder_stack.push(folder);
                        } else {
                            state.overflow = true;
                        }
                    }
                }
                (TagKind::EndTag, "dl") => {
                    state.folder_stack.pop();
                    state.pending_folder = None;
                }
                (TagKind::StartTag, "a") => {
                    let href = tag
                        .attrs
                        .iter()
                        .find(|a| a.name.local.to_string() == "href")
                        .map(|a| a.value.to_string())
                        .unwrap_or_default();
                    state.anchor = Some((href, String::new()));
                }
                (TagKind::EndTag, "a") => {
                    if let Some((raw, title)) = state.anchor.take() {
                        if state.entries.len() < MAX_BOOKMARK_NODES && raw.len() <= 8192 {
                            if let Ok(url) = web_url(&raw) {
                                let parent_index = state.folder_stack.last().copied();
                                state.entries.push(ImportEntry {
                                    parent_index,
                                    url: Some(url.into()),
                                    title: title.trim().chars().take(4096).collect(),
                                });
                            }
                        } else if raw.len() > 8192 || state.entries.len() >= MAX_BOOKMARK_NODES {
                            state.overflow = true;
                        }
                    }
                }
                _ => (),
            },
            Token::CharacterTokens(text) => {
                if let Some(title) = state.heading.as_mut() {
                    append_bounded(title, &text, 4096);
                } else if let Some((_, title)) = state.anchor.as_mut() {
                    append_bounded(title, &text, 4096);
                }
            }
            _ => (),
        }
        TokenSinkResult::Continue
    }
}
fn parse_bookmarks(html: &str) -> Result<Vec<ImportEntry>> {
    let input = BufferQueue::default();
    input.push_back(StrTendril::from_slice(html));
    let tokenizer = Tokenizer::new(
        BookmarkHtmlSink(std::cell::RefCell::new(BookmarkHtmlState::default())),
        Default::default(),
    );
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let state = tokenizer.sink.0.into_inner();
    if state.overflow {
        return Err(ProfileError::Html(
            "bookmark import exceeds node or nesting-depth limit".into(),
        ));
    }
    Ok(state.entries)
}
fn append_bounded(target: &mut String, text: &str, max: usize) {
    let remaining = max.saturating_sub(target.len());
    let mut end = remaining.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    target.push_str(&text[..end]);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_and_history_persist() {
        let d = tempfile::tempdir().unwrap();
        // macOS returns a temporary path below /var, which is a system symlink.
        let directory = d.path().canonicalize().unwrap();
        let path = directory.join("profile.sqlite");
        {
            let p = Profile::open(&path).unwrap();
            let s = Settings {
                theme: "dark".into(),
                ..Settings::default()
            };
            p.save_settings(&s).unwrap();
            p.add_visit(
                "https://example.com/path",
                "Example search title",
                Some(1000),
            )
            .unwrap();
            assert_eq!(p.settings().unwrap(), s);
        }
        let p = Profile::open(&path).unwrap();
        assert_eq!(p.history(4).unwrap().len(), 1);
        assert_eq!(p.settings().unwrap().theme, "dark");
        #[cfg(unix)]
        {
            let linked_database = directory.join("linked-profile.sqlite");
            std::os::unix::fs::symlink(&path, &linked_database).unwrap();
            assert!(Profile::open(&linked_database).is_err());
        }
    }
    #[test]
    fn fts_search_ranks_matches_and_clear_range_updates_index() {
        let p = Profile::in_memory().unwrap();
        p.add_visit("https://example.com/rust", "Rust language guide", Some(100))
            .unwrap();
        p.add_visit("https://other.test/", "Unrelated", Some(200))
            .unwrap();
        assert_eq!(
            p.suggest("rust", 5).unwrap()[0].title,
            "Rust language guide"
        );
        p.clear_history_range(Some(0), Some(150)).unwrap();
        assert!(p.suggest("rust", 5).unwrap().is_empty());
    }
    #[test]
    fn history_search_and_folder_moves_are_bounded_and_cycle_safe() {
        let p = Profile::in_memory().unwrap();
        p.add_visit("https://example.com/one", "Local title", Some(1))
            .unwrap();
        assert_eq!(p.search_history("title", 200).unwrap().len(), 1);
        assert_eq!(p.search_history("", 200).unwrap().len(), 1);
        let folder = p.add_bookmark(None, None, "Folder").unwrap();
        let nested = p.add_bookmark(Some(folder), None, "Nested").unwrap();
        let link = p
            .add_bookmark(Some(nested), Some("https://example.com"), "Example")
            .unwrap();
        assert!(p.move_bookmark(folder, Some(nested)).is_err());
        assert!(p.move_bookmark(link, None).unwrap());
        assert_eq!(
            p.bookmarks()
                .unwrap()
                .iter()
                .find(|b| b.id == link)
                .unwrap()
                .parent_id,
            None
        );
    }
    #[test]
    fn bookmark_html_roundtrip_rejects_unsafe() {
        let mut p = Profile::in_memory().unwrap();
        p.add_bookmark(None, Some("https://example.com/?a=1&b=2"), "A & B")
            .unwrap();
        let html = p.export_bookmarks_html().unwrap();
        assert!(html.contains("&amp;"));
        assert_eq!(
            parse_bookmarks(
                "<A HREF='javascript:alert(1)'>bad</A><A HREF='https://ok.test'>ok</A>"
            )
            .unwrap()
            .iter()
            .filter(|e| e.url.is_some())
            .count(),
            1
        );
        assert_eq!(p.import_bookmarks_html(&html).unwrap(), 1);
    }
    #[test]
    fn nested_html_roundtrip_keeps_sibling_order_and_ignores_injection() {
        let source="<!doctype html><DL><p><DT><H3>Top &amp; Safe</H3><DL><p><DT><A HREF='https://one.test/?x=1&amp;y=2'>First &amp; one</A><DT><H3>Nested</H3><DL><p><DT><A HREF=\"https://nested.test\">Nested link</A></DL><DT><A HREF='https://two.test'>Second</A></DL><DT><A HREF='https://root.test'>Root link</A><!-- <A HREF='https://comment.test'>bad</A> --><script><A HREF='https://script.test'>bad</A></script></DL>";
        let mut p = Profile::in_memory().unwrap();
        assert_eq!(p.import_bookmarks_html(source).unwrap(), 4);
        let items = p.bookmarks().unwrap();
        assert_eq!(items.len(), 6);
        let root = items.iter().find(|b| b.title == "Top & Safe").unwrap();
        let nested = items.iter().find(|b| b.title == "Nested").unwrap();
        assert_eq!(nested.parent_id, Some(root.id));
        assert_eq!(
            items
                .iter()
                .find(|b| b.title == "Nested link")
                .unwrap()
                .parent_id,
            Some(nested.id)
        );
        let html = p.export_bookmarks_html().unwrap();
        assert!(html.find("First &amp; one").unwrap() < html.find("Nested").unwrap());
        assert!(html.find("Nested").unwrap() < html.find("Second").unwrap());
        assert!(html.find("Second").unwrap() < html.find("Root link").unwrap());
        let mut q = Profile::in_memory().unwrap();
        q.import_bookmarks_html(&html).unwrap();
        let again = q.bookmarks().unwrap();
        assert_eq!(
            again.iter().map(|b| (&b.url, &b.title)).collect::<Vec<_>>(),
            items.iter().map(|b| (&b.url, &b.title)).collect::<Vec<_>>()
        );
        assert!(!again
            .iter()
            .any(|b| b.title.contains("comment.test") || b.title.contains("script.test")));
    }
    #[test]
    fn omnibox_includes_unvisited_bookmarks_without_duplicates() {
        let p = Profile::in_memory().unwrap();
        let id = p
            .add_bookmark(None, Some("https://docs.example.test/rust"), "Rust docs")
            .unwrap();
        assert_eq!(
            p.bookmark_for_url("https://docs.example.test/rust")
                .unwrap(),
            Some(id)
        );
        let suggestions = p.suggest("rust", 10).unwrap();
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].bookmarked);
        p.add_visit(
            "https://docs.example.test/rust",
            "Rust docs",
            Some(now_ms()),
        )
        .unwrap();
        assert_eq!(p.suggest("rust", 10).unwrap().len(), 1);
    }
    #[test]
    fn rejects_credentials_and_non_web_schemes() {
        assert!(web_url("https://user:secret@example.com").is_err());
        assert!(web_url("file:///tmp/a").is_err());
    }
}
