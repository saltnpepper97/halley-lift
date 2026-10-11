use halley_ui::assets::ImageData;
use halley_ui::ui::{Align, CardStyle, PaintItem, PreparedView};
// Lift composes components; Halley UI owns layout, text, assets, and pixel drawing.
use crate::config::LiftConfig;
use crate::icons::IconCache;
use crate::mode::{LiftMode, ModeInputState};
use crate::model::{ClusterDraft, LiftResult};
use halley_ui::input::{InputEvent, Key, Modifiers};
use halley_ui::software::{PixelFormat, Surface};
use halley_ui::{
    ActionId, Button, Card, Color, Column, Font, Image, Label, Point, Rect as UiRect, Row,
    TextInput, TextOverflow, TextSystem, Theme, UiView,
};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
#[derive(Clone, Copy)]
pub struct View<'a> {
    pub config: &'a LiftConfig,
    pub input: &'a ModeInputState,
    pub mode: LiftMode,
    pub results: &'a [LiftResult],
    pub selected: usize,
    pub scroll_offset: usize,
    pub draft: &'a ClusterDraft,
    pub status: Option<&'a str>,
    pub cursor_visible: bool,
}
pub fn panel_height(config: &LiftConfig) -> i32 {
    config.ui.search_height.max(1)
}
fn dropdown_visible(view: View<'_>) -> bool {
    results_visible(view) || view.status.is_some()
}
fn results_visible(view: View<'_>) -> bool {
    !view.input.query.trim().is_empty()
        || view.input.mode != LiftMode::General
        || (view.mode == LiftMode::Clusters && view.draft.count() > 0)
}
fn dropdown_base_height(view: View<'_>) -> i32 {
    let ui = &view.config.ui;
    let mut height =
        ui.search_height + ui.dropdown_gap + ui.dropdown_padding * 2 + ui.row_gap.max(8);
    if view.mode == LiftMode::Clusters && view.draft.count() > 0 {
        height += ui.draft_height + ui.row_gap;
    }
    if ui.footer_height > 0 {
        height += ui.row_gap + ui.footer_height;
    }
    if view.status.is_some() {
        height += status_height(view.config) + ui.row_gap;
    }
    height
}

fn status_height(config: &LiftConfig) -> i32 {
    config.ui.hint_font_size.clamp(1, u16::MAX as u32) as i32 + 6
}

/// Limit the viewport to complete rows instead of letting Taffy shrink their text.
pub fn visible_results(view: View<'_>, height: u32) -> usize {
    if !results_visible(view) {
        return 0;
    }
    let ui = &view.config.ui;
    let mut used = dropdown_base_height(view);
    let mut section = "";
    let mut count = 0;
    for result in view
        .results
        .iter()
        .skip(view.scroll_offset)
        .take(view.config.visible_results)
    {
        let mut extra = ui.row_height;
        if count > 0 {
            extra += ui.row_gap;
        }
        if view.config.show_section_labels && section != result.section {
            extra += ui.section_height;
        }
        if i64::from(used + extra) > i64::from(height) {
            break;
        }
        used += extra;
        section = &result.section;
        count += 1;
    }
    count
}

pub fn surface_height(view: View<'_>) -> i32 {
    let ui = &view.config.ui;
    if !dropdown_visible(view) {
        return panel_height(view.config);
    }
    let mut height = dropdown_base_height(view);
    if !results_visible(view) {
        return height.clamp(panel_height(view.config), 980);
    }
    // A constrained viewport may scroll farther than the requested viewport.
    // Keep requesting the full content size when navigating its final rows.
    let scroll_offset = view.scroll_offset.min(
        view.results
            .len()
            .saturating_sub(view.config.visible_results),
    );
    let rows: Vec<_> = view
        .results
        .iter()
        .skip(scroll_offset)
        .take(view.config.visible_results)
        .collect();
    let mut last = "";
    if view.config.show_section_labels {
        for result in &rows {
            if result.section != last {
                height += ui.section_height;
                last = &result.section;
            }
        }
    }
    let count = rows.len().max(1) as i32;
    height += count * ui.row_height + (count - 1) * ui.row_gap;
    height.clamp(panel_height(view.config), 980)
}
pub fn panel_rect(_: &LiftConfig, width: u32, height: u32) -> Rect {
    Rect {
        x: 0,
        y: 0,
        w: width.max(1) as i32,
        h: height.max(1) as i32,
    }
}
pub fn contains(rect: Rect, x: f64, y: f64) -> bool {
    x >= rect.x as f64
        && x < (rect.x + rect.w) as f64
        && y >= rect.y as f64
        && y < (rect.y + rect.h) as f64
}
/// Use the same Taffy rectangles as painting, never a second row-position formula.
pub fn result_index_at(
    renderer: &FontRenderer,
    view: View<'_>,
    _: u32,
    _: u32,
    x: f64,
    y: f64,
) -> Option<usize> {
    renderer.rows.iter().find_map(|(i, rect, title)| {
        (rect.contains(Point::new(x as f32, y as f32))
            && view.results.get(*i).is_some_and(|r| r.title == *title))
        .then_some(*i)
    })
}
fn transparent() -> Color {
    Color::rgba(0.0, 0.0, 0.0, 0.0)
}
fn color(raw: &str, fallback: Color) -> Color {
    let value = raw.trim().trim_start_matches('#');
    if value.len() != 6 && value.len() != 8 {
        return fallback;
    }
    let Ok(rgb) = u32::from_str_radix(value, 16) else {
        return fallback;
    };
    let (rgb, a) = if value.len() == 8 {
        (rgb >> 8, (rgb & 255) as f32 / 255.0)
    } else {
        (rgb, 1.0)
    };
    Color::rgba(
        ((rgb >> 16) & 255) as f32 / 255.0,
        ((rgb >> 8) & 255) as f32 / 255.0,
        (rgb & 255) as f32 / 255.0,
        a,
    )
}
fn style(fill: Color, border: Color, width: i32, radius: i32) -> CardStyle {
    CardStyle {
        fill,
        border,
        border_width: width.max(0) as f32,
        radius: radius.max(0) as f32,
    }
}
/// Retained toolkit state. The historical name keeps the native host adapter small.
pub struct FontRenderer {
    text: TextSystem,
    view: UiView,
    theme: Theme,
    rows: Vec<(usize, UiRect, String)>,
    pub snapshot: Option<PreparedView>,
    tints: HashMap<(u64, [u8; 4]), Arc<ImageData>>,
}
impl FontRenderer {
    pub fn new(family: &str) -> Result<Self, String> {
        let theme = Theme {
            font: Font {
                family: family.into(),
                size: 22,
            },
            ..Theme::default()
        };
        let mut out = Self {
            text: TextSystem::new(),
            view: UiView::new("lift")
                .content(TextInput::new("query", "").width(700.0).height(60.0)),
            theme,
            rows: Vec::new(),
            snapshot: None,
            tints: HashMap::new(),
        };
        out.view
            .prepare(
                UiRect::new(0.0, 0.0, 760.0, 60.0),
                &out.theme,
                &mut out.text,
            )
            .map_err(|e| e.to_string())?;
        out.view.handle_event(InputEvent::WindowFocus(true));
        out.view.handle_event(InputEvent::KeyDown {
            key: Key::Tab,
            modifiers: Modifiers::default(),
        });
        Ok(out)
    }
    pub fn edit(&mut self, current: &str, event: InputEvent) -> String {
        if self.view.text_value("query") != Some(current) {
            self.view.set_text_value("query", current);
        }
        if self.view.focused() != Some("query") {
            for _ in 0..self.rows.len() + 2 {
                self.view.handle_event(InputEvent::KeyDown {
                    key: Key::Tab,
                    modifiers: Modifiers::default(),
                });
                if self.view.focused() == Some("query") {
                    break;
                }
            }
        }
        self.view.handle_event(event);
        self.view.text_value("query").unwrap_or(current).to_owned()
    }
    pub fn pointer(&mut self, event: InputEvent) {
        self.view.handle_event(event);
    }
    pub fn accessibility_action(
        &mut self,
        action: halley_ui::accessibility::AccessibleAction,
    ) -> Vec<halley_ui::input::UiEvent> {
        self.view.handle_accessibility(action)
    }
    fn image(
        &mut self,
        key: String,
        data: Arc<ImageData>,
        tint: Option<Color>,
        size: f32,
    ) -> Image {
        let data = if let Some(tint) = tint {
            let rgba = [
                tint.bytes()[0],
                tint.bytes()[1],
                tint.bytes()[2],
                (tint.a * 255.0).round() as u8,
            ];
            let cache_key = (data.id(), rgba);
            if self.tints.len() >= 128 {
                self.tints.clear();
            }
            self.tints
                .entry(cache_key)
                .or_insert_with(|| {
                    let mut pixels = data.pixels().to_vec();
                    for p in pixels.chunks_exact_mut(4) {
                        let alpha = u16::from(p[3]) * u16::from(rgba[3]) / 255;
                        for i in 0..3 {
                            p[i] = (u16::from(rgba[i]) * alpha / 255) as u8;
                        }
                        p[3] = alpha as u8;
                    }
                    Arc::new(
                        ImageData::from_rgba(data.width(), data.height(), pixels.into())
                            .expect("same image geometry"),
                    )
                })
                .clone()
        } else {
            data
        };
        Image::new(key, data).width(size).height(size)
    }
}
fn label(
    key: impl Into<String>,
    text: impl Into<String>,
    config: &LiftConfig,
    size: u32,
    tint: Color,
) -> Label {
    Label::new(key, text)
        .font(Font {
            family: config.ui.font.clone(),
            size: size.clamp(1, u16::MAX as u32) as u16,
        })
        .color(tint)
}

pub fn draw_palette(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    renderer: &mut FontRenderer,
    icons: &mut IconCache,
    view: View<'_>,
) -> Result<(), String> {
    let config = view.config;
    let ui = &config.ui;
    let visible_results = visible_results(view, height);
    let text_color = color(&config.colors.text, Color::rgb8(242, 245, 255));
    let hint = color(&config.colors.hint, Color::rgb8(133, 143, 168));
    let accent = color(&config.colors.accent, Color::rgb8(143, 181, 255));
    let subtext = color(&config.colors.subtext, Color::rgb8(158, 167, 191));
    let search_fill = color(&config.colors.search, Color::rgba(0.035, 0.043, 0.07, 0.85));
    let dropdown_fill = color(&config.colors.dropdown, Color::rgba(0.08, 0.09, 0.13, 0.94));
    let bw = if config.border.enabled {
        config.border.width.max(0)
    } else {
        0
    };
    let outline = config.border.style != "inset";
    let border = color(&config.colors.panel_border, Color::rgb8(43, 50, 72));
    renderer.theme = Theme {
        font: Font {
            family: ui.font.clone(),
            size: ui.search_font_size.clamp(1, u16::MAX as u32) as u16,
        },
        text: text_color,
        subtext: hint,
        card: style(transparent(), transparent(), 0, 0),
        button: style(transparent(), transparent(), 0, config.rounding.row),
        button_hover: transparent(),
        button_pressed: transparent(),
    };
    let mut search = Row::new("search-content")
        .gap(12.0)
        .align(Align::Center)
        .height(ui.search_height as f32)
        .grow(1.0);
    let glyph = if config.search_icon.enabled {
        icons
            .search_glyph(config.search_icon.size.max(1) as u32, view.mode)
            .cloned()
    } else {
        None
    };
    let image = glyph.map(|data| {
        renderer.image(
            "search-glyph".into(),
            data,
            Some(color(&config.colors.search_icon, hint)),
            config.search_icon.size.clamp(1, ui.search_height) as f32,
        )
    });
    let right = config.search_icon.side.eq_ignore_ascii_case("right");
    if !right && let Some(image) = image.clone() {
        search = search.child(image);
    }
    search = search.child(
        TextInput::new("query", &view.input.query)
            .placeholder(&config.placeholder)
            .accessible_label("Search apps, nodes, clusters, and actions")
            .padding(0.0)
            .height((ui.search_height - 4).max(1) as f32)
            .grow(1.0),
    );
    if right && let Some(image) = image {
        search = search.child(image);
    }
    let search_card = Card::new("search-chrome")
        .height(ui.search_height as f32)
        .width(width as f32)
        .padding_xy(ui.padding as f32, 0.0)
        .style(style(
            search_fill,
            border,
            if outline { bw } else { 0 },
            config.rounding.search,
        ))
        .child(search);
    let mut root = Column::new("panel")
        .width(width as f32)
        .gap(ui.dropdown_gap as f32)
        .child(search_card);
    let show_dropdown = dropdown_visible(view)
        && (visible_results > 0
            || view.results.is_empty()
            || view.draft.count() > 0
            || view.status.is_some());
    if show_dropdown {
        let mut list = Column::new("results").gap(0.0).grow(1.0);
        if let Some(status) = view.status {
            list = list
                .child(
                    label(
                        "status",
                        status,
                        config,
                        ui.hint_font_size,
                        color(&config.colors.danger, Color::rgb8(235, 154, 143)),
                    )
                    .overflow(TextOverflow::EllipsisEnd)
                    .height(status_height(config) as f32),
                )
                .child(Row::new("status-gap").height(ui.row_gap as f32));
        }
        if view.mode == LiftMode::Clusters && view.draft.count() > 0 {
            let name = if view.input.query.trim().is_empty() {
                "untitled"
            } else {
                view.input.query.trim()
            };
            list = list.child(
                Card::new("draft")
                    .height(ui.draft_height as f32)
                    .padding_xy(14.0, 0.0)
                    .style(style(search_fill, transparent(), 0, config.rounding.draft))
                    .child(label(
                        "draft-label",
                        format!("Cluster Draft: {name} · {} selected", view.draft.count()),
                        config,
                        ui.subtitle_font_size,
                        text_color,
                    )),
            );
        }
        if view.mode == LiftMode::Clusters && view.draft.count() > 0 {
            list = list.child(Row::new("draft-gap").height(ui.row_gap as f32));
        }
        let mut section = "";
        for (visible, result) in view
            .results
            .iter()
            .skip(view.scroll_offset)
            .take(visible_results)
            .enumerate()
        {
            let index = view.scroll_offset + visible;
            if config.show_section_labels && section != result.section {
                list = list.child(
                    label(
                        format!("section-{index}"),
                        &result.section,
                        config,
                        ui.hint_font_size,
                        hint,
                    )
                    .height(ui.section_height.max(0) as f32),
                );
                section = &result.section;
            }
            let mut row = Row::new(format!("row-content-{index}"))
                .gap(16.0)
                .align(Align::Center)
                .grow(1.0)
                .height(ui.row_height as f32);
            let mut marker = Row::new(format!("marker-slot-{index}"))
                .width(18.0)
                .height(18.0);
            if view.mode == LiftMode::Clusters
                && view.draft.contains_result(result)
                && let Some(data) = icons.selection_glyph(18).cloned()
            {
                marker = marker.child(renderer.image(
                    format!("marker-{index}"),
                    data,
                    Some(accent),
                    18.0,
                ));
            }
            row = row.child(marker);
            if config.icons {
                let mut slot = Row::new(format!("icon-slot-{index}"))
                    .width(config.icon_size as f32)
                    .height(config.icon_size as f32);
                if let Some((data, tint)) = icons.result_icon(result, config) {
                    slot = slot.child(renderer.image(
                        format!("icon-{index}"),
                        data,
                        tint.then(|| color(&config.colors.icon, accent)),
                        config.icon_size as f32,
                    ));
                }
                row = row.child(slot);
            }
            let mut titles = Column::new(format!("titles-{index}"))
                .gap(7.0)
                .grow(1.0)
                .child(label(
                    format!("title-{index}"),
                    &result.title,
                    config,
                    ui.title_font_size,
                    text_color,
                ));
            if let Some(subtitle) = &result.subtitle {
                titles = titles.child(label(
                    format!("subtitle-{index}"),
                    subtitle,
                    config,
                    ui.subtitle_font_size,
                    subtext,
                ));
            }
            row = row.child(titles);
            let shortcut = if config.alt_number_jump && visible < 10 {
                Some(format!("Alt+{}", (visible + 1) % 10))
            } else {
                result.shortcut_hint.clone()
            };
            if let Some(shortcut) = shortcut {
                row = row.child(label(
                    format!("hint-{index}"),
                    shortcut,
                    config,
                    ui.hint_font_size,
                    color(&config.colors.alt_hint, hint),
                ));
            }
            let fill = if index == view.selected {
                color(
                    &config.colors.row_selected,
                    Color::rgba(0.18, 0.27, 0.46, 0.92),
                )
            } else {
                transparent()
            };
            list = list.child(
                Card::new(format!("row-{index}"))
                    .height(ui.row_height as f32)
                    .style(style(fill, transparent(), 0, config.rounding.row))
                    .child(
                        Button::new(format!("open-{index}"), "")
                            .accessible_label(&result.title)
                            .action(ActionId::new(format!("open-{index}")))
                            .padding_xy(5.0, 0.0)
                            .height(ui.row_height as f32)
                            .grow(1.0)
                            .child(row),
                    ),
            );
            if visible + 1
                < view
                    .results
                    .len()
                    .saturating_sub(view.scroll_offset)
                    .min(visible_results)
            {
                list = list.child(Row::new(format!("row-gap-{index}")).height(ui.row_gap as f32));
            }
        }
        list = list.child(Row::new("list-bottom").height(ui.row_gap.max(8) as f32));
        if results_visible(view) && view.results.is_empty() {
            list = list.child(
                label(
                    "no-results",
                    "No results",
                    config,
                    ui.title_font_size,
                    subtext,
                )
                .height(ui.row_height as f32),
            );
        }
        if ui.footer_height > 0 {
            list = list.child(Row::new("footer-gap").height(ui.row_gap as f32));
            list = list.child(
                label(
                    "footer",
                    format!(
                        "Enter Open    Space Select    Tab Actions    Ctrl+Enter {}    Esc Close",
                        if view.draft.count() == 0 {
                            "Create"
                        } else {
                            "Finalize draft"
                        }
                    ),
                    config,
                    ui.hint_font_size,
                    hint,
                )
                .height(ui.footer_height as f32),
            );
        }
        root = root.child(
            Card::new("dropdown-chrome")
                .width(width as f32)
                .padding_xy(ui.padding as f32, ui.dropdown_padding as f32)
                .style(style(
                    dropdown_fill,
                    if outline {
                        border
                    } else {
                        color(&config.colors.dropdown_border, border)
                    },
                    bw,
                    if outline {
                        config.rounding.panel
                    } else {
                        config.rounding.dropdown
                    },
                ))
                .child(list),
        );
    }
    renderer.view.set_content(root);
    if renderer.view.text_value("query") != Some(view.input.query.as_str()) {
        renderer.view.set_text_value("query", &view.input.query);
    }
    let prepared = renderer
        .view
        .prepare(
            UiRect::new(0.0, 0.0, width as f32, height as f32),
            &renderer.theme,
            &mut renderer.text,
        )
        .map_err(|e| e.to_string())?;
    renderer.snapshot = Some(prepared.clone());
    renderer.rows.clear();
    for (index, result) in view.results.iter().enumerate() {
        if let Some(rect) = prepared.rects.get(&format!("row-{index}")) {
            renderer.rows.push((
                index,
                rect.intersection(prepared.bounds),
                result.title.clone(),
            ));
        }
    }
    // Presentation overrides retain Lift's connected chrome and configurable caret.
    // Rasterization and clipping still run exclusively through the toolkit surface.
    let mut prepared: PreparedView = prepared.clone();
    prepared.items.retain_mut(|item| {
        if let PaintItem::Card {
            key,
            rect,
            clip,
            style,
        } = item
        {
            if key.ends_with("/query/caret") {
                if !view.cursor_visible || !config.cursor.enabled {
                    return false;
                }
                rect.size.width = config.cursor.width.max(1) as f32;
                style.fill = accent;
            }
            if show_dropdown && ui.dropdown_gap == 0 {
                if key.ends_with("/search-chrome") {
                    rect.size.height += style.radius + style.border_width;
                } else if key.ends_with("/dropdown-chrome") {
                    let extra = style.radius + style.border_width;
                    rect.origin.y -= extra;
                    rect.size.height += extra;
                }
                *clip = clip.intersection(UiRect::new(0.0, 0.0, width as f32, height as f32));
            }
        }
        true
    });
    let mut surface = Surface::new(canvas, width, height, width as usize * 4, PixelFormat::Bgra)
        .map_err(|e| e.to_string())?;
    surface.clear(transparent());
    surface.draw(&prepared, &mut renderer.text, 1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LiftAction, LiftResultKind};

    #[test]
    fn constrained_viewport_keeps_complete_rows_and_the_scrolled_selection_visible() {
        let config = LiftConfig {
            icons: false,
            ..Default::default()
        };
        let input = ModeInputState {
            query: "editor".into(),
            ..Default::default()
        };
        let results: Vec<_> = (0..12)
            .map(|index| LiftResult {
                section: "Apps".into(),
                title: format!("Editor {index}"),
                subtitle: Some("Text editor".into()),
                icon_name: None,
                kind: LiftResultKind::App,
                score: 1.0,
                is_field_pinned: false,
                shortcut_hint: None,
                action: LiftAction::ReloadConfig,
            })
            .collect();
        let draft = ClusterDraft::default();
        let mut renderer = FontRenderer::new("sans-serif").unwrap();
        let mut icons = IconCache::new(&config);
        for (height, offset, selected, count) in [
            (294, 0, 0, 2),
            (294, 10, 11, 2),
            (664, 0, 0, 8),
            (60, 0, 0, 0),
        ] {
            let view = View {
                config: &config,
                input: &input,
                mode: LiftMode::General,
                results: &results,
                selected,
                scroll_offset: offset,
                draft: &draft,
                status: None,
                cursor_visible: true,
            };
            // Scrolling a short viewport to the end must not request a smaller
            // surface, shrink the viewport again, and hide the selected row.
            assert_eq!(surface_height(view), 664);
            assert_eq!(visible_results(view, height), count);
            let width = 640;
            let mut pixels = vec![0; (width * height * 4) as usize];
            draw_palette(&mut pixels, width, height, &mut renderer, &mut icons, view).unwrap();
            assert_eq!(renderer.rows.len(), count);
            for (index, rect, _) in &renderer.rows {
                assert!(
                    rect.size.height >= config.ui.row_height as f32 - 0.01,
                    "{rect:?}"
                );
                assert!(rect.origin.y >= config.ui.search_height as f32);
                assert!(rect.origin.y + rect.size.height <= height as f32);
                assert_eq!(
                    result_index_at(
                        &renderer,
                        view,
                        width,
                        height,
                        (rect.origin.x + 5.0) as f64,
                        (rect.origin.y + 5.0) as f64
                    ),
                    Some(*index)
                );
            }
            if count > 0 {
                assert!(renderer.rows.iter().any(|(index, _, _)| *index == selected));
            }
        }
    }

    #[test]
    fn constrained_row_capacity_reserves_section_headers_and_footer() {
        let mut config = LiftConfig::default();
        let input = ModeInputState {
            query: "editor".into(),
            ..Default::default()
        };
        let mut results = vec![
            LiftResult {
                section: "Apps".into(),
                title: "Editor".into(),
                subtitle: None,
                icon_name: None,
                kind: LiftResultKind::App,
                score: 1.0,
                is_field_pinned: false,
                shortcut_hint: None,
                action: LiftAction::ReloadConfig,
            };
            3
        ];
        let draft = ClusterDraft::default();
        let capacity = |config: &LiftConfig, results: &[LiftResult]| {
            visible_results(
                View {
                    config,
                    input: &input,
                    mode: LiftMode::General,
                    results,
                    selected: 0,
                    scroll_offset: 0,
                    draft: &draft,
                    status: None,
                    cursor_visible: true,
                },
                244,
            )
        };
        assert_eq!(capacity(&config, &results), 2);
        results[1].section = "Nodes".into();
        assert_eq!(capacity(&config, &results), 1);
        results[1].section = "Apps".into();
        config.ui.footer_height = 28;
        assert_eq!(capacity(&config, &results), 1);
    }

    #[test]
    fn blur_region_matches_rendered_corners_connected_chrome_and_dropdown_gap() {
        use crate::blur::alpha_rects;

        let mut renderer = FontRenderer::new("sans-serif").unwrap();
        // Reuse the renderer across growth/shrink transitions, as the live launcher does.
        for (query, gap, radius) in [
            ("", 0, 18),
            ("editor", 0, 18),
            ("editor", 18, 18),
            ("editor", 18, 0),
            ("editor", 18, 1000),
            ("", 0, 18),
        ] {
            let mut config = LiftConfig::default();
            config.ui.dropdown_gap = gap;
            config.rounding.search = radius;
            config.rounding.panel = radius;
            config.rounding.dropdown = radius;
            let mut icons = IconCache::new(&config);
            let input = ModeInputState {
                mode: LiftMode::General,
                query: query.into(),
            };
            let draft = ClusterDraft::default();
            let view = View {
                config: &config,
                input: &input,
                mode: LiftMode::General,
                results: &[],
                selected: 0,
                scroll_offset: 0,
                draft: &draft,
                status: None,
                cursor_visible: true,
            };
            let width = config.width;
            let height = surface_height(view) as u32;
            let mut pixels = vec![0; (width * height * 4) as usize];
            draw_palette(&mut pixels, width, height, &mut renderer, &mut icons, view).unwrap();
            let rects = alpha_rects(&pixels, width, height);
            let mut covered = vec![false; (width * height) as usize];
            for rect in &rects {
                assert!(rect.x >= 0 && rect.y >= 0);
                assert!(rect.x + rect.width <= width as i32);
                assert!(rect.y + rect.height <= height as i32);
                for y in rect.y..rect.y + rect.height {
                    for x in rect.x..rect.x + rect.width {
                        let index = (y as u32 * width + x as u32) as usize;
                        assert!(!covered[index], "overlapping blur rectangles");
                        covered[index] = true;
                    }
                }
            }
            for (index, pixel) in pixels.chunks_exact(4).enumerate() {
                assert_eq!(covered[index], pixel[3] != 0, "pixel {index}");
            }
            assert!(covered[(config.ui.search_height as u32 / 2 * width + width / 2) as usize]);
            if radius > 0 {
                assert!(!covered[0]);
                assert!(!covered[width as usize - 1]);
                assert!(!covered[((height - 1) * width) as usize]);
                assert!(!covered[(height * width - 1) as usize]);
            }
            if !query.is_empty() && gap > 0 {
                let y = config.ui.search_height as u32 + gap as u32 / 2;
                assert!(
                    covered[(y * width) as usize..((y + 1) * width) as usize]
                        .iter()
                        .all(|&pixel| !pixel)
                );
            }
        }
    }

    #[test]
    fn native_bgra_alpha_and_taffy_hits_follow_the_visible_rows() {
        let config = LiftConfig {
            show_section_labels: false,
            ..Default::default()
        };
        let input = ModeInputState {
            query: "editor".into(),
            ..Default::default()
        };
        let results = vec![LiftResult {
            section: "Apps".into(),
            title: "Editor".into(),
            subtitle: Some("Text editor".into()),
            icon_name: None,
            kind: LiftResultKind::App,
            score: 1.0,
            is_field_pinned: false,
            shortcut_hint: None,
            action: LiftAction::ReloadConfig,
        }];
        let draft = ClusterDraft::default();
        let view = View {
            config: &config,
            input: &input,
            mode: LiftMode::General,
            results: &results,
            selected: 0,
            scroll_offset: 0,
            draft: &draft,
            status: None,
            cursor_visible: true,
        };
        let mut renderer = FontRenderer::new("sans-serif").unwrap();
        let mut icons = IconCache::new(&config);
        let height = surface_height(view) as u32;
        let mut pixels = vec![0; (config.width * height * 4) as usize];
        draw_palette(
            &mut pixels,
            config.width,
            height,
            &mut renderer,
            &mut icons,
            view,
        )
        .unwrap();
        if let Some(dir) = std::env::var_os("HALLEY_LIFT_PREVIEW_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("lift.bgra"), &pixels).unwrap();
            std::fs::write(dir.join("size.txt"), format!("{} {}", config.width, height)).unwrap();
        }
        let (_, rect, _) = renderer.rows[0].clone();
        assert_eq!(
            result_index_at(
                &renderer,
                view,
                config.width,
                height,
                (rect.origin.x + 5.0) as f64,
                (rect.origin.y + 5.0) as f64
            ),
            Some(0)
        );
        assert_eq!(
            result_index_at(&renderer, view, config.width, height, 5.0, 5.0),
            None
        );
        assert!(
            pixels
                .chunks_exact(4)
                .all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3])
        );
        assert!(pixels.chunks_exact(4).any(|p| p[3] != 0));
    }
    #[test]
    fn search_editing_deletes_whole_graphemes_and_preserves_selection() {
        let mut renderer = FontRenderer::new("sans-serif").unwrap();
        let input = "work👩‍💻";
        let edited = renderer.edit(
            input,
            InputEvent::KeyDown {
                key: Key::Backspace,
                modifiers: Modifiers::default(),
            },
        );
        assert_eq!(edited, "work");
        let edited = renderer.edit(&edited, InputEvent::Text("space".into()));
        assert_eq!(edited, "workspace");
        renderer.edit(
            &edited,
            InputEvent::KeyDown {
                key: Key::A,
                modifiers: Modifiers {
                    control: true,
                    ..Modifiers::default()
                },
            },
        );
        assert_eq!(
            renderer.edit(&edited, InputEvent::Text("node".into())),
            "node"
        );
    }
}
