use chrono::{DateTime, Datelike, Local, NaiveDate, Offset, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

pub const HOURS: u8 = 24;
pub const MINUTES_PER_HOUR: u8 = 60;
pub const BLOCKS_PER_DAY: u16 = (HOURS as u16) * (MINUTES_PER_HOUR as u16);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub hour: u8,
    pub minute: u8,
}

impl Cell {
    pub const fn new(hour: u8, minute: u8) -> Self {
        Self { hour, minute }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CellState {
    Future,
    PastDay,
    CurrentHourPast,
    Now,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeFormat {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "12h")]
    Hour12,
    #[serde(rename = "24h")]
    Hour24,
}

impl TimeFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (24h)",
            Self::Hour12 => "12-hour",
            Self::Hour24 => "24-hour",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NavDirection {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NowSnapshot {
    pub cell: Cell,
    pub date_label: String,
    pub tz_label: String,
    pub is_dst: bool,
}

pub fn block_index(cell: Cell) -> u16 {
    (cell.hour as u16) * (MINUTES_PER_HOUR as u16) + (cell.minute as u16)
}

pub fn block_number(cell: Cell) -> u16 {
    block_index(cell) + 1
}

pub fn day_percentage(cell: Cell) -> f64 {
    (block_index(cell) as f64 / BLOCKS_PER_DAY as f64) * 100.0
}

pub fn cell_from_block(index: u16) -> Cell {
    let clamped = index.min(BLOCKS_PER_DAY - 1);
    Cell {
        hour: (clamped / (MINUTES_PER_HOUR as u16)) as u8,
        minute: (clamped % (MINUTES_PER_HOUR as u16)) as u8,
    }
}

pub fn state_at(now: Cell, cell: Cell) -> CellState {
    if cell.hour < now.hour {
        CellState::PastDay
    } else if cell.hour > now.hour {
        CellState::Future
    } else if cell.minute < now.minute {
        CellState::CurrentHourPast
    } else if cell.minute == now.minute {
        CellState::Now
    } else {
        CellState::Future
    }
}

pub fn cell_state_label(state: CellState) -> &'static str {
    match state {
        CellState::Future => "Upcoming",
        CellState::PastDay => "Earlier today",
        CellState::CurrentHourPast => "Earlier this hour",
        CellState::Now => "Current minute",
    }
}

pub fn format_time(cell: Cell, mode: TimeFormat) -> String {
    match mode {
        TimeFormat::Auto | TimeFormat::Hour24 => format!("{:02}:{:02}", cell.hour, cell.minute),
        TimeFormat::Hour12 => {
            let period = if cell.hour < 12 { "AM" } else { "PM" };
            let h12 = match cell.hour % 12 {
                0 => 12,
                h => h,
            };
            format!("{}:{:02} {}", h12, cell.minute, period)
        }
    }
}

pub fn adjacent_cell(cell: Cell, dir: NavDirection) -> Cell {
    let idx = block_index(cell) as i32;
    let row_start = (cell.hour as i32) * (MINUTES_PER_HOUR as i32);
    let row_end = row_start + (MINUTES_PER_HOUR as i32) - 1;

    match dir {
        NavDirection::Up => Cell::new(cell.hour.saturating_sub(1), cell.minute),
        NavDirection::Down => Cell::new((cell.hour + 1).min(HOURS - 1), cell.minute),
        NavDirection::Left => cell_from_block((idx - 1).max(0) as u16),
        NavDirection::Right => cell_from_block((idx + 1).min((BLOCKS_PER_DAY - 1) as i32) as u16),
        NavDirection::Home => cell_from_block(row_start as u16),
        NavDirection::End => cell_from_block(row_end as u16),
    }
}

pub fn current_now() -> NowSnapshot {
    let local_now = Local::now();
    snapshot_from_local(local_now, iana_time_zone::get_timezone().ok())
}

fn snapshot_from_local(local_now: DateTime<Local>, iana_tz: Option<String>) -> NowSnapshot {
    let cell = Cell::new(local_now.hour() as u8, local_now.minute() as u8);
    let date_label = local_now.format("%A, %-d %B").to_string();

    let tz_label = iana_tz.unwrap_or_else(|| {
        let secs = local_now.offset().fix().local_minus_utc();
        let sign = if secs >= 0 { '+' } else { '-' };
        let abs = secs.abs();
        format!("UTC{}{:02}:{:02}", sign, abs / 3600, (abs % 3600) / 60)
    });

    let is_dst = detect_local_dst(local_now);

    NowSnapshot {
        cell,
        date_label,
        tz_label,
        is_dst,
    }
}

fn detect_local_dst(local_now: DateTime<Local>) -> bool {
    let year = local_now.year();
    let jan = offset_for_local_date(year, 1, 15);
    let jul = offset_for_local_date(year, 7, 15);
    match (jan, jul) {
        (Some(w), Some(s)) if w != s => {
            let current = local_now.offset().fix().local_minus_utc();
            current == w.max(s)
        }
        _ => false,
    }
}

fn offset_for_local_date(year: i32, month: u32, day: u32) -> Option<i32> {
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let naive_dt = date.and_hms_opt(12, 0, 0)?;
    let local_dt = Local.from_local_datetime(&naive_dt).earliest()?;
    Some(local_dt.offset().fix().local_minus_utc())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math_conversions() {
        assert_eq!(block_index(Cell::new(0, 0)), 0);
        assert_eq!(block_number(Cell::new(0, 0)), 1);
        assert_eq!(block_index(Cell::new(23, 59)), 1439);
        assert_eq!(block_number(Cell::new(23, 59)), 1440);
        assert_eq!(cell_from_block(0), Cell::new(0, 0));
        assert_eq!(cell_from_block(872), Cell::new(14, 32));
        assert_eq!(cell_from_block(9999), Cell::new(23, 59));
        assert!((day_percentage(Cell::new(12, 0)) - 50.0).abs() < 1e-6);
    }

    #[test]
    fn state_transitions() {
        let now = Cell::new(14, 30);
        assert_eq!(state_at(now, Cell::new(13, 59)), CellState::PastDay);
        assert_eq!(state_at(now, Cell::new(14, 0)), CellState::CurrentHourPast);
        assert_eq!(state_at(now, Cell::new(14, 29)), CellState::CurrentHourPast);
        assert_eq!(state_at(now, Cell::new(14, 30)), CellState::Now);
        assert_eq!(state_at(now, Cell::new(14, 31)), CellState::Future);
        assert_eq!(state_at(now, Cell::new(15, 0)), CellState::Future);
    }

    #[test]
    fn time_formatting() {
        assert_eq!(format_time(Cell::new(0, 5), TimeFormat::Auto), "00:05");
        assert_eq!(format_time(Cell::new(0, 5), TimeFormat::Hour24), "00:05");
        assert_eq!(format_time(Cell::new(0, 5), TimeFormat::Hour12), "12:05 AM");
        assert_eq!(
            format_time(Cell::new(12, 0), TimeFormat::Hour12),
            "12:00 PM"
        );
        assert_eq!(format_time(Cell::new(13, 5), TimeFormat::Hour12), "1:05 PM");
        assert_eq!(
            format_time(Cell::new(23, 59), TimeFormat::Hour12),
            "11:59 PM"
        );
    }

    #[test]
    fn grid_navigation() {
        assert_eq!(
            adjacent_cell(Cell::new(0, 0), NavDirection::Left),
            Cell::new(0, 0)
        );
        assert_eq!(
            adjacent_cell(Cell::new(0, 0), NavDirection::Up),
            Cell::new(0, 0)
        );
        assert_eq!(
            adjacent_cell(Cell::new(1, 0), NavDirection::Left),
            Cell::new(0, 59)
        );
        assert_eq!(
            adjacent_cell(Cell::new(0, 59), NavDirection::Right),
            Cell::new(1, 0)
        );
        assert_eq!(
            adjacent_cell(Cell::new(23, 59), NavDirection::Right),
            Cell::new(23, 59)
        );
        assert_eq!(
            adjacent_cell(Cell::new(23, 15), NavDirection::Down),
            Cell::new(23, 15)
        );
        assert_eq!(
            adjacent_cell(Cell::new(10, 27), NavDirection::Home),
            Cell::new(10, 0)
        );
        assert_eq!(
            adjacent_cell(Cell::new(10, 27), NavDirection::End),
            Cell::new(10, 59)
        );
    }

    #[test]
    fn current_now_returns_valid_snapshot() {
        let snap = current_now();
        assert!(snap.cell.hour < HOURS);
        assert!(snap.cell.minute < MINUTES_PER_HOUR);
        assert!(!snap.date_label.is_empty());
        assert!(!snap.tz_label.is_empty());
    }
}
