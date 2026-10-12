//! Gold bars and power-up items.
use super::tuning::*;
use crate::engine::collision::Aabb;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coin {
    /// Centre.
    pub x: f32,
    pub y: f32,
    /// Seconds since collection, if collected (drawn as a sparkle, then removed).
    pub taken: Option<f32>,
    /// Scrolled past the hero without being collected.
    pub missed: bool,
}

impl Coin {
    pub fn bounds(&self) -> Aabb {
        Aabb::new(self.x - COIN_W / 2.0, self.y - COIN_H / 2.0, COIN_W, COIN_H)
    }
}

/// Result of one coin update.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoinUpdate {
    pub collected: u32,
    pub missed: u32,
}

#[derive(Clone, Debug)]
pub struct Coins {
    pub slots: [Option<Coin>; MAX_COINS],
}

impl Coins {
    pub fn new() -> Self {
        Self { slots: [None; MAX_COINS] }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Coin> {
        self.slots.iter().flatten()
    }

    pub fn add(&mut self, x: f32, y: f32) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
            *slot = Some(Coin { x, y, taken: None, missed: false });
        }
    }

    /// Scrolls, applies the magnet, collects and ages sparkles. `dt` is real time.
    pub fn update(&mut self, dt: f32, travel: f32, body: &Aabb, magnet: bool) -> CoinUpdate {
        let mut out = CoinUpdate::default();
        let (cx, cy) = (body.x + body.w / 2.0, body.y + body.h / 2.0);
        for slot in self.slots.iter_mut() {
            let Some(c) = slot else { continue };
            c.x -= travel;
            if let Some(age) = &mut c.taken {
                *age += dt;
                if *age > SPARKLE_TIME {
                    *slot = None;
                }
                continue;
            }
            if magnet && !c.missed {
                let (dx, dy) = (cx - c.x, cy - c.y);
                if dx.abs() < MAGNET_RANGE && dy.abs() < MAGNET_RANGE {
                    let step = MAGNET_PULL * dt;
                    c.x += dx.clamp(-step, step);
                    c.y += dy.clamp(-step, step);
                }
            }
            if c.bounds().intersects(body) {
                c.taken = Some(0.0);
                out.collected += 1;
            } else if !c.missed && c.x + COIN_W / 2.0 < body.x {
                c.missed = true;
                out.missed += 1;
            }
            if c.x + COIN_W < -20.0 {
                *slot = None;
            }
        }
        out
    }
}

impl Default for Coins {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerKind {
    Shield,
    Magnet,
    SlowMo,
}

impl PowerKind {
    pub fn name(self) -> &'static str {
        match self {
            PowerKind::Shield => "SHIELD",
            PowerKind::Magnet => "MAGNET",
            PowerKind::SlowMo => "SLOW-MO",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    pub kind: PowerKind,
    pub x: f32,
    pub y: f32,
}

impl Item {
    pub fn bounds(&self) -> Aabb {
        let s = POWERUP_SIZE;
        Aabb::new(self.x - s / 2.0, self.y - s / 2.0, s, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> Aabb {
        Aabb::new(PLAYER_X, GROUND_Y - PLAYER_H, PLAYER_W, PLAYER_H)
    }

    #[test]
    fn collect_once_then_sparkle_then_gone() {
        let mut coins = Coins::new();
        coins.add(PLAYER_X + 10.0, GROUND_Y - 20.0);
        assert_eq!(coins.update(0.01, 0.0, &body(), false).collected, 1);
        assert_eq!(coins.update(0.01, 0.0, &body(), false).collected, 0);
        coins.update(SPARKLE_TIME + 0.01, 0.0, &body(), false);
        assert_eq!(coins.iter().count(), 0);
    }

    #[test]
    fn passing_coin_counts_as_missed_once() {
        let mut coins = Coins::new();
        coins.add(PLAYER_X + 10.0, 100.0); // far above the hero
        let r = coins.update(0.01, 40.0, &body(), false);
        assert_eq!(r, CoinUpdate { collected: 0, missed: 1 });
        assert_eq!(coins.update(0.01, 1.0, &body(), false).missed, 0);
    }

    #[test]
    fn magnet_pulls_only_within_range() {
        let mut coins = Coins::new();
        let b = body();
        coins.add(b.x + 100.0, b.y - 60.0);
        coins.add(b.x + 400.0, b.y - 60.0);
        for _ in 0..60 {
            coins.update(1.0 / 60.0, 0.0, &b, true);
        }
        let left: Vec<_> = coins.iter().filter(|c| c.taken.is_none()).collect();
        assert_eq!(left.len(), 1, "near coin collected, far one untouched");
        assert_eq!(left[0].x, b.x + 400.0);
    }
}
