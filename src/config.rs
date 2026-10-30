use std::path::{Path, PathBuf};

use rune_cfg::RuneConfig;

use crate::mode::LiftMode;

/// Embedded default config template. Written to the user config path on first
/// launch so users have a documented starting point instead of silently running
/// on baked-in defaults.
const DEFAULT_CONFIG_TEMPLATE: &str = include_str!("../lift.default.rune");

#[derive(Clone, Debug)]
pub struct LiftConfig {
    pub placeholder: String,
    pub width: u32,
    pub max_results: usize,
    pub visible_results: usize,
    pub fuzzy: bool,
    pub show_section_labels: bool,
    pub icons: bool,
    pub icon_size: u32,
    pub icon_search_depth: usize,
    pub icon_theme: String,
    pub terminal: String,
    pub close_on_focus_loss: bool,
    pub alt_number_jump: bool,
    pub ui: LiftUiConfig,
    pub position: LiftPositionConfig,
    pub rounding: LiftRoundingConfig,
    pub colors: LiftColorConfig,
    pub border: LiftBorderConfig,
    pub search_icon: LiftSearchIconConfig,
    pub cursor: LiftCursorConfig,
    pub modes: LiftModeConfig,
    pub providers: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct LiftBorderConfig {
    /// Whether to draw a border at all.
    pub enabled: bool,
    /// Border thickness in pixels.
    pub width: i32,
    /// "outline" wraps the whole app (search bar when collapsed, search +
    /// results as one unit when expanded). "inset" keeps the legacy look where
    /// only the results dropdown is bordered.
    pub style: String,
}

#[derive(Clone, Debug)]
pub struct LiftPositionConfig {
    pub anchor: String,
    pub offset_x: i32,
    pub offset_y: i32,
}

#[derive(Clone, Debug)]
pub struct LiftRoundingConfig {
    pub panel: i32,
    pub dropdown: i32,
    pub search: i32,
    pub row: i32,
    pub badge: i32,
    pub draft: i32,
}

#[derive(Clone, Debug)]
pub struct LiftColorConfig {
    pub panel: String,
    pub panel_border: String,
    pub dropdown: String,
    pub dropdown_border: String,
    pub search: String,
    pub row_selected: String,
    /// One-pixel separator between connected search and dropdown chrome.
    /// A fully transparent color hides it.
    pub divider: String,
    pub text: String,
    pub subtext: String,
    pub hint: String,
    pub accent: String,
    pub badge: String,
    pub danger: String,
    /// Tint for the search-bar magnifier. Empty falls back to the hint color.
    pub search_icon: String,
    /// Tint for result-list icons (fallback shapes and tinted glyphs).
    /// Empty falls back to the accent color.
    pub icon: String,
    /// Tint for the Alt+<n> jump hint labels. Empty falls back to the hint color.
    pub alt_hint: String,
}

#[derive(Clone, Debug)]
pub struct LiftSearchIconConfig {
    /// Whether to show the magnifier glyph in the search bar.
    pub enabled: bool,
    /// "left" or "right" of the search text.
    pub side: String,
    /// Glyph size in pixels.
    pub size: i32,
}

#[derive(Clone, Debug)]
pub struct LiftCursorConfig {
    pub enabled: bool,
    pub width: i32,
    pub blink_ms: u64,
    pub stop_blink_after_ms: u64,
}

#[derive(Clone, Debug)]
pub struct LiftUiConfig {
    pub top_margin: i32,
    pub padding: i32,
    pub search_gap: i32,
    pub dropdown_gap: i32,
    pub dropdown_padding: i32,
    pub results_padding: i32,
    pub results_bottom_gap: i32,
    pub row_padding: i32,
    pub row_content_gap: i32,
    pub row_text_gap: i32,
    pub marker_size: i32,
    pub draft_padding: i32,
    pub status_padding: i32,
    pub hover_scroll: bool,
    pub hover_scroll_delay_ms: u64,
    pub hover_scroll_speed: u32,
    pub hover_scroll_pause_ms: u64,
    pub hover_scroll_transition_ms: u64,
    pub search_height: i32,
    pub draft_height: i32,
    pub row_height: i32,
    pub row_gap: i32,
    pub section_height: i32,
    pub footer_height: i32,
    pub font: String,
    pub search_font_size: u32,
    pub badge_font_size: u32,
    pub title_font_size: u32,
    pub subtitle_font_size: u32,
    pub hint_font_size: u32,
}

#[derive(Clone, Debug)]
pub struct LiftModeConfig {
    pub apps: bool,
    pub clusters: bool,
    pub nodes: bool,
    pub actions: bool,
    pub config: bool,
}

impl Default for LiftConfig {
    fn default() -> Self {
        Self {
            placeholder: "Search apps, nodes, clusters, actions...".into(),
            width: 760,
            max_results: 12,
            visible_results: 8,
            fuzzy: true,
            show_section_labels: true,
            icons: true,
            icon_size: 28,
            icon_search_depth: 5,
            icon_theme: "auto".into(),
            terminal: "x-terminal-emulator -e".into(),
            close_on_focus_loss: false,
            alt_number_jump: true,
            ui: LiftUiConfig::default(),
            position: LiftPositionConfig::default(),
            rounding: LiftRoundingConfig::default(),
            colors: LiftColorConfig::default(),
            border: LiftBorderConfig::default(),
            search_icon: LiftSearchIconConfig::default(),
            cursor: LiftCursorConfig::default(),
            modes: LiftModeConfig::default(),
            providers: vec![
                "apps".into(),
                "clusters".into(),
                "nodes".into(),
                "actions".into(),
            ],
        }
    }
}

impl Default for LiftPositionConfig {
    fn default() -> Self {
        Self {
            anchor: "center".into(),
            offset_x: 0,
            offset_y: 0,
        }
    }
}

impl Default for LiftRoundingConfig {
    fn default() -> Self {
        Self {
            panel: 12,
            dropdown: 12,
            search: 12,
            row: 12,
            badge: 10,
            draft: 10,
        }
    }
}

impl Default for LiftColorConfig {
    fn default() -> Self {
        Self {
            panel: "#151720ee".into(),
            panel_border: "#2b3248cc".into(),
            dropdown: "#151720ee".into(),
            dropdown_border: "#2b3248cc".into(),
            search: "#090b12d8".into(),
            row_selected: "#2e4575ea".into(),
            divider: "#2b324899".into(),
            text: "#f2f5ff".into(),
            subtext: "#9ea7bf".into(),
            hint: "#858fa8".into(),
            accent: "#8fb5ff".into(),
            badge: "#334875f2".into(),
            danger: "#eb9a8f".into(),
            search_icon: String::new(),
            icon: String::new(),
            alt_hint: String::new(),
        }
    }
}

impl Default for LiftBorderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 1,
            style: "outline".into(),
        }
    }
}

impl Default for LiftSearchIconConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            side: "left".into(),
            size: 22,
        }
    }
}

impl Default for LiftCursorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 2,
            blink_ms: 500,
            stop_blink_after_ms: 5000,
        }
    }
}

impl Default for LiftUiConfig {
    fn default() -> Self {
        Self {
            top_margin: 96,
            padding: 20,
            search_gap: 12,
            dropdown_gap: 0,
            dropdown_padding: 10,
            results_padding: 20,
            results_bottom_gap: 8,
            row_padding: 5,
            row_content_gap: 16,
            row_text_gap: 7,
            marker_size: 18,
            draft_padding: 14,
            status_padding: 3,
            hover_scroll: true,
            hover_scroll_delay_ms: 900,
            hover_scroll_speed: 30,
            hover_scroll_pause_ms: 1200,
            hover_scroll_transition_ms: 180,
            search_height: 60,
            draft_height: 36,
            row_height: 64,
            row_gap: 6,
            section_height: 22,
            footer_height: 0,
            font: "sans-serif".into(),
            search_font_size: 22,
            badge_font_size: 15,
            title_font_size: 17,
            subtitle_font_size: 13,
            hint_font_size: 12,
        }
    }
}

impl Default for LiftModeConfig {
    fn default() -> Self {
        Self {
            apps: true,
            clusters: true,
            nodes: true,
            actions: true,
            config: true,
        }
    }
}

impl LiftModeConfig {
    pub fn enabled(&self, mode: LiftMode) -> bool {
        match mode {
            LiftMode::General | LiftMode::Term => true,
            LiftMode::Apps => self.apps,
            LiftMode::Clusters => self.clusters,
            LiftMode::Nodes => self.nodes,
            LiftMode::Actions => self.actions,
            LiftMode::Config => self.config,
        }
    }
}

// New paths take precedence; legacy paths keep existing files and gathered themes working.
struct CompatibleConfig(RuneConfig);

impl CompatibleConfig {
    fn get_or<T>(&self, legacy: &str, default: T) -> T
    where
        T: TryFrom<rune_cfg::Value, Error = rune_cfg::RuneError>,
    {
        let default = self.0.get_or(legacy, default);
        match current_path(legacy) {
            Some(path) => self.0.get_or(path, default),
            None => default,
        }
    }

    fn get_optional<T>(&self, legacy: &str) -> Result<Option<T>, rune_cfg::RuneError>
    where
        T: TryFrom<rune_cfg::Value, Error = rune_cfg::RuneError>,
    {
        if let Some(path) = current_path(legacy) {
            if let Some(value) = self.0.get_optional(path)? {
                return Ok(Some(value));
            }
        }
        self.0.get_optional(legacy)
    }
}

fn current_path(legacy: &str) -> Option<&'static str> {
    match legacy {
        "lift.alt-number-jump" => Some("behavior.alt-number-jump"),
        "lift.border.enabled" => Some("border.enabled"),
        "lift.border.style" => Some("border.style"),
        "lift.border.width" => Some("border.width"),
        "lift.close-on-focus-loss" => Some("behavior.close-on-focus-loss"),
        "lift.colors.accent" => Some("colors.accent"),
        "lift.colors.alt-hint" => Some("colors.alt-hint"),
        "lift.colors.badge" => Some("colors.badge"),
        "lift.colors.danger" => Some("colors.danger"),
        "lift.colors.divider" => Some("colors.divider"),
        "lift.colors.dropdown" => Some("colors.dropdown"),
        "lift.colors.dropdown-border" => Some("colors.dropdown-border"),
        "lift.colors.hint" => Some("colors.hint"),
        "lift.colors.icon" => Some("colors.icon"),
        "lift.colors.panel" => Some("colors.panel"),
        "lift.colors.panel-border" => Some("colors.panel-border"),
        "lift.colors.row-selected" => Some("colors.row-selected"),
        "lift.colors.search" => Some("colors.search"),
        "lift.colors.search-icon" => Some("colors.search-icon"),
        "lift.colors.subtext" => Some("colors.subtext"),
        "lift.colors.text" => Some("colors.text"),
        "lift.cursor.blink-ms" => Some("cursor.blink-ms"),
        "lift.cursor.enabled" => Some("cursor.enabled"),
        "lift.cursor.stop-blink-after-ms" => Some("cursor.stop-blink-after-ms"),
        "lift.cursor.width" => Some("cursor.width"),
        "lift.fuzzy" => Some("search.fuzzy"),
        "lift.icon-size" => Some("icons.size"),
        "lift.icon-theme" => Some("icons.theme"),
        "lift.icons" => Some("icons.enabled"),
        "lift.max-results" => Some("search.max-results"),
        "lift.modes.actions" => Some("search.modes.actions"),
        "lift.modes.apps" => Some("search.modes.apps"),
        "lift.modes.clusters" => Some("search.modes.clusters"),
        "lift.modes.config" => Some("search.modes.config"),
        "lift.modes.nodes" => Some("search.modes.nodes"),
        "lift.placeholder" => Some("search.placeholder"),
        "lift.position.anchor" => Some("position.anchor"),
        "lift.position.offset-x" => Some("position.offset-x"),
        "lift.position.offset-y" => Some("position.offset-y"),
        "lift.providers" => Some("search.providers"),
        "lift.rounding.badge" => Some("rounding.badge"),
        "lift.rounding.draft" => Some("rounding.draft"),
        "lift.rounding.dropdown" => Some("rounding.dropdown"),
        "lift.rounding.panel" => Some("rounding.panel"),
        "lift.rounding.row" => Some("rounding.row"),
        "lift.rounding.search" => Some("rounding.search"),
        "lift.search-icon.enabled" => Some("search-icon.enabled"),
        "lift.search-icon.side" => Some("search-icon.side"),
        "lift.search-icon.size" => Some("search-icon.size"),
        "lift.show-section-labels" => Some("search.show-section-labels"),
        "lift.terminal" => Some("behavior.terminal"),
        "lift.ui.badge-font-size" => Some("typography.badge-font-size"),
        "lift.ui.draft-height" => Some("layout.draft.height"),
        "lift.ui.draft-padding" => Some("layout.draft.padding"),
        "lift.ui.dropdown-gap" => Some("layout.dropdown.gap"),
        "lift.ui.dropdown-padding" => Some("layout.dropdown.padding"),
        "lift.ui.font" => Some("typography.font"),
        "lift.ui.footer-height" => Some("layout.results.footer-height"),
        "lift.ui.hint-font-size" => Some("typography.hint-font-size"),
        "lift.ui.hover-scroll" => Some("text-scroll.enabled"),
        "lift.ui.hover-scroll-delay-ms" => Some("text-scroll.delay-ms"),
        "lift.ui.hover-scroll-pause-ms" => Some("text-scroll.pause-ms"),
        "lift.ui.hover-scroll-speed" => Some("text-scroll.speed"),
        "lift.ui.hover-scroll-transition-ms" => Some("text-scroll.transition-ms"),
        "lift.ui.marker-size" => Some("layout.row.cluster-marker-size"),
        "lift.ui.padding" => Some("layout.search.padding"),
        "lift.ui.results-bottom-gap" => Some("layout.results.bottom-gap"),
        "lift.ui.results-padding" => Some("layout.results.padding"),
        "lift.ui.row-content-gap" => Some("layout.row.content-gap"),
        "lift.ui.row-gap" => Some("layout.row.gap"),
        "lift.ui.row-height" => Some("layout.row.height"),
        "lift.ui.row-padding" => Some("layout.row.padding"),
        "lift.ui.row-text-gap" => Some("layout.row.text-gap"),
        "lift.ui.search-font-size" => Some("typography.search-font-size"),
        "lift.ui.search-gap" => Some("layout.search.icon-gap"),
        "lift.ui.search-height" => Some("layout.search.height"),
        "lift.ui.section-height" => Some("layout.results.section-height"),
        "lift.ui.status-padding" => Some("layout.status.padding"),
        "lift.ui.subtitle-font-size" => Some("typography.subtitle-font-size"),
        "lift.ui.title-font-size" => Some("typography.title-font-size"),
        "lift.ui.top-margin" => Some("position.top-margin"),
        "lift.visible-results" => Some("search.visible-results"),
        "lift.width" => Some("layout.width"),
        _ => None,
    }
}

impl LiftConfig {
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let cfg = CompatibleConfig(
            RuneConfig::from_file(path)
                .map_err(|err| format!("failed to parse {}: {err}", path.display()))?,
        );
        let mut out = Self::default();
        out.placeholder = cfg.get_or("lift.placeholder", out.placeholder.clone());
        out.width = cfg.get_or("lift.width", out.width).clamp(420, 1400);
        out.max_results = cfg
            .get_or::<u32>("lift.max-results", out.max_results as u32)
            .clamp(3, 40) as usize;
        out.visible_results = cfg
            .get_or::<u32>("lift.visible-results", out.visible_results as u32)
            .clamp(3, out.max_results as u32) as usize;
        out.fuzzy = cfg.get_or("lift.fuzzy", out.fuzzy);
        out.show_section_labels = cfg.get_or("lift.show-section-labels", out.show_section_labels);
        out.icons = cfg.get_or("lift.icons", out.icons);
        out.icon_size = cfg
            .get_or::<u32>("lift.icon-size", out.icon_size)
            .clamp(16, 64);
        out.icon_search_depth = cfg
            .get_or::<u32>("lift.icon-search-depth", out.icon_search_depth as u32)
            .clamp(1, 8) as usize;
        out.icon_theme = cfg.get_or("lift.icon-theme", out.icon_theme.clone());
        out.terminal = cfg.get_or("lift.terminal", out.terminal.clone());
        out.close_on_focus_loss = cfg.get_or("lift.close-on-focus-loss", out.close_on_focus_loss);
        out.alt_number_jump = cfg.get_or("lift.alt-number-jump", out.alt_number_jump);
        out.position.anchor = cfg.get_or("lift.position.anchor", out.position.anchor.clone());
        out.position.offset_x = cfg
            .get_or("lift.position.offset-x", out.position.offset_x)
            .clamp(-2000, 2000);
        out.position.offset_y = cfg
            .get_or("lift.position.offset-y", out.position.offset_y)
            .clamp(-2000, 2000);
        out.rounding.panel = cfg
            .get_or("lift.rounding.panel", out.rounding.panel)
            .clamp(0, 48);
        out.rounding.dropdown = cfg
            .get_or("lift.rounding.dropdown", out.rounding.dropdown)
            .clamp(0, 48);
        out.rounding.search = cfg
            .get_or("lift.rounding.search", out.rounding.search)
            .clamp(0, 48);
        out.rounding.row = cfg
            .get_or("lift.rounding.row", out.rounding.row)
            .clamp(0, 48);
        out.rounding.badge = cfg
            .get_or("lift.rounding.badge", out.rounding.badge)
            .clamp(0, 48);
        out.rounding.draft = cfg
            .get_or("lift.rounding.draft", out.rounding.draft)
            .clamp(0, 48);
        out.colors.panel = cfg.get_or("lift.colors.panel", out.colors.panel.clone());
        out.colors.panel_border =
            cfg.get_or("lift.colors.panel-border", out.colors.panel_border.clone());
        out.colors.dropdown = cfg.get_or("lift.colors.dropdown", out.colors.dropdown.clone());
        out.colors.dropdown_border = cfg.get_or(
            "lift.colors.dropdown-border",
            out.colors.dropdown_border.clone(),
        );
        out.colors.search = cfg.get_or("lift.colors.search", out.colors.search.clone());
        out.colors.row_selected =
            cfg.get_or("lift.colors.row-selected", out.colors.row_selected.clone());
        out.colors.divider = cfg.get_or("lift.colors.divider", out.colors.divider.clone());
        out.colors.text = cfg.get_or("lift.colors.text", out.colors.text.clone());
        out.colors.subtext = cfg.get_or("lift.colors.subtext", out.colors.subtext.clone());
        out.colors.hint = cfg.get_or("lift.colors.hint", out.colors.hint.clone());
        out.colors.accent = cfg.get_or("lift.colors.accent", out.colors.accent.clone());
        out.colors.badge = cfg.get_or("lift.colors.badge", out.colors.badge.clone());
        out.colors.danger = cfg.get_or("lift.colors.danger", out.colors.danger.clone());
        out.border.enabled = cfg.get_or("lift.border.enabled", out.border.enabled);
        out.border.width = cfg
            .get_or("lift.border.width", out.border.width)
            .clamp(0, 16);
        out.border.style = cfg.get_or("lift.border.style", out.border.style.clone());
        out.search_icon.enabled = cfg.get_or("lift.search-icon.enabled", out.search_icon.enabled);
        out.search_icon.side = cfg.get_or("lift.search-icon.side", out.search_icon.side.clone());
        out.search_icon.size = cfg
            .get_or("lift.search-icon.size", out.search_icon.size)
            .clamp(8, 48);
        out.colors.search_icon =
            cfg.get_or("lift.colors.search-icon", out.colors.search_icon.clone());
        out.colors.icon = cfg.get_or("lift.colors.icon", out.colors.icon.clone());
        out.colors.alt_hint = cfg.get_or("lift.colors.alt-hint", out.colors.alt_hint.clone());
        out.cursor.enabled = cfg.get_or("lift.cursor.enabled", out.cursor.enabled);
        out.cursor.width = cfg
            .get_or("lift.cursor.width", out.cursor.width)
            .clamp(1, 8);
        out.cursor.blink_ms = cfg
            .get_or::<u64>("lift.cursor.blink-ms", out.cursor.blink_ms)
            .clamp(150, 2000);
        let stop_blink_after_ms = cfg.get_or::<u64>(
            "lift.cursor.stop-blink-after-ms",
            out.cursor.stop_blink_after_ms,
        );
        out.cursor.stop_blink_after_ms = if stop_blink_after_ms == 0 {
            0
        } else {
            stop_blink_after_ms.clamp(1000, 60_000)
        };
        out.ui.top_margin = cfg
            .get_or("lift.ui.top-margin", out.ui.top_margin)
            .clamp(0, 480);
        out.ui.padding = cfg.get_or("lift.ui.padding", out.ui.padding).clamp(0, 48);
        out.ui.search_gap = cfg
            .get_or("lift.ui.search-gap", out.ui.search_gap)
            .clamp(0, 48);
        out.ui.dropdown_gap = cfg
            .get_or("lift.ui.dropdown-gap", out.ui.dropdown_gap)
            .clamp(0, 32);
        out.ui.dropdown_padding = cfg
            .get_or("lift.ui.dropdown-padding", out.ui.dropdown_padding)
            .clamp(0, 32);
        out.ui.search_height = cfg
            .get_or("lift.ui.search-height", out.ui.search_height)
            .clamp(42, 96);
        out.ui.draft_height = cfg
            .get_or("lift.ui.draft-height", out.ui.draft_height)
            .clamp(28, 72);
        out.ui.row_height = cfg
            .get_or("lift.ui.row-height", out.ui.row_height)
            .clamp(46, 110);
        out.ui.row_gap = cfg.get_or("lift.ui.row-gap", out.ui.row_gap).clamp(0, 20);
        // Existing configs keep their shared inset and bottom-gap behavior
        // until they explicitly set the independent results spacing.
        out.ui.results_padding = cfg
            .get_or("lift.ui.results-padding", out.ui.padding)
            .clamp(0, 48);
        out.ui.results_bottom_gap = cfg
            .get_or("lift.ui.results-bottom-gap", out.ui.row_gap.max(8))
            .clamp(0, 32);
        out.ui.row_padding = cfg
            .get_or("lift.ui.row-padding", out.ui.row_padding)
            .clamp(0, 32);
        out.ui.row_content_gap = cfg
            .get_or("lift.ui.row-content-gap", out.ui.row_content_gap)
            .clamp(0, 48);
        out.ui.row_text_gap = cfg
            .get_or("lift.ui.row-text-gap", out.ui.row_text_gap)
            .clamp(0, 32);
        out.ui.marker_size = cfg
            .get_or("lift.ui.marker-size", out.ui.marker_size)
            .clamp(8, 32);
        out.ui.draft_padding = cfg
            .get_or("lift.ui.draft-padding", out.ui.draft_padding)
            .clamp(0, 48);
        out.ui.status_padding = cfg
            .get_or("lift.ui.status-padding", out.ui.status_padding)
            .clamp(0, 16);
        out.ui.hover_scroll = cfg.get_or("lift.ui.hover-scroll", out.ui.hover_scroll);
        out.ui.hover_scroll_delay_ms = cfg
            .get_or(
                "lift.ui.hover-scroll-delay-ms",
                out.ui.hover_scroll_delay_ms,
            )
            .clamp(0, 5000);
        out.ui.hover_scroll_speed = cfg
            .get_or("lift.ui.hover-scroll-speed", out.ui.hover_scroll_speed)
            .clamp(8, 160);
        out.ui.hover_scroll_pause_ms = cfg
            .get_or(
                "lift.ui.hover-scroll-pause-ms",
                out.ui.hover_scroll_pause_ms,
            )
            .clamp(0, 5000);
        out.ui.hover_scroll_transition_ms = cfg
            .get_or(
                "lift.ui.hover-scroll-transition-ms",
                out.ui.hover_scroll_transition_ms,
            )
            .clamp(50, 600);
        out.ui.section_height = cfg
            .get_or("lift.ui.section-height", out.ui.section_height)
            .clamp(0, 40);
        out.ui.footer_height = cfg
            .get_or("lift.ui.footer-height", out.ui.footer_height)
            .clamp(0, 70);
        out.ui.font = cfg.get_or("lift.ui.font", out.ui.font.clone());
        out.ui.search_font_size = cfg
            .get_or::<u32>("lift.ui.search-font-size", out.ui.search_font_size)
            .clamp(14, 42);
        out.ui.badge_font_size = cfg
            .get_or::<u32>("lift.ui.badge-font-size", out.ui.badge_font_size)
            .clamp(10, 28);
        out.ui.title_font_size = cfg
            .get_or::<u32>("lift.ui.title-font-size", out.ui.title_font_size)
            .clamp(12, 34);
        out.ui.subtitle_font_size = cfg
            .get_or::<u32>("lift.ui.subtitle-font-size", out.ui.subtitle_font_size)
            .clamp(10, 26);
        out.ui.hint_font_size = cfg
            .get_or::<u32>("lift.ui.hint-font-size", out.ui.hint_font_size)
            .clamp(10, 24);
        out.modes.apps = cfg.get_or("lift.modes.apps", out.modes.apps);
        out.modes.clusters = cfg.get_or("lift.modes.clusters", out.modes.clusters);
        out.modes.nodes = cfg.get_or("lift.modes.nodes", out.modes.nodes);
        out.modes.actions = cfg.get_or("lift.modes.actions", out.modes.actions);
        out.modes.config = cfg.get_or("lift.modes.config", out.modes.config);
        if let Some(providers) = cfg
            .get_optional::<Vec<String>>("lift.providers")
            .map_err(|error| format!("invalid lift.providers: {error}"))?
        {
            out.providers = providers;
        }
        validate(&out)?;
        Ok(out)
    }
}

fn known_provider(value: &str) -> bool {
    matches!(value, "apps" | "clusters" | "nodes" | "actions" | "config")
}

fn validate(config: &LiftConfig) -> Result<(), String> {
    if let Some(provider) = config
        .providers
        .iter()
        .find(|provider| !known_provider(provider))
    {
        return Err(format!(
            "unknown lift.providers entry {provider:?}; expected apps, clusters, nodes, actions, or config"
        ));
    }
    if config.width < 420 {
        return Err("lift.width must be at least 420".into());
    }
    if config.max_results == 0 {
        return Err("lift.max-results must be greater than zero".into());
    }
    if config.terminal.trim().is_empty() {
        return Err("lift.terminal must not be empty".into());
    }
    if config.visible_results == 0 || config.visible_results > config.max_results {
        return Err("lift.visible-results must be between 1 and lift.max-results".into());
    }
    if !matches!(
        config.position.anchor.to_ascii_lowercase().as_str(),
        "center" | "top" | "top-left" | "top-right" | "bottom" | "bottom-left" | "bottom-right"
    ) {
        return Err("lift.position.anchor must be center, top, top-left, top-right, bottom, bottom-left, or bottom-right".into());
    }
    Ok(())
}

pub fn default_config_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Path::new(&xdg).join("halley/lift.rune");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Path::new(&home).join(".config/halley/lift.rune");
    }
    PathBuf::from("lift.rune")
}

/// Ensures a user config exists at [`default_config_path`]. If the file is
/// missing, creates its parent directory and writes the embedded default
/// template. Failures are non-fatal: Lift can still run on baked-in defaults,
/// so errors are reported via stderr and swallowed.
pub fn bootstrap_default_config() {
    let path = default_config_path();
    if path.exists() {
        return;
    }
    let Some(parent) = path.parent() else {
        return;
    };
    if let Err(err) = std::fs::create_dir_all(parent) {
        eprintln!(
            "halley-lift: bootstrap: failed to create config directory {}: {err}",
            parent.display()
        );
        return;
    }
    if let Err(err) = std::fs::write(&path, DEFAULT_CONFIG_TEMPLATE) {
        eprintln!(
            "halley-lift: bootstrap: failed to write {}: {err}",
            path.display()
        );
    }
}

pub fn resolved_halley_config_path() -> PathBuf {
    if let Ok(path) = std::env::var("HALLEY_WL_CONFIG") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return absolutize_path(trimmed);
        }
    }

    let user_path = default_halley_user_config_path();
    if user_path.exists() {
        return user_path;
    }

    let system_path = PathBuf::from("/etc/halley/halley.rune");
    if system_path.exists() {
        return system_path;
    }

    user_path
}

fn default_halley_user_config_path() -> PathBuf {
    if let Ok(home) = std::env::var("XDG_CONFIG_HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return Path::new(trimmed).join("halley/halley.rune");
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return Path::new(trimmed).join(".config/halley/halley.rune");
        }
    }

    PathBuf::from("halley.rune")
}

fn absolutize_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&path))
            .unwrap_or(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn load_text(text: &str) -> Result<LiftConfig, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lift-config-{}-{}.rune",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, text).unwrap();
        let result = LiftConfig::load(&path);
        std::fs::remove_file(path).unwrap();
        result
    }

    #[test]
    fn settings_parse_and_empty_providers_do_not_restore_defaults() {
        let config = load_text(
            r##"
lift:
  fuzzy false
  providers []
  modes:
    nodes false
    clusters false
  end
  colors:
    panel "#164b80dd"
    search ""
    dropdown ""
  end
end
"##,
        )
        .unwrap();
        assert!(!config.fuzzy);
        assert!(config.providers.is_empty());
        assert!(!config.modes.nodes);
        assert!(!config.modes.clusters);
        assert!(config.modes.apps);
        assert_eq!(config.colors.panel, "#164b80dd");
        assert!(config.colors.search.is_empty());
        assert!(config.colors.dropdown.is_empty());
    }

    #[test]
    fn invalid_provider_names_and_types_are_reported() {
        let error = load_text("lift:\n  providers [\"apps\" \"typo\"]\nend\n").unwrap_err();
        assert!(error.contains("unknown lift.providers entry"));
        assert!(error.contains("typo"));
        let error = load_text("lift:\n  providers 42\nend\n").unwrap_err();
        assert!(error.contains("lift.providers"));
    }

    #[test]
    fn spacing_settings_allow_zero_and_preserve_legacy_inheritance() {
        let legacy =
            load_text("lift:\n  ui:\n    padding 13\n    row-gap 12\n  end\nend\n").unwrap();
        assert_eq!(legacy.ui.results_padding, 13);
        assert_eq!(legacy.ui.results_bottom_gap, 12);
        let explicit = load_text(
            "lift:\n  ui:\n    padding 13\n    row-gap 12\n    search-gap 0\n    results-padding 0\n    row-padding 0\n    row-content-gap 0\n    row-text-gap 0\n    marker-size 24\n    draft-padding 0\n    results-bottom-gap 0\n    status-padding 0\n  end\nend\n",
        ).unwrap();
        assert_eq!(explicit.ui.padding, 13);
        assert_eq!(explicit.ui.search_gap, 0);
        assert_eq!(explicit.ui.results_padding, 0);
        assert_eq!(explicit.ui.row_padding, 0);
        assert_eq!(explicit.ui.row_content_gap, 0);
        assert_eq!(explicit.ui.row_text_gap, 0);
        assert_eq!(explicit.ui.marker_size, 24);
        assert_eq!(explicit.ui.draft_padding, 0);
        assert_eq!(explicit.ui.results_bottom_gap, 0);
        assert_eq!(explicit.ui.status_padding, 0);
        let bounded = load_text(
            "lift:\n  ui:\n    padding -1\n    results-padding 999\n    row-padding -1\n    row-content-gap 999\n    row-text-gap -1\n    marker-size 0\n    search-gap -1\n    draft-padding 999\n    results-bottom-gap 999\n    status-padding -1\n  end\nend\n",
        ).unwrap();
        assert_eq!(bounded.ui.padding, 0);
        assert_eq!(bounded.ui.results_padding, 48);
        assert_eq!(bounded.ui.row_padding, 0);
        assert_eq!(bounded.ui.row_content_gap, 48);
        assert_eq!(bounded.ui.row_text_gap, 0);
        assert_eq!(bounded.ui.marker_size, 8);
        assert_eq!(bounded.ui.search_gap, 0);
        assert_eq!(bounded.ui.draft_padding, 48);
        assert_eq!(bounded.ui.results_bottom_gap, 32);
        assert_eq!(bounded.ui.status_padding, 0);
    }

    #[test]
    fn hover_scroll_settings_parse_and_bound_animation_timing() {
        let config = load_text("lift:\n  ui:\n    hover-scroll false\n    hover-scroll-delay-ms 99999\n    hover-scroll-speed 0\n    hover-scroll-pause-ms 0\n    hover-scroll-transition-ms 0\n  end\nend\n").unwrap();
        assert!(!config.ui.hover_scroll);
        assert_eq!(config.ui.hover_scroll_delay_ms, 5000);
        assert_eq!(config.ui.hover_scroll_speed, 8);
        assert_eq!(config.ui.hover_scroll_pause_ms, 0);
        assert_eq!(config.ui.hover_scroll_transition_ms, 50);
    }

    const LEGACY_PATHS: &[&str] = &[
        "lift.alt-number-jump",
        "lift.border.enabled",
        "lift.border.style",
        "lift.border.width",
        "lift.close-on-focus-loss",
        "lift.colors.accent",
        "lift.colors.alt-hint",
        "lift.colors.badge",
        "lift.colors.danger",
        "lift.colors.divider",
        "lift.colors.dropdown",
        "lift.colors.dropdown-border",
        "lift.colors.hint",
        "lift.colors.icon",
        "lift.colors.panel",
        "lift.colors.panel-border",
        "lift.colors.row-selected",
        "lift.colors.search",
        "lift.colors.search-icon",
        "lift.colors.subtext",
        "lift.colors.text",
        "lift.cursor.blink-ms",
        "lift.cursor.enabled",
        "lift.cursor.stop-blink-after-ms",
        "lift.cursor.width",
        "lift.fuzzy",
        "lift.icon-size",
        "lift.icon-theme",
        "lift.icons",
        "lift.max-results",
        "lift.modes.actions",
        "lift.modes.apps",
        "lift.modes.clusters",
        "lift.modes.config",
        "lift.modes.nodes",
        "lift.placeholder",
        "lift.position.anchor",
        "lift.position.offset-x",
        "lift.position.offset-y",
        "lift.providers",
        "lift.rounding.badge",
        "lift.rounding.draft",
        "lift.rounding.dropdown",
        "lift.rounding.panel",
        "lift.rounding.row",
        "lift.rounding.search",
        "lift.search-icon.enabled",
        "lift.search-icon.side",
        "lift.search-icon.size",
        "lift.show-section-labels",
        "lift.terminal",
        "lift.ui.badge-font-size",
        "lift.ui.draft-height",
        "lift.ui.draft-padding",
        "lift.ui.dropdown-gap",
        "lift.ui.dropdown-padding",
        "lift.ui.font",
        "lift.ui.footer-height",
        "lift.ui.hint-font-size",
        "lift.ui.hover-scroll",
        "lift.ui.hover-scroll-delay-ms",
        "lift.ui.hover-scroll-pause-ms",
        "lift.ui.hover-scroll-speed",
        "lift.ui.hover-scroll-transition-ms",
        "lift.ui.marker-size",
        "lift.ui.padding",
        "lift.ui.results-bottom-gap",
        "lift.ui.results-padding",
        "lift.ui.row-content-gap",
        "lift.ui.row-gap",
        "lift.ui.row-height",
        "lift.ui.row-padding",
        "lift.ui.row-text-gap",
        "lift.ui.search-font-size",
        "lift.ui.search-gap",
        "lift.ui.search-height",
        "lift.ui.section-height",
        "lift.ui.status-padding",
        "lift.ui.subtitle-font-size",
        "lift.ui.title-font-size",
        "lift.ui.top-margin",
        "lift.visible-results",
        "lift.width",
    ];

    #[test]
    fn new_paths_match_legacy_paths_and_take_precedence() {
        fn snippet(path: &str, value: &str) -> String {
            let parts: Vec<_> = path.split('.').collect();
            let mut text = String::new();
            for part in &parts[..parts.len() - 1] {
                text.push_str(&format!("{part}:\n"));
            }
            text.push_str(&format!("{} {value}\n", parts[parts.len() - 1]));
            for _ in &parts[..parts.len() - 1] {
                text.push_str("end\n");
            }
            text
        }

        // Exercise every active setting with a non-default value through both
        // formats. This also catches misspelled paths in the default template.
        let mut blocks = Vec::new();
        let mut count = 0;
        for line in DEFAULT_CONFIG_TEMPLATE.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with('@') || line.is_empty() {
                continue;
            }
            if line == "end" {
                blocks.pop();
            } else if let Some(block) = line.strip_suffix(':') {
                blocks.push(block);
            } else {
                let (key, raw) = line.split_once(' ').unwrap();
                let path = format!("{}.{}", blocks.join("."), key);
                let legacy = LEGACY_PATHS
                    .iter()
                    .find(|old| current_path(old) == Some(path.as_str()))
                    .unwrap_or_else(|| panic!("no legacy mapping for {path}"));
                let value = if raw.starts_with('[') {
                    "[\"config\"]"
                } else if raw.starts_with("true") {
                    "false"
                } else if raw.starts_with("false") {
                    "true"
                } else if path == "position.anchor" {
                    "\"top\""
                } else if path == "border.style" {
                    "\"inset\""
                } else if path == "search-icon.side" {
                    "\"right\""
                } else if raw.starts_with('"') {
                    "\"custom\""
                } else if path == "layout.width" {
                    "900"
                } else {
                    "21"
                };
                let old = load_text(&snippet(legacy, value)).unwrap();
                let new = load_text(&snippet(&path, value)).unwrap();
                assert_eq!(format!("{old:?}"), format!("{new:?}"), "{path}");
                count += 1;
            }
        }
        assert_eq!(count, LEGACY_PATHS.len());

        let both = load_text("lift:\n  width 600\n  providers [\"apps\"]\nend\nlayout:\n  width 900\nend\nsearch:\n  providers []\nend\n").unwrap();
        assert_eq!(both.width, 900);
        assert!(both.providers.is_empty());
        assert!(
            load_text("lift:\n  providers [\"apps\"]\nend\nsearch:\n  providers 42\nend\n")
                .is_err()
        );
    }

    #[test]
    fn gathered_new_theme_overrides_new_color_settings() {
        let dir = std::env::temp_dir().join(format!("lift-gather-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let theme = dir.join("theme.rune");
        let config = dir.join("lift.rune");
        std::fs::write(&theme, "colors:\n  panel \"#123456ff\"\nend\n").unwrap();
        std::fs::write(
            &config,
            "gather \"theme.rune\"\ncolors:\n  panel \"#abcdef00\"\nend\n",
        )
        .unwrap();
        assert_eq!(LiftConfig::load(&config).unwrap().colors.panel, "#123456ff");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn documented_templates_keep_default_settings() {
        for text in [
            DEFAULT_CONFIG_TEMPLATE,
            include_str!("../examples/lift.rune"),
        ] {
            let config = load_text(text).unwrap();
            assert!(config.fuzzy);
            let defaults = LiftConfig::default().ui;
            assert_eq!(config.ui.search_gap, defaults.search_gap);
            assert_eq!(config.ui.results_padding, defaults.results_padding);
            assert_eq!(config.ui.results_bottom_gap, defaults.results_bottom_gap);
            assert_eq!(config.ui.row_padding, defaults.row_padding);
            assert_eq!(config.ui.row_content_gap, defaults.row_content_gap);
            assert_eq!(config.ui.row_text_gap, defaults.row_text_gap);
            assert_eq!(config.ui.marker_size, defaults.marker_size);
            assert_eq!(config.ui.draft_padding, defaults.draft_padding);
            assert_eq!(config.ui.status_padding, defaults.status_padding);
            assert_eq!(config.ui.hover_scroll, defaults.hover_scroll);
            assert_eq!(
                config.ui.hover_scroll_delay_ms,
                defaults.hover_scroll_delay_ms
            );
            assert_eq!(config.ui.hover_scroll_speed, defaults.hover_scroll_speed);
            assert_eq!(
                config.ui.hover_scroll_pause_ms,
                defaults.hover_scroll_pause_ms
            );
            assert_eq!(
                config.ui.hover_scroll_transition_ms,
                defaults.hover_scroll_transition_ms
            );
            assert_eq!(config.providers, LiftConfig::default().providers);
            for mode in [
                LiftMode::Apps,
                LiftMode::Nodes,
                LiftMode::Clusters,
                LiftMode::Actions,
                LiftMode::Config,
                LiftMode::Term,
            ] {
                assert!(config.modes.enabled(mode));
            }
        }
    }
}
