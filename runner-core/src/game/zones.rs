//! The endless run is a loop of four zones, each faster and denser than the last.
use super::{patterns::POOLS, pickups::PowerKind, tuning::*};

pub struct Zone {
    pub name: &'static str,
    /// Scroll speed in px/s on the first lap.
    pub speed: f32,
}

pub static ZONES: [Zone; 4] = [
    Zone { name: "NEON CITY", speed: 200.0 },
    Zone { name: "SUNSET DESERT", speed: 240.0 },
    Zone { name: "DEEP SPACE", speed: 280.0 },
    Zone { name: "HYPER GRID", speed: 320.0 },
];

/// Where a bar sits in the zone loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarInfo {
    pub lap: u32,
    pub zone: usize,
    pub bar_in_zone: u32,
}

pub fn bar_info(bar: u32, zone_bars: u32) -> BarInfo {
    let zone_total = bar / zone_bars;
    BarInfo { lap: zone_total / ZONES.len() as u32, zone: zone_total as usize % ZONES.len(), bar_in_zone: bar % zone_bars }
}

pub fn speed_of_bar(bar: u32, zone_bars: u32) -> f32 {
    let info = bar_info(bar, zone_bars);
    let percent = 100 + (info.lap * LAP_SPEEDUP_PERCENT).min(30);
    ZONES[info.zone].speed * percent as f32 / 100.0
}

/// Intro bars and each zone's closing gate bars are left empty.
pub fn is_empty_bar(bar: u32, zone_bars: u32) -> bool {
    bar < INTRO_BARS || bar_info(bar, zone_bars).bar_in_zone >= zone_bars.saturating_sub(GATE_BARS)
}

pub fn pool(zone: usize) -> &'static [super::patterns::Pattern] {
    POOLS[zone]
}

/// The power-up offered in this bar, if any (one per zone, halfway through).
pub fn power_for_bar(bar: u32, zone_bars: u32) -> Option<PowerKind> {
    let info = bar_info(bar, zone_bars);
    if is_empty_bar(bar, zone_bars) || info.bar_in_zone != (zone_bars - GATE_BARS) / 2 {
        return None;
    }
    Some(match (info.zone + info.lap as usize) % 3 {
        0 => PowerKind::Magnet,
        1 => PowerKind::Shield,
        _ => PowerKind::SlowMo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_cycle_and_laps_speed_up() {
        assert_eq!(bar_info(0, 16), BarInfo { lap: 0, zone: 0, bar_in_zone: 0 });
        assert_eq!(bar_info(17, 16), BarInfo { lap: 0, zone: 1, bar_in_zone: 1 });
        assert_eq!(bar_info(64, 16), BarInfo { lap: 1, zone: 0, bar_in_zone: 0 });
        assert_eq!(speed_of_bar(0, 16), 200.0);
        assert_eq!(speed_of_bar(64, 16), 220.0);
        assert!(speed_of_bar(16 * 4 * 10, 16) <= 200.0 * 1.3 + 0.01, "lap bonus is capped");
    }

    #[test]
    fn gates_and_intro_are_empty() {
        assert!(is_empty_bar(0, 16) && is_empty_bar(1, 16));
        assert!(!is_empty_bar(2, 16));
        assert!(is_empty_bar(14, 16) && is_empty_bar(15, 16));
        assert!(!is_empty_bar(16, 16));
    }

    #[test]
    fn one_power_up_per_zone() {
        for zone_bars in [6, 16] {
            let n = (0..zone_bars * 4).filter(|&b| power_for_bar(b, zone_bars).is_some()).count();
            assert_eq!(n, 4, "zone_bars {zone_bars}");
        }
    }
}

/// World time is measured in 1/[`Q`] updates.
pub const BAR_Q: u64 = BAR_TICKS as u64 * Q;

/// Distance the world scrolls between world times `q0` and `q1`, following the
/// per-bar speed schedule exactly (so things spawned bars ahead arrive on time).
pub fn travel_between(q0: u64, q1: u64, zone_bars: u32) -> f32 {
    let mut d = 0.0;
    let mut q = q0;
    while q < q1 {
        let bar = (q / BAR_Q) as u32;
        let end = ((bar as u64 + 1) * BAR_Q).min(q1);
        d += speed_of_bar(bar, zone_bars) * (end - q) as f32 / (Q as f32 * UPDATES_PER_SECOND as f32);
        q = end;
    }
    d
}

#[cfg(test)]
mod travel_tests {
    use super::*;

    #[test]
    fn travel_follows_speed_changes() {
        let one_bar = BAR_TICKS as f32 / 60.0;
        let d = travel_between(0, BAR_Q, 16);
        assert!((d - 200.0 * one_bar).abs() < 0.01);
        // Straddling the zone 1 -> 2 boundary at bar 16.
        let d = travel_between(15 * BAR_Q, 17 * BAR_Q, 16);
        assert!((d - (200.0 + 240.0) * one_bar).abs() < 0.01);
        // Splitting an interval anywhere gives the same total.
        let whole = travel_between(1000, 40 * BAR_Q + 77, 16);
        let parts = travel_between(1000, 15 * BAR_Q + 3, 16) + travel_between(15 * BAR_Q + 3, 40 * BAR_Q + 77, 16);
        assert!((whole - parts).abs() < 0.05);
    }
}
