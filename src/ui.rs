use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    AnyElement, App, BorderStyle, Bounds, Context, Corners, DispatchPhase, Edges, FocusHandle,
    Focusable, FontWeight, Hsla, MouseButton, MouseExitEvent, Pixels, Point, SharedString,
    Subscription, Task, Window, actions, canvas, div, fill, prelude::*, px, quad, relative, svg,
};

use crate::clock::{
    BLOCKS_PER_DAY, Cell, CellState, HOUR_LABELS, HOURS, MINUTES_PER_HOUR, NavDirection,
    NowSnapshot, TimeFormat, adjacent_cell, block_number, cell_state_label, current_now,
    day_percentage, duration_until_next_minute, format_time, state_at,
};
use crate::platform;
use crate::settings::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, Settings};
use crate::theme::{Palette, ThemeMode};

actions!(
    block_clock,
    [
        TogglePin,
        ToggleFullscreen,
        OpenSettings,
        Escape,
        JumpToNow,
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        MoveRowStart,
        MoveRowEnd
    ]
);

pub struct BlockClockView {
    now: NowSnapshot,
    selected: Option<Cell>,
    hovered: Option<Cell>,
    grid_bounds: Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    settings_open: bool,
    settings: Settings,
    settings_path: Option<PathBuf>,
    notice: Option<String>,
    focus_handle: FocusHandle,
    save_bounds_task: Option<Task<()>>,
    _tick_task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy, Debug)]
struct GridGeometry {
    origin_x: f32,
    origin_y: f32,
    width: f32,
    height: f32,
    cell_w: f32,
    cell_h: f32,
}

impl GridGeometry {
    const COL_GAP: f32 = 1.0;
    const DIVIDER_W: f32 = 1.0;
    const ROW_GAP: f32 = 3.0;
    const QUARTER_BREAK: f32 = 4.0;
    const HORIZONTAL_FIXED: f32 = 65.0; // 62 * 1px gaps + 3 * 1px dividers
    const VERTICAL_FIXED: f32 = 81.0; // 23 * 3px gaps + 3 * 4px 6-hour breaks

    fn from_bounds(bounds: Bounds<Pixels>) -> Option<Self> {
        let origin_x: f32 = bounds.origin.x.into();
        let origin_y: f32 = bounds.origin.y.into();
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        let cell_w = ((width - Self::HORIZONTAL_FIXED) / f32::from(MINUTES_PER_HOUR)).max(1.0);
        let cell_h = ((height - Self::VERTICAL_FIXED) / f32::from(HOURS)).max(2.0);
        Some(Self {
            origin_x,
            origin_y,
            width,
            height,
            cell_w,
            cell_h,
        })
    }

    fn col_x(&self, minute: u8) -> f32 {
        let m = f32::from(minute);
        let dividers = f32::from(minute / 15);
        // Each 15-minute divider adds 1px gap + 1px divider width on top of the column gap.
        let extra = dividers * (Self::COL_GAP + Self::DIVIDER_W);
        self.origin_x + m * (self.cell_w + Self::COL_GAP) + extra
    }

    fn row_y(&self, hour: u8) -> f32 {
        let h = f32::from(hour);
        let breaks = f32::from(hour / 6);
        self.origin_y + h * (self.cell_h + Self::ROW_GAP) + breaks * Self::QUARTER_BREAK
    }

    fn cell_rect(&self, cell: Cell) -> Bounds<Pixels> {
        Bounds {
            origin: Point::new(px(self.col_x(cell.minute)), px(self.row_y(cell.hour))),
            size: gpui::size(px(self.cell_w), px(self.cell_h)),
        }
    }

    fn divider_rect(&self, quarter: u8) -> Bounds<Pixels> {
        let minute = quarter * 15;
        let x = self.col_x(minute) - Self::COL_GAP - Self::DIVIDER_W;
        Bounds {
            origin: Point::new(px(x), px(self.origin_y)),
            size: gpui::size(px(Self::DIVIDER_W), px(self.height)),
        }
    }

    fn cell_at_point(&self, pos: Point<Pixels>) -> Option<Cell> {
        let px_x: f32 = pos.x.into();
        let px_y: f32 = pos.y.into();
        if px_x < self.origin_x
            || px_x > self.origin_x + self.width
            || px_y < self.origin_y
            || px_y > self.origin_y + self.height
        {
            return None;
        }

        let mut best_minute = 0u8;
        let mut best_dx = f32::INFINITY;
        for minute in 0..MINUTES_PER_HOUR {
            let center = self.col_x(minute) + self.cell_w * 0.5;
            let dx = (px_x - center).abs();
            if dx < best_dx {
                best_dx = dx;
                best_minute = minute;
            }
        }

        let mut best_hour = 0u8;
        let mut best_dy = f32::INFINITY;
        for hour in 0..HOURS {
            let center = self.row_y(hour) + self.cell_h * 0.5;
            let dy = (px_y - center).abs();
            if dy < best_dy {
                best_dy = dy;
                best_hour = hour;
            }
        }

        Some(Cell::new(best_hour, best_minute))
    }
}

impl Focusable for BlockClockView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BlockClockView {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        settings: Settings,
        settings_path: Option<PathBuf>,
        initial_notice: Option<String>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        let appearance_sub = cx.observe_window_appearance(window, |_this, _window, cx| {
            cx.notify();
        });

        let activation_sub = cx.observe_window_activation(window, |this, _window, cx| {
            let next = current_now();
            if next.cell != this.now.cell || next.date_label != this.now.date_label {
                this.now = next;
                cx.notify();
            }
        });

        let bounds_sub = cx.observe_window_bounds(window, |this, window, cx| {
            if window.is_fullscreen() {
                return;
            }
            let bounds = window.bounds();
            let x: f32 = bounds.origin.x.into();
            let y: f32 = bounds.origin.y.into();
            let w: f32 = bounds.size.width.into();
            let h: f32 = bounds.size.height.into();
            if w < MIN_WINDOW_WIDTH || h < MIN_WINDOW_HEIGHT {
                return;
            }
            if (
                this.settings.window_x,
                this.settings.window_y,
                this.settings.window_w,
                this.settings.window_h,
            ) == (Some(x), Some(y), Some(w), Some(h))
            {
                return;
            }
            this.settings.window_x = Some(x);
            this.settings.window_y = Some(y);
            this.settings.window_w = Some(w);
            this.settings.window_h = Some(h);
            this.save_bounds_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(350))
                    .await;
                let _ = this.update(cx, |view, cx| {
                    view.persist_settings(cx);
                });
            }));
        });

        let tick_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(duration_until_next_minute())
                    .await;
                let updated = this.update(cx, |view, cx| {
                    let next = current_now();
                    if next.cell != view.now.cell || next.date_label != view.now.date_label {
                        view.now = next;
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        });

        Self {
            now: current_now(),
            selected: None,
            hovered: None,
            grid_bounds: Rc::new(std::cell::Cell::new(None)),
            settings_open: false,
            settings,
            settings_path,
            notice: initial_notice,
            focus_handle,
            save_bounds_task: None,
            _tick_task: tick_task,
            _subscriptions: vec![appearance_sub, activation_sub, bounds_sub],
        }
    }

    fn persist_settings(&mut self, cx: &mut Context<Self>) {
        let Some(path) = &self.settings_path else {
            return;
        };
        if let Err(err) = self.settings.save_to(path) {
            self.notice = Some(format!("Could not save settings: {err}"));
            cx.notify();
        }
    }

    fn select_cell(&mut self, cell: Cell, cx: &mut Context<Self>) {
        self.selected = Some(cell);
        cx.notify();
    }

    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_some() {
            self.selected = None;
            cx.notify();
        }
    }

    fn jump_to_now(&mut self, cx: &mut Context<Self>) {
        self.selected = Some(self.now.cell);
        cx.notify();
    }

    fn move_selection(&mut self, dir: NavDirection, cx: &mut Context<Self>) {
        if self.settings_open {
            return;
        }
        let anchor = self.selected.unwrap_or(self.now.cell);
        self.selected = Some(adjacent_cell(anchor, dir));
        cx.notify();
    }

    fn toggle_pin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !platform::supports_always_on_top() {
            return;
        }
        let next = !self.settings.pin;
        if platform::set_always_on_top(window, next) {
            self.settings.pin = next;
            self.persist_settings(cx);
            cx.notify();
        }
    }

    fn toggle_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.toggle_fullscreen();
        cx.notify();
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        self.settings_open = true;
        cx.notify();
    }

    fn close_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings_open {
            self.settings_open = false;
            cx.notify();
        }
    }

    fn handle_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
            self.settings_open = false;
            cx.notify();
        } else if window.is_fullscreen() {
            window.toggle_fullscreen();
            cx.notify();
        } else if self.selected.is_some() {
            self.selected = None;
            cx.notify();
        }
    }

    fn set_theme(&mut self, mode: ThemeMode, cx: &mut Context<Self>) {
        self.settings.theme = mode;
        self.persist_settings(cx);
        cx.notify();
    }

    fn set_time_format(&mut self, format: TimeFormat, cx: &mut Context<Self>) {
        self.settings.time_format = format;
        self.persist_settings(cx);
        cx.notify();
    }

    fn render_header(
        &self,
        palette: Palette,
        is_fullscreen: bool,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let time_str = format_time(self.now.cell, self.settings.time_format);

        let controls = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .id("jump-to-now-header")
                    .cursor_pointer()
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.now)
                    .hover(|s| s.bg(palette.surface))
                    .on_click(cx.listener(|this, _, _, cx| this.jump_to_now(cx)))
                    .child("JUMP TO NOW"),
            )
            .when(platform::supports_always_on_top(), |el| {
                el.child(self.render_header_icon_button(
                    "pin-btn",
                    "icons/pin.svg",
                    self.settings.pin,
                    palette,
                    cx.listener(|this, _, window, cx| this.toggle_pin(window, cx)),
                ))
            })
            .child(self.render_header_icon_button(
                "fullscreen-btn",
                if is_fullscreen {
                    "icons/minimize.svg"
                } else {
                    "icons/maximize.svg"
                },
                is_fullscreen,
                palette,
                cx.listener(|this, _, window, cx| this.toggle_fullscreen(window, cx)),
            ))
            .child(self.render_header_icon_button(
                "settings-btn",
                "icons/settings.svg",
                self.settings_open,
                palette,
                cx.listener(|this, _, _, cx| this.open_settings(cx)),
            ));

        let title_block = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(palette.text_faint)
                    .child("BLOCK CLOCK"),
            )
            .child(
                div()
                    .text_size(px(if compact { 38. } else { 48. }))
                    .line_height(px(if compact { 38. } else { 48. }))
                    .font_weight(FontWeight::BOLD)
                    .text_color(palette.text)
                    .child(time_str),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.text_muted)
                    .child(self.now.date_label.clone()),
            );

        let mut header = div()
            .flex()
            .justify_between()
            .gap(px(16.))
            .pb(px(16.))
            .border_b_1()
            .border_color(palette.grid);

        if compact {
            header = header.flex_col().items_start();
        } else {
            header = header.flex_row().items_end();
        }

        header.child(title_block).child(controls).into_any_element()
    }

    fn render_header_icon_button(
        &self,
        id: &'static str,
        icon_path: &'static str,
        active: bool,
        palette: Palette,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        let border = if active { palette.now } else { palette.grid };
        let color = if active {
            palette.now
        } else {
            palette.text_muted
        };

        div()
            .id(id)
            .cursor_pointer()
            .w(px(36.))
            .h(px(36.))
            .rounded(px(9.))
            .border_1()
            .border_color(border)
            .flex()
            .items_center()
            .justify_center()
            .text_color(color)
            .hover(|s| s.bg(palette.future).border_color(palette.grid_strong))
            .on_click(on_click)
            .child(svg().path(icon_path).size(px(16.)).text_color(color))
            .into_any_element()
    }

    fn render_notice(&self, palette: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let msg = self.notice.as_ref()?;
        Some(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .px(px(12.))
                .py(px(8.))
                .rounded(px(8.))
                .border_1()
                .border_color(palette.now)
                .bg(palette.surface)
                .text_size(px(12.))
                .text_color(palette.text)
                .child(msg.clone())
                .child(
                    div()
                        .id("dismiss-notice")
                        .cursor_pointer()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.text_muted)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.notice = None;
                            cx.notify();
                        }))
                        .child("DISMISS"),
                )
                .into_any_element(),
        )
    }

    fn render_grid(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let hover_text: Option<SharedString> = self.hovered.map(|cell| {
            format!(
                "{} · Block {} / {}",
                format_time(cell, self.settings.time_format),
                block_number(cell),
                BLOCKS_PER_DAY
            )
            .into()
        });

        let readout_bar = div()
            .h(px(16.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(palette.text_faint)
                    .child("DAY GRID · 1,440 MINUTES"),
            )
            .when_some(hover_text, |el, text| {
                el.child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.text)
                        .child(text),
                )
            });

        let ruler = div()
            .h(px(14.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().w(px(28.)).flex_shrink_0())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.text_faint)
                    .child(div().flex_1().child("00"))
                    .child(div().flex_1().child("15"))
                    .child(div().flex_1().child("30"))
                    .child(div().flex_1().child("45")),
            );

        let mut hour_labels = div()
            .w(px(28.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(3.));

        for hour in 0..HOURS {
            let mut label = div()
                .flex_1()
                .min_h(px(2.))
                .flex()
                .items_center()
                .justify_end()
                .pr(px(4.))
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.text_muted)
                .child(HOUR_LABELS[hour as usize]);
            if hour > 0 && hour.is_multiple_of(6) {
                label = label.mt(px(4.));
            }
            hour_labels = hour_labels.child(label);
        }

        let now_cell = self.now.cell;
        let selected_cell = self.selected;
        let hovered_cell = self.hovered;
        let grid_bounds_prepaint = self.grid_bounds.clone();
        let grid_bounds_move = self.grid_bounds.clone();
        let grid_bounds_click = self.grid_bounds.clone();
        let on_window_exit = cx.listener(|this, _: &MouseExitEvent, _, cx| {
            if this.hovered.take().is_some() {
                cx.notify();
            }
        });

        let grid_canvas = div()
            .id("block-grid-canvas")
            .flex_1()
            .min_w_0()
            .h_full()
            .cursor_pointer()
            .on_mouse_move(
                cx.listener(move |this, event: &gpui::MouseMoveEvent, _, cx| {
                    let next = grid_bounds_move
                        .get()
                        .and_then(GridGeometry::from_bounds)
                        .and_then(|geom| geom.cell_at_point(event.position));
                    if this.hovered != next {
                        this.hovered = next;
                        cx.notify();
                    }
                }),
            )
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered && this.hovered.take().is_some() {
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    if let Some(cell) = grid_bounds_click
                        .get()
                        .and_then(GridGeometry::from_bounds)
                        .and_then(|geom| geom.cell_at_point(event.position))
                    {
                        this.select_cell(cell, cx);
                    }
                }),
            )
            .child(
                canvas(
                    move |bounds, _, _| {
                        grid_bounds_prepaint.set(Some(bounds));
                    },
                    move |bounds, _, window, _| {
                        // A pointer that jumps outside the window may send an exit without a move.
                        window.on_mouse_event(move |event, phase, window, cx| {
                            if phase == DispatchPhase::Bubble {
                                on_window_exit(event, window, cx);
                            }
                        });
                        let Some(geom) = GridGeometry::from_bounds(bounds) else {
                            return;
                        };
                        for quarter in 1..=3 {
                            window.paint_quad(fill(geom.divider_rect(quarter), palette.grid));
                        }
                        let radius = Corners::all(px(2.));
                        for hour in 0..HOURS {
                            for minute in 0..MINUTES_PER_HOUR {
                                let cell = Cell::new(hour, minute);
                                let state = state_at(now_cell, cell);
                                let bg = match state {
                                    CellState::Future => palette.future,
                                    CellState::PastDay => palette.past_day,
                                    CellState::CurrentHourPast => palette.current_hour_past,
                                    CellState::Now => palette.now,
                                };
                                let (border_widths, border_color) = if state == CellState::Now {
                                    (Edges::all(px(2.)), palette.now_ring.into())
                                } else if selected_cell == Some(cell) {
                                    (Edges::all(px(1.)), palette.text.into())
                                } else if hovered_cell == Some(cell) {
                                    (Edges::all(px(1.)), palette.text_muted.into())
                                } else {
                                    (Edges::default(), Hsla::transparent_black())
                                };
                                window.paint_quad(quad(
                                    geom.cell_rect(cell),
                                    radius,
                                    bg,
                                    border_widths,
                                    border_color,
                                    BorderStyle::Solid,
                                ));
                            }
                        }
                    },
                )
                .size_full(),
            );

        let grid_body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_row()
            .gap(px(6.))
            .child(hour_labels)
            .child(grid_canvas);

        div()
            .flex_1()
            .min_h(px(340.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(18.))
            .rounded(px(14.))
            .border_1()
            .border_color(palette.grid)
            .bg(palette.surface)
            .child(readout_bar)
            .child(ruler)
            .child(grid_body)
            .into_any_element()
    }

    fn render_rail(&self, palette: Palette, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let block_num = block_number(self.now.cell);
        let pct = day_percentage(self.now.cell).floor() as u32;
        let progress_fraction = (pct as f32 / 100.0).clamp(0.0, 1.0);

        let day_position = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(palette.text_faint)
                    .child("DAY POSITION"),
            )
            .child(self.render_stat_row(
                "Block",
                format!("{block_num} / {BLOCKS_PER_DAY}"),
                palette,
            ))
            .child(self.render_stat_row(
                "Hour · minute",
                format!("{:02} · {:02}", self.now.cell.hour, self.now.cell.minute),
                palette,
            ))
            .child(self.render_stat_row("Day elapsed", format!("{pct}%"), palette))
            .child(
                div()
                    .h(px(4.))
                    .w_full()
                    .rounded(px(999.))
                    .overflow_hidden()
                    .bg(palette.grid)
                    .child(
                        div()
                            .h_full()
                            .w(relative(progress_fraction))
                            .rounded(px(999.))
                            .bg(palette.now),
                    ),
            );

        let selection_header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(palette.text_faint)
                    .child("MINUTE DETAIL"),
            )
            .when(self.selected.is_some(), |el| {
                el.child(
                    div()
                        .id("clear-selection")
                        .cursor_pointer()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.text_muted)
                        .hover(|s| s.text_color(palette.text))
                        .on_click(cx.listener(|this, _, _, cx| this.clear_selection(cx)))
                        .child("CLEAR"),
                )
            });

        let mut selection_section = div().flex().flex_col().gap(px(8.)).child(selection_header);
        if let Some(cell) = self.selected {
            let state = state_at(self.now.cell, cell);
            selection_section = selection_section
                .child(
                    div()
                        .text_size(px(28.))
                        .line_height(px(30.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(palette.text)
                        .child(format_time(cell, self.settings.time_format)),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.now)
                        .child(cell_state_label(state).to_uppercase()),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.text_muted)
                        .child(format!("Block {} / {}", block_number(cell), BLOCKS_PER_DAY)),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(palette.text_muted)
                        .child("Selected from the day grid"),
                );
        } else {
            selection_section = selection_section.child(
                div()
                    .text_size(px(12.))
                    .text_color(palette.text_muted)
                    .child("Select any minute to inspect its position in the day."),
            );
        }

        let tz_row = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.text_muted)
            .child(self.now.tz_label.clone())
            .when(self.now.is_dst, |el| {
                el.child(
                    div()
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(palette.grid)
                        .text_size(px(10.))
                        .child("DST"),
                )
            });

        let legend = div()
            .flex()
            .flex_col()
            .gap(px(7.))
            .py(px(12.))
            .border_t_1()
            .border_color(palette.grid)
            .child(self.render_legend_item(
                palette.past_day,
                cell_state_label(CellState::PastDay),
                palette,
            ))
            .child(self.render_legend_item(
                palette.current_hour_past,
                cell_state_label(CellState::CurrentHourPast),
                palette,
            ))
            .child(self.render_legend_item(palette.now, cell_state_label(CellState::Now), palette))
            .child(self.render_legend_item(
                palette.future,
                cell_state_label(CellState::Future),
                palette,
            ));

        let footer = div()
            .mt_auto()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(tz_row)
            .child(legend)
            .child(
                div()
                    .id("jump-to-now-rail")
                    .cursor_pointer()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.now)
                    .hover(|s| s.text_color(palette.text))
                    .on_click(cx.listener(|this, _, _, cx| this.jump_to_now(cx)))
                    .child("JUMP TO NOW"),
            );

        let mut rail = div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .bg(palette.bg)
            .p(px(24.));

        if compact {
            rail = rail.w_full().border_t_1().border_color(palette.grid);
        } else {
            rail = rail
                .w(px(296.))
                .flex_shrink_0()
                .h_full()
                .border_l_1()
                .border_color(palette.grid);
        }

        rail.child(day_position)
            .child(div().h(px(1.)).bg(palette.grid))
            .child(selection_section)
            .child(footer)
            .into_any_element()
    }

    fn render_stat_row(&self, label: &'static str, value: String, palette: Palette) -> AnyElement {
        div()
            .flex()
            .items_baseline()
            .justify_between()
            .gap(px(12.))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(palette.text_muted)
                    .child(label),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.text)
                    .child(value),
            )
            .into_any_element()
    }

    fn render_legend_item(
        &self,
        swatch: gpui::Rgba,
        label: &'static str,
        palette: Palette,
    ) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .w(px(9.))
                    .h(px(9.))
                    .rounded(px(2.))
                    .border_1()
                    .border_color(palette.grid_strong)
                    .bg(swatch),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.text_muted)
                    .child(label),
            )
            .into_any_element()
    }

    fn render_settings_modal(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.settings_open {
            return None;
        }

        let backdrop = Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 0.45,
        };

        let theme_row = div()
            .flex()
            .flex_row()
            .gap(px(8.))
            .child(self.render_segment_button(
                "theme-system",
                ThemeMode::System.label(),
                self.settings.theme == ThemeMode::System,
                palette,
                cx.listener(|this, _, _, cx| this.set_theme(ThemeMode::System, cx)),
            ))
            .child(self.render_segment_button(
                "theme-light",
                ThemeMode::Light.label(),
                self.settings.theme == ThemeMode::Light,
                palette,
                cx.listener(|this, _, _, cx| this.set_theme(ThemeMode::Light, cx)),
            ))
            .child(self.render_segment_button(
                "theme-dark",
                ThemeMode::Dark.label(),
                self.settings.theme == ThemeMode::Dark,
                palette,
                cx.listener(|this, _, _, cx| this.set_theme(ThemeMode::Dark, cx)),
            ));

        let format_row = div()
            .flex()
            .flex_row()
            .gap(px(8.))
            .child(self.render_segment_button(
                "fmt-24h",
                TimeFormat::Hour24.label(),
                self.settings.time_format == TimeFormat::Hour24,
                palette,
                cx.listener(|this, _, _, cx| this.set_time_format(TimeFormat::Hour24, cx)),
            ))
            .child(self.render_segment_button(
                "fmt-12h",
                TimeFormat::Hour12.label(),
                self.settings.time_format == TimeFormat::Hour12,
                palette,
                cx.listener(|this, _, _, cx| this.set_time_format(TimeFormat::Hour12, cx)),
            ));

        let pin_toggle = div()
            .id("modal-pin-toggle")
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.))
            .py(px(10.))
            .rounded(px(8.))
            .border_1()
            .border_color(if self.settings.pin {
                palette.now
            } else {
                palette.grid
            })
            .bg(palette.future)
            .on_click(cx.listener(|this, _, window, cx| this.toggle_pin(window, cx)))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.text)
                    .child("Always on top"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(if self.settings.pin {
                        palette.now
                    } else {
                        palette.text_muted
                    })
                    .child(if self.settings.pin { "ON" } else { "OFF" }),
            );

        let dialog = div()
            .id("settings-dialog")
            .w(px(420.))
            .p(px(24.))
            .rounded(px(14.))
            .border_1()
            .border_color(palette.grid_strong)
            .bg(palette.surface)
            .flex()
            .flex_col()
            .gap(px(16.))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(palette.text)
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .id("close-settings")
                            .cursor_pointer()
                            .w(px(32.))
                            .h(px(32.))
                            .rounded(px(8.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(palette.text_muted)
                            .hover(|s| s.bg(palette.future).text_color(palette.text))
                            .on_click(cx.listener(|this, _, _, cx| this.close_settings(cx)))
                            .child(
                                svg()
                                    .path("icons/close.svg")
                                    .size(px(16.))
                                    .text_color(palette.text_muted),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text_muted)
                            .child("Theme"),
                    )
                    .child(theme_row),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text_muted)
                            .child("Time format"),
                    )
                    .child(format_row),
            )
            .when(platform::supports_always_on_top(), |el| el.child(pin_toggle))
            .child(
                div()
                    .pt(px(12.))
                    .border_t_1()
                    .border_color(palette.grid)
                    .text_size(px(11.))
                    .text_color(palette.text_muted)
                    .child("Shortcuts: Cmd/Ctrl+P pin · Cmd/Ctrl+Shift+F fullscreen · Cmd/Ctrl+, settings · J jump to now · Esc close"),
            );

        Some(
            div()
                .id("settings-backdrop")
                .absolute()
                .inset_0()
                .bg(backdrop)
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .on_click(cx.listener(|this, _, _, cx| this.close_settings(cx)))
                .child(dialog)
                .into_any_element(),
        )
    }

    fn render_segment_button(
        &self,
        id: &'static str,
        label: &'static str,
        selected: bool,
        palette: Palette,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .flex_1()
            .cursor_pointer()
            .px(px(10.))
            .py(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(if selected { palette.now } else { palette.grid })
            .bg(palette.future)
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if selected {
                palette.now
            } else {
                palette.text_muted
            })
            .hover(|s| s.border_color(palette.grid_strong))
            .on_click(on_click)
            .child(label)
            .into_any_element()
    }
}

impl Render for BlockClockView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::resolve(self.settings.theme, window.appearance());
        let is_fullscreen = window.is_fullscreen();
        let compact = window.viewport_size().width < px(760.);

        let mut clock_panel = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(18.))
            .p(px(if compact { 16. } else { 26. }))
            .child(self.render_header(palette, is_fullscreen, compact, cx));

        if let Some(notice) = self.render_notice(palette, cx) {
            clock_panel = clock_panel.child(notice);
        }

        clock_panel = clock_panel.child(self.render_grid(palette, cx));

        let mut shell = div()
            .id("block-clock-root")
            .track_focus(&self.focus_handle)
            .key_context("BlockClock")
            .on_action(cx.listener(|this, _: &TogglePin, window, cx| this.toggle_pin(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleFullscreen, window, cx| {
                this.toggle_fullscreen(window, cx)
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.open_settings(cx)))
            .on_action(cx.listener(|this, _: &Escape, window, cx| this.handle_escape(window, cx)))
            .on_action(cx.listener(|this, _: &JumpToNow, _, cx| this.jump_to_now(cx)))
            .on_action(
                cx.listener(|this, _: &MoveLeft, _, cx| {
                    this.move_selection(NavDirection::Left, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &MoveRight, _, cx| {
                this.move_selection(NavDirection::Right, cx)
            }))
            .on_action(
                cx.listener(|this, _: &MoveUp, _, cx| this.move_selection(NavDirection::Up, cx)),
            )
            .on_action(
                cx.listener(|this, _: &MoveDown, _, cx| {
                    this.move_selection(NavDirection::Down, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &MoveRowStart, _, cx| {
                this.move_selection(NavDirection::Home, cx)
            }))
            .on_action(
                cx.listener(|this, _: &MoveRowEnd, _, cx| {
                    this.move_selection(NavDirection::End, cx)
                }),
            )
            .relative()
            .size_full()
            .bg(palette.bg)
            .text_color(palette.text)
            .flex();

        if compact {
            shell = shell.flex_col();
        } else {
            shell = shell.flex_row();
        }

        shell = shell
            .child(clock_panel)
            .child(self.render_rail(palette, compact, cx));

        if let Some(modal) = self.render_settings_modal(palette, cx) {
            shell = shell.child(modal);
        }

        shell
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    #[gpui::test]
    fn grid_hover_keeps_layout_stable_and_click_selects(cx: &mut TestAppContext) {
        let window = cx.add_window(|window, cx| {
            BlockClockView::new(window, cx, Settings::default(), None, None)
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        visual.update(|window, cx| window.draw(cx).clear());
        let bounds = window
            .update(cx, |view, _, _| {
                view.grid_bounds.get().expect("grid bounds")
            })
            .expect("read grid bounds");
        let geometry = GridGeometry::from_bounds(bounds).expect("grid geometry");
        let target = Cell::new(12, 14);
        let cell = geometry.cell_rect(target);
        let center = cell.center();
        visual.simulate_mouse_move(center, None, Modifiers::none());
        visual.update(|window, cx| window.draw(cx).clear());
        window
            .update(cx, |view, _, _| {
                assert_eq!(view.hovered, Some(target));
                assert_eq!(view.selected, None);
                assert_eq!(view.grid_bounds.get(), Some(bounds));
            })
            .expect("redraw hovered grid");

        // Cross into the quarter-hour divider, then move to the next cell.
        let gap = Point::new(cell.origin.x + cell.size.width + px(0.5), center.y);
        visual.simulate_mouse_move(gap, None, Modifiers::none());
        window
            .update(cx, |view, _, _| assert_eq!(view.hovered, Some(target)))
            .expect("inspect hover in gap");

        let next = Cell::new(12, 15);
        let next_center = geometry.cell_rect(next).center();
        visual.simulate_mouse_move(next_center, None, Modifiers::none());
        visual.simulate_click(next_center, Modifiers::none());
        visual.simulate_mouse_move(
            Point::new(bounds.origin.x - px(1.), center.y),
            None,
            Modifiers::none(),
        );
        window
            .update(cx, |view, _, _| {
                assert_eq!(view.hovered, None);
                assert_eq!(view.selected, Some(next));
            })
            .expect("inspect hover exit and selection");

        visual.simulate_mouse_move(next_center, None, Modifiers::none());
        visual.simulate_event(MouseExitEvent {
            position: Point::new(px(-1.), center.y),
            pressed_button: None,
            modifiers: Modifiers::none(),
        });
        window
            .update(cx, |view, _, _| assert_eq!(view.hovered, None))
            .expect("inspect window exit");
    }

    #[gpui::test]
    fn selection_navigation_and_escape_precedence(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("tempdir");
        let settings_path = dir.path().join("settings.json");

        let window = cx.add_window(|window, cx| {
            BlockClockView::new(
                window,
                cx,
                Settings::default(),
                Some(settings_path.clone()),
                None,
            )
        });

        window
            .update(cx, |view, window, cx| {
                assert_eq!(view.selected, None);

                view.select_cell(Cell::new(10, 15), cx);
                assert_eq!(view.selected, Some(Cell::new(10, 15)));

                view.move_selection(NavDirection::Right, cx);
                assert_eq!(view.selected, Some(Cell::new(10, 16)));

                view.move_selection(NavDirection::Home, cx);
                assert_eq!(view.selected, Some(Cell::new(10, 0)));

                view.open_settings(cx);
                assert!(view.settings_open);

                // Escape closes settings first, keeping selection intact.
                view.handle_escape(window, cx);
                assert!(!view.settings_open);
                assert_eq!(view.selected, Some(Cell::new(10, 0)));

                // Second Escape clears selection.
                view.handle_escape(window, cx);
                assert_eq!(view.selected, None);

                // JumpToNow selects current minute.
                view.jump_to_now(cx);
                assert_eq!(view.selected, Some(view.now.cell));

                // Theme and format updates persist to settings.json.
                view.set_theme(ThemeMode::Dark, cx);
                view.set_time_format(TimeFormat::Hour12, cx);
            })
            .expect("update window");

        let saved = Settings::load_from(&settings_path).expect("load saved settings");
        assert_eq!(saved.theme, ThemeMode::Dark);
        assert_eq!(saved.time_format, TimeFormat::Hour12);
    }

    #[gpui::test]
    fn unwritable_settings_path_surfaces_and_clears_notice(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("tempdir");
        // Point settings_path at an existing directory so save_to fails deterministically.
        let bad_path = dir.path().to_path_buf();

        let window = cx.add_window(|window, cx| {
            BlockClockView::new(window, cx, Settings::default(), Some(bad_path), None)
        });

        window
            .update(cx, |view, _, cx| {
                assert!(view.notice.is_none());
                view.set_theme(ThemeMode::Dark, cx);
                assert!(
                    view.notice
                        .as_deref()
                        .is_some_and(|msg| msg.starts_with("Could not save settings:"))
                );

                view.open_settings(cx);
                assert!(view.notice.is_none());
            })
            .expect("update window");
    }

    #[test]
    fn grid_geometry_hit_testing_snaps_gaps_and_rejects_outside() {
        let bounds = Bounds {
            origin: Point::new(px(100.0), px(80.0)),
            size: gpui::size(px(665.0), px(321.0)),
        };
        let geom = GridGeometry::from_bounds(bounds).expect("valid geometry");
        assert!((geom.cell_w - 10.0).abs() < 0.01);
        assert!((geom.cell_h - 10.0).abs() < 0.01);

        // Top-left and bottom-right corners map to 00:00 and 23:59.
        assert_eq!(
            geom.cell_at_point(Point::new(px(100.0), px(80.0))),
            Some(Cell::new(0, 0))
        );
        assert_eq!(
            geom.cell_at_point(Point::new(px(765.0), px(401.0))),
            Some(Cell::new(23, 59))
        );

        // 1px gap between minute 0 and minute 1 still resolves to a cell (never None).
        let c0 = geom.cell_rect(Cell::new(0, 0));
        let gap_x = f32::from(c0.origin.x) + f32::from(c0.size.width) + 0.5;
        let mid_y = f32::from(c0.origin.y) + f32::from(c0.size.height) * 0.5;
        assert!(
            geom.cell_at_point(Point::new(px(gap_x), px(mid_y)))
                .is_some()
        );

        // The standard gap and the longer six-hour break both resolve to cells.
        let r4 = geom.cell_rect(Cell::new(4, 10));
        let row_gap = Point::new(r4.center().x, r4.origin.y + r4.size.height + px(1.));
        assert_eq!(geom.cell_at_point(row_gap), Some(Cell::new(4, 10)));

        let r5 = geom.cell_rect(Cell::new(5, 10));
        let mid_x = f32::from(r5.origin.x) + f32::from(r5.size.width) * 0.5;
        let break_y = f32::from(r5.origin.y) + f32::from(r5.size.height) + 3.5;
        assert!(
            geom.cell_at_point(Point::new(px(mid_x), px(break_y)))
                .is_some()
        );

        // Outside bounds returns None.
        assert_eq!(geom.cell_at_point(Point::new(px(99.0), px(80.0))), None);
        assert_eq!(geom.cell_at_point(Point::new(px(100.0), px(402.0))), None);
    }
}
