use crate::calendar::{local_midnight, CalendarContent, CalendarEvent};
use crate::ui::window::ERROR_COLOR;
use chrono::{DateTime, Local, NaiveDate};
use eframe::egui;
use std::time::Duration;

pub const SIDEBAR_WIDTH: f32 = 300.0;
const FIRST_HOUR: f32 = 8.0;
const LAST_HOUR: f32 = 18.0;
const GUTTER: f32 = 36.0;
const GRID_TOP: f32 = 8.0;
const GRID_BOTTOM: f32 = 4.0;
const FINISHED_FILL_FADE: f32 = 0.45;
const FINISHED_TEXT_FADE: f32 = 0.15;
const ROW_HEIGHT: f32 = 16.0;
const ALL_DAY_HEIGHT: f32 = 22.0;
const HOUR_LINE_COLOR: egui::Color32 = egui::Color32::from_rgb(0x2c, 0x2c, 0x2c);
const SETUP_YAML: &str = "calendar:\n  clientId: <OAuth client ID>\n  clientSecret: <secret>";

pub struct Freshness {
    pub age: Option<Duration>,
    pub refreshing: bool,
}

fn freshness_label(content: &CalendarContent, freshness: &Freshness) -> Option<String> {
    if *content == CalendarContent::Loading {
        return None;
    }
    if freshness.refreshing {
        return Some("refreshing".to_string());
    }
    let minutes = freshness.age?.as_secs() / 60;
    Some(match minutes {
        0 => "updated just now".to_string(),
        n => format!("updated {n} min ago"),
    })
}

pub fn render_sidebar(
    ui: &mut egui::Ui,
    content: &CalendarContent,
    date: NaiveDate,
    freshness: &Freshness,
) {
    let now = Local::now();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(date.format("%a %-d %b").to_string())
                .size(16.0)
                .color(egui::Color32::WHITE),
        );
        if let Some(label) = freshness_label(content, freshness) {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(label)
                        .size(12.0)
                        .color(egui::Color32::GRAY),
                );
            });
        }
    });
    ui.separator();
    match content {
        CalendarContent::Loading => gray_label(ui, "Loading calendar..."),
        CalendarContent::NotConfigured => render_setup(ui),
        CalendarContent::Days(days) => {
            let viewed_now = (date == now.date_naive()).then_some(now);
            render_day(ui, days.events_on(date), date, viewed_now);
        }
    }
}

fn finished(event: &CalendarEvent, now: Option<DateTime<Local>>) -> bool {
    now.is_some_and(|now| event.end <= now)
}

fn render_day(
    ui: &mut egui::Ui,
    events: &[CalendarEvent],
    date: NaiveDate,
    now: Option<DateTime<Local>>,
) {
    let panel_fill = ui.visuals().panel_fill;
    for event in events.iter().filter(|event| event.all_day) {
        let (block, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), ALL_DAY_HEIGHT),
            egui::Sense::hover(),
        );
        paint_all_day(ui.painter(), block, event, finished(event, now), panel_fill);
        response.on_hover_text(format!("{}\nAll day", event.title));
    }
    let Some(day_start) = local_midnight(date) else {
        return;
    };
    render_grid(ui, events, day_start, now);
}

fn paint_all_day(
    painter: &egui::Painter,
    block: egui::Rect,
    event: &CalendarEvent,
    finished: bool,
    panel_fill: egui::Color32,
) {
    let block = block.shrink2(egui::vec2(0.0, 1.0));
    let (fill, text_color) = event_colours(event, finished, panel_fill);
    painter.rect_filled(block, 2.0, fill);
    painter.with_clip_rect(block).text(
        egui::pos2(block.left() + 4.0, block.center().y),
        egui::Align2::LEFT_CENTER,
        &event.title,
        egui::FontId::proportional(13.0),
        text_color,
    );
}

fn gray_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(16.0)
            .color(egui::Color32::GRAY),
    );
}

fn render_setup(ui: &mut egui::Ui) {
    gray_label(
        ui,
        "No calendars configured. Add a Google Cloud Desktop OAuth client to ~/.project-switch.yml:",
    );
    ui.code(egui::RichText::new(SETUP_YAML).size(13.0));
    gray_label(ui, "then run project-switch calendar auth work");
}

fn rgb([r, g, b]: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

fn hour_range(spans: &[(f32, f32)]) -> (f32, f32) {
    spans
        .iter()
        .fold((FIRST_HOUR, LAST_HOUR), |(first, last), &(start, end)| {
            (first.min(start.floor()), last.max(end.ceil()))
        })
}

fn event_tooltip(event: &CalendarEvent) -> String {
    format!(
        "{}\n{} – {}",
        event.title,
        event.start.format("%H:%M"),
        event.end.format("%H:%M")
    )
}

fn render_grid(
    ui: &mut egui::Ui,
    events: &[CalendarEvent],
    day_start: DateTime<Local>,
    now: Option<DateTime<Local>>,
) {
    let hour_of = |t: DateTime<Local>| (t - day_start).num_seconds() as f32 / 3600.0;
    let timed: Vec<(&CalendarEvent, f32, f32)> = events
        .iter()
        .filter(|event| !event.all_day)
        .map(|event| {
            (
                event,
                hour_of(event.start).max(0.0),
                hour_of(event.end).min(24.0),
            )
        })
        .filter(|(_, start, end)| end > start)
        .collect();
    let spans: Vec<(f32, f32)> = timed.iter().map(|&(_, s, e)| (s, e)).collect();
    let (first_hour, last_hour) = hour_range(&spans);
    let per_hour = (ui.available_height() - GRID_TOP - GRID_BOTTOM) / (LAST_HOUR - FIRST_HOUR);
    let height = GRID_TOP + (last_hour - first_hour) * per_hour + GRID_BOTTOM;

    let salt = ("calendar_grid", first_hour as u32, last_hour as u32);
    let positioned = ui.id().with(salt);
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink(false);
    if ui.ctx().data(|d| d.get_temp::<bool>(positioned)).is_none() {
        ui.ctx().data_mut(|d| d.insert_temp(positioned, true));
        scroll = scroll.vertical_scroll_offset((FIRST_HOUR - first_hour) * per_hour);
    }
    scroll.show(ui, |ui| {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::hover(),
        );
        let grid = Grid {
            rect,
            top: rect.top() + GRID_TOP,
            per_hour,
            first_hour,
            last_hour,
        };
        paint_grid(ui, &grid, &timed, &spans, now.map(hour_of), now);
    });
}

struct Grid {
    rect: egui::Rect,
    top: f32,
    per_hour: f32,
    first_hour: f32,
    last_hour: f32,
}

impl Grid {
    fn y(&self, hour: f32) -> f32 {
        self.top + (hour - self.first_hour) * self.per_hour
    }
}

fn paint_grid(
    ui: &egui::Ui,
    grid: &Grid,
    timed: &[(&CalendarEvent, f32, f32)],
    spans: &[(f32, f32)],
    now_hour: Option<f32>,
    now: Option<DateTime<Local>>,
) {
    let rect = grid.rect;
    let painter = ui.painter_at(rect);
    let y = |hour: f32| grid.y(hour);
    let left = rect.left() + GUTTER;

    for hour in grid.first_hour as u32..=grid.last_hour as u32 {
        let line_y = y(hour as f32);
        painter.hline(left - 4.0..=rect.right(), line_y, (1.0, HOUR_LINE_COLOR));
        painter.text(
            egui::pos2(rect.left(), line_y),
            egui::Align2::LEFT_CENTER,
            format!("{hour:02}"),
            egui::FontId::monospace(12.0),
            egui::Color32::GRAY,
        );
    }

    let panel_fill = ui.visuals().panel_fill;
    for (index, ((event, start, end), (column, columns))) in
        timed.iter().zip(side_by_side_columns(spans)).enumerate()
    {
        let width = (rect.right() - left) / columns as f32;
        let x = left + column as f32 * width;
        let block = egui::Rect::from_min_max(
            egui::pos2(x, y(*start) + 1.0),
            egui::pos2(x + width - 2.0, y(*end) - 1.0),
        );
        paint_event(&painter, block, event, finished(event, now), panel_fill);
        ui.interact(
            block,
            ui.id().with(("calendar_event", index)),
            egui::Sense::hover(),
        )
        .on_hover_text(event_tooltip(event));
    }

    if let Some(now_hour) =
        now_hour.filter(|hour| (grid.first_hour..=grid.last_hour).contains(hour))
    {
        let now_y = y(now_hour);
        painter.hline(left - 2.0..=rect.right(), now_y, (2.0, ERROR_COLOR));
        painter.circle_filled(egui::pos2(left - 2.0, now_y), 4.0, ERROR_COLOR);
    }
}

fn close_cluster(
    placed: &mut [(usize, usize)],
    cluster: &mut Vec<usize>,
    column_ends: &mut Vec<f32>,
) {
    for &i in cluster.iter() {
        placed[i].1 = column_ends.len();
    }
    cluster.clear();
    column_ends.clear();
}

fn side_by_side_columns(spans: &[(f32, f32)]) -> Vec<(usize, usize)> {
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by(|&a, &b| {
        spans[a]
            .0
            .total_cmp(&spans[b].0)
            .then(spans[b].1.total_cmp(&spans[a].1))
    });
    let mut placed = vec![(0, 1); spans.len()];
    let mut cluster = Vec::new();
    let mut column_ends: Vec<f32> = Vec::new();
    let mut cluster_end = f32::NEG_INFINITY;
    for i in order {
        let (start, end) = spans[i];
        if start >= cluster_end {
            close_cluster(&mut placed, &mut cluster, &mut column_ends);
        }
        let column = column_ends
            .iter()
            .position(|&e| e <= start)
            .unwrap_or_else(|| {
                column_ends.push(end);
                column_ends.len() - 1
            });
        column_ends[column] = end;
        placed[i].0 = column;
        cluster.push(i);
        cluster_end = cluster_end.max(end);
    }
    close_cluster(&mut placed, &mut cluster, &mut column_ends);
    placed
}

fn event_colours(
    event: &CalendarEvent,
    finished: bool,
    panel_fill: egui::Color32,
) -> (egui::Color32, egui::Color32) {
    let fade = |colour: [u8; 3], amount: f32| {
        let colour = rgb(colour);
        if finished {
            colour.lerp_to_gamma(panel_fill, amount)
        } else {
            colour
        }
    };
    (
        fade(event.colour.background, FINISHED_FILL_FADE),
        fade(event.colour.foreground, FINISHED_TEXT_FADE),
    )
}

fn paint_event(
    painter: &egui::Painter,
    block: egui::Rect,
    event: &CalendarEvent,
    finished: bool,
    panel_fill: egui::Color32,
) {
    let (fill, text_color) = event_colours(event, finished, panel_fill);
    painter.rect_filled(block, 2.0, fill);

    let rows = ((block.height() - 2.0) / ROW_HEIGHT).floor().max(1.0) as usize;
    let time = format!("{} ", event.start.format("%H:%M"));
    let layout = |time_color: egui::Color32| {
        let font = egui::FontId::proportional(13.0);
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &time,
            0.0,
            egui::TextFormat::simple(font.clone(), time_color),
        );
        job.append(
            &event.title,
            0.0,
            egui::TextFormat::simple(font, text_color),
        );
        job.wrap.max_width = block.width() - 8.0;
        job.wrap.max_rows = rows;
        job.wrap.break_anywhere = rows == 1;
        painter.layout_job(job)
    };
    let galley = layout(text_color);
    let title_overpaint = layout(egui::Color32::TRANSPARENT);
    let top = if rows == 1 {
        block.center().y - galley.size().y / 2.0
    } else {
        block.top() + 1.0
    };
    let origin = egui::pos2(block.left() + 4.0, top);
    let painter = painter.with_clip_rect(block.intersect(painter.clip_rect()));
    painter.galley(origin, galley, text_color);
    painter.galley(
        origin + egui::vec2(1.0 / painter.pixels_per_point(), 0.0),
        title_overpaint,
        text_color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn freshness(age_secs: Option<u64>, refreshing: bool) -> Freshness {
        Freshness {
            age: age_secs.map(Duration::from_secs),
            refreshing,
        }
    }

    #[test]
    fn header_shows_the_data_age() {
        let day = CalendarContent::Days(crate::calendar::CalendarDays::default());
        let label = |age, refreshing| freshness_label(&day, &freshness(age, refreshing));
        assert_eq!(label(Some(30), false).as_deref(), Some("updated just now"));
        assert_eq!(
            label(Some(150), false).as_deref(),
            Some("updated 2 min ago")
        );
        assert_eq!(label(Some(400), true).as_deref(), Some("refreshing"));
        assert_eq!(label(None, false), None);
        assert_eq!(
            freshness_label(&CalendarContent::Loading, &freshness(None, true)),
            None
        );
    }

    #[test]
    fn grid_stays_at_working_hours_when_events_fit() {
        assert_eq!(hour_range(&[(9.0, 10.5), (17.0, 18.0)]), (8.0, 18.0));
    }

    #[test]
    fn grid_grows_to_cover_early_and_late_events() {
        assert_eq!(hour_range(&[(6.5, 7.0), (19.25, 20.5)]), (6.0, 21.0));
    }

    #[test]
    fn separate_events_take_the_full_width() {
        assert_eq!(
            side_by_side_columns(&[(9.0, 9.5), (9.5, 10.0)]),
            [(0, 1), (0, 1)]
        );
    }

    #[test]
    fn overlapping_events_sit_side_by_side() {
        assert_eq!(
            side_by_side_columns(&[(15.5, 16.5), (16.0, 17.0), (18.0, 19.0)]),
            [(0, 2), (1, 2), (0, 1)]
        );
    }

    #[test]
    fn freed_columns_are_reused_within_a_cluster() {
        assert_eq!(
            side_by_side_columns(&[(9.0, 12.0), (9.0, 10.0), (10.0, 11.0)]),
            [(0, 2), (1, 2), (1, 2)]
        );
    }
}
