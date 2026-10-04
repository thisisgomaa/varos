//! Start v2 — Boards: the VIEW-MODEL the page draws (lane L4). Plain data, already filtered, sorted
//! and formatted by the model; the page never reads files, never filters, never formats dates.
//!
//! The page reads a [`StartView`] and returns [`StartIntent`]s; the host turns intents into commands
//! (K1: the kit/page know no `AppCommand`). Tag filtering and Search are the MODEL's job: the page only
//! emits `SetTagFilter` / `Search` and draws whatever cards the next view carries.
use std::path::PathBuf;

/// One recent board, as the card and the list row show it.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardCard {
    /// Stable identity (the recents key): focus and the open menu survive a rebuild by this key.
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// Artboards inside the board; 0 = a free board ("free").
    pub artboards: u32,
    /// The board file. The card shows its folder, `~`-relative and middle-elided to fit.
    pub path: PathBuf,
    /// Already formatted ("Today 14:32", "2 Oct").
    pub modified: String,
    pub missing: bool,
    /// The board's thumbnail texture (lane L3). `None` → the typographic placeholder.
    pub thumb: Option<egui::TextureId>,
}

/// One recovered (unsaved) board: the Recovered band.
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveredRow {
    pub id: String,
    pub name: String,
    /// Already formatted, e.g. "unsaved changes from 11:48 today".
    pub when: String,
    /// The original folder, already `~`-relative; `None` for a never-saved board.
    pub folder: Option<String>,
    /// Why this copy cannot be recovered (Recover is then disabled with this reason).
    pub problem: Option<String>,
    /// A recover/discard is running for this row: both actions are busy.
    pub busy: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

/// The artboard presets ("…or start with an artboard"); `Custom` asks for a size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PresetId {
    Square,
    Portrait,
    Story,
    A4,
    Custom,
}
impl PresetId {
    pub const ALL: [PresetId; 5] = [Self::Square, Self::Portrait, Self::Story, Self::A4, Self::Custom];
    pub fn label(self) -> &'static str {
        match self {
            Self::Square => "Square",
            Self::Portrait => "Portrait",
            Self::Story => "Story",
            Self::A4 => "A4",
            Self::Custom => "Custom…",
        }
    }
    /// Width, height and unit label; `None` for Custom.
    pub fn size(self) -> Option<(u32, u32, &'static str)> {
        match self {
            Self::Square => Some((1080, 1080, "px")),
            Self::Portrait => Some((1080, 1350, "px")),
            Self::Story => Some((1080, 1920, "px")),
            Self::A4 => Some((595, 842, "pt")),
            Self::Custom => None,
        }
    }
    /// The mono size line under the name ("1080 × 1350 px", "any size").
    pub fn size_text(self) -> String {
        self.size().map_or_else(|| "any size".to_string(), |(w, h, u)| format!("{w} × {h} {u}"))
    }
}

/// Everything the Start page draws this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StartView {
    /// The cards to show — already filtered by `filter` and `search`, in display order.
    pub cards: Vec<BoardCard>,
    pub recovered: Vec<RecoveredRow>,
    /// Every tag with its board count, in filter order (the model sorts: count desc, then name).
    pub tags: Vec<(String, usize)>,
    /// Boards in Recent before filtering ("Recent boards 10", "All 10"). 0 + nothing recovered =
    /// the first-launch page.
    pub total: usize,
    /// The selected tag; `None` = All.
    pub filter: Option<String>,
    pub search: String,
    pub view: ViewMode,
    /// Documents are open (the top bar shows tabs). The page itself does not change; the example
    /// gallery uses it for its stand-in bar.
    pub has_tabs: bool,
}
impl StartView {
    /// No board in Recent and nothing recovered: the centred "Start with a board" page.
    pub fn is_first_launch(&self) -> bool {
        self.total == 0 && self.recovered.is_empty()
    }
}

/// What the user asked for. One per activation; the host maps each to a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartIntent {
    NewBoard,
    Open,
    NewWithPreset(PresetId),
    OpenBoard(String),
    Remove(String),
    Locate(String),
    Recover(String),
    Discard(String),
    SetTagFilter(Option<String>),
    SetView(ViewMode),
    Search(String),
}

// TODO(moderator, after lane L2 lands) — the adapter `StartModel → StartView`, deliberately NOT
// compiled here because L2 is changing `StartModel` in parallel. Sketch:
//
//     pub fn from_model(m: &crate::start::StartModel, thumbs: &impl Fn(&ThumbKey) -> Option<egui::TextureId>) -> StartView {
//         StartView {
//             cards: m.boards().iter().map(|b| BoardCard {
//                 key: b.key.clone(), name: b.name.clone(),
//                 description: (!b.description.is_empty()).then(|| b.description.clone()),
//                 tags: b.tags.clone(), artboards: b.artboards, path: b.path.clone(),
//                 modified: b.when_text.clone(), missing: b.missing, thumb: b.thumb.as_ref().and_then(thumbs),
//             }).collect(),
//             recovered: m.recovery().iter().map(|r| RecoveredRow {
//                 id: r.rid.clone(), name: r.name.clone(), when: r.saved_at_text.clone(),
//                 folder: r.original_dir.clone(), problem: r.problem.clone(), busy: r.busy,
//             }).collect(),
//             tags: m.tag_counts(), total: m.total(), filter: m.filter().cloned(),
//             search: m.search().to_string(), view: m.view(), has_tabs,
//         }
//     }
//
// and the intent → command map: NewBoard/NewWithPreset → L2's new commands, Open → StartAction::Open,
// OpenBoard(key) → OpenRecent(path of key), Remove → RemoveRecent, Locate → Locate, Recover → Recover,
// Discard → DiscardRecovery, SetTagFilter/SetView/Search → model setters (then rebuild the view).

/// Realistic stand-in data (the ten boards of the mockup, one Recovered row) for the example gallery
/// and the tests. Not used by the app.
#[doc(hidden)]
pub mod demo {
    use super::*;

    /// (name, description, tags, folder under the home dir, date, artboards)
    pub type DemoBoard = (&'static str, &'static str, &'static [&'static str], &'static str, &'static str, u32);
    pub const BOARDS: [DemoBoard; 10] = [
        (
            "Ramadan campaign",
            "Key visual for Noor Foods — the portrait poster and its story cut-down.",
            &["client", "ramadan", "social"],
            "Design/Clients/Noor Foods/Ramadan 2026",
            "Today 14:32",
            2,
        ),
        (
            "Logo marks v3",
            "Third round of marks for Atlas Coffee. Free board, no artboards yet.",
            &["client", "logo"],
            "Design/Clients/Atlas Coffee/Identity",
            "Today 10:05",
            0,
        ),
        ("Form poster", "", &["personal", "print"], "Design/Personal/Posters", "Yesterday 22:41", 1),
        (
            "Icon set 24",
            "24 px outline icons, 1.5 stroke, for the product app.",
            &["product"],
            "Work/Product/Icons",
            "Yesterday 16:10",
            0,
        ),
        ("Story launch", "", &["client", "social"], "Design/Clients/Noor Foods/Social", "2 Oct", 1),
        (
            "Business card",
            "Front and back, 85 × 55 mm.",
            &["client", "print"],
            "Design/Clients/Atlas Coffee/Print",
            "30 Sep",
            2,
        ),
        (
            "Pattern tiles",
            "Quarter-circle tiles, still exploring the repeat.",
            &["personal"],
            "Design/Explorations",
            "28 Sep",
            0,
        ),
        ("Invoice template", "", &["studio", "print"], "Documents/Studio/Admin", "21 Sep", 1),
        (
            "Eid greetings",
            "Square post for Eid al-Adha, two colourways.",
            &["client", "social"],
            "Design/Clients/Noor Foods/Social",
            "17 Sep",
            1,
        ),
        (
            "Type specimen",
            "Display sizes and figures for the studio site.",
            &["personal", "type"],
            "Design/Explorations/Type",
            "9 Sep",
            0,
        ),
    ];

    /// The ten boards under `home`; board index `missing` (if any) is Missing.
    pub fn cards(home: &std::path::Path, missing: Option<usize>) -> Vec<BoardCard> {
        BOARDS
            .iter()
            .enumerate()
            .map(|(i, (name, desc, tags, folder, date, artboards))| BoardCard {
                key: format!("board-{i}"),
                name: name.to_string(),
                description: (!desc.is_empty()).then(|| desc.to_string()),
                tags: tags.iter().map(|t| t.to_string()).collect(),
                artboards: *artboards,
                path: home.join(folder).join(format!("{name}.vrs")),
                modified: date.to_string(),
                missing: missing == Some(i),
                thumb: None,
            })
            .collect()
    }

    /// Tag counts in filter order: count desc, then name.
    pub fn tags(cards: &[BoardCard]) -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = vec![];
        for tag in cards.iter().flat_map(|c| c.tags.iter()) {
            match counts.iter_mut().find(|(t, _)| t == tag) {
                Some((_, n)) => *n += 1,
                None => counts.push((tag.clone(), 1)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        counts
    }

    pub fn recovered() -> RecoveredRow {
        RecoveredRow {
            id: "recovery-menu-card".into(),
            name: "Menu card".into(),
            when: "unsaved changes from 11:48 today".into(),
            folder: Some("~/Design/Clients/Noor Foods/Menu".into()),
            problem: None,
            busy: false,
        }
    }

    /// The mockup's `inter-recent` state (Missing on `missing`), filtered the way the model would.
    pub fn view(home: &std::path::Path, missing: Option<usize>, filter: Option<&str>, search: &str) -> StartView {
        let all = cards(home, missing);
        let tags = tags(&all);
        let total = all.len();
        let needle = search.to_lowercase();
        let cards = all
            .into_iter()
            .filter(|c| filter.is_none_or(|f| c.tags.iter().any(|t| t == f)))
            .filter(|c| {
                needle.is_empty()
                    || c.name.to_lowercase().contains(&needle)
                    || c.description.as_deref().is_some_and(|d| d.to_lowercase().contains(&needle))
                    || c.tags.iter().any(|t| t.contains(&needle))
            })
            .collect();
        StartView {
            cards,
            recovered: vec![recovered()],
            tags,
            total,
            filter: filter.map(str::to_string),
            search: search.to_string(),
            view: ViewMode::Grid,
            has_tabs: true,
        }
    }
}
