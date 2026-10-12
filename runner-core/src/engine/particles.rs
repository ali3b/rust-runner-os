//! Fixed-size pool of short-lived particles in 1/16-pixel fixed point.
use super::{
    graphics::{Canvas, Color},
    rng::Rng,
};

pub const MAX_PARTICLES: usize = 640;
/// Positions and velocities are in 1/`FP` pixels.
pub const FP: i32 = 16;

#[derive(Clone, Copy, Debug, Default)]
pub struct Particle {
    x: i32,
    y: i32,
    vx: i32,
    vy: i32,
    /// Updates left; 0 = free slot.
    life: u16,
    max_life: u16,
    color: Color,
    size: u8,
    gravity: bool,
}

/// What a burst looks like.
#[derive(Clone, Copy, Debug)]
pub struct Burst {
    pub count: u32,
    /// Max speed in px per update.
    pub speed: i32,
    pub life: u16,
    pub size: u8,
    pub gravity: bool,
    /// Colors picked at random.
    pub colors: &'static [Color],
    /// Bias the spray to the right (debris flying away from the hero) when > 0.
    pub push_x: i32,
}

#[derive(Clone, Debug)]
pub struct Particles {
    pool: [Particle; MAX_PARTICLES],
    next: usize,
}

impl Particles {
    pub fn new() -> Self {
        Self { pool: [Particle::default(); MAX_PARTICLES], next: 0 }
    }

    pub fn clear(&mut self) {
        self.pool = [Particle::default(); MAX_PARTICLES];
    }

    pub fn alive(&self) -> usize {
        self.pool.iter().filter(|p| p.life > 0).count()
    }

    /// Spawns a burst at `(x, y)`. When the pool is full, the oldest are recycled.
    pub fn burst(&mut self, x: i32, y: i32, b: &Burst, rng: &mut Rng) {
        let span = (b.speed * FP).max(1) as u32;
        for _ in 0..b.count {
            let vx = rng.range(0, span * 2) as i32 - span as i32 + b.push_x * FP;
            let vy = rng.range(0, span * 2) as i32 - span as i32 - if b.gravity { span as i32 / 2 } else { 0 };
            let color = b.colors[rng.range(0, b.colors.len() as u32 - 1) as usize];
            let life = b.life / 2 + rng.range(0, b.life as u32 / 2) as u16;
            self.spawn(x * FP, y * FP, vx, vy, life, color, b.size, b.gravity);
        }
    }

    /// Spawns one particle. Position and velocity are in 1/[`FP`] px (per update).
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(&mut self, x: i32, y: i32, vx: i32, vy: i32, life: u16, color: Color, size: u8, gravity: bool) {
        let life = life.max(1);
        self.pool[self.next] = Particle { x, y, vx, vy, life, max_life: life, color, size, gravity };
        self.next = (self.next + 1) % MAX_PARTICLES;
    }

    /// Moves everything left by `scroll` (1/[`FP`] px; the world moving, so
    /// particles stay where they were dropped) and advances one update.
    pub fn update(&mut self, scroll: i32) {
        for p in self.pool.iter_mut().filter(|p| p.life > 0) {
            p.life -= 1;
            p.x += p.vx - scroll;
            p.y += p.vy;
            if p.gravity {
                p.vy += FP / 3;
            }
        }
    }

    pub fn draw(&self, c: &mut Canvas<'_>) {
        for p in self.pool.iter().filter(|p| p.life > 0) {
            // Shrink as they age, down to a single pixel.
            let s = ((p.size as u32 * p.life as u32).div_ceil(p.max_life as u32) as i32).max(1);
            c.fill_rect(p.x / FP - s / 2, p.y / FP - s / 2, s, s, p.color);
        }
    }
}

impl Default for Particles {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Burst = Burst { count: 100, speed: 3, life: 30, size: 3, gravity: true, colors: &[1, 2], push_x: 0 };

    #[test]
    fn bursts_live_then_die() {
        let mut p = Particles::new();
        let mut rng = Rng::new(1);
        p.burst(100, 100, &B, &mut rng);
        assert_eq!(p.alive(), 100);
        for _ in 0..40 {
            p.update(0);
        }
        assert_eq!(p.alive(), 0);
    }

    #[test]
    fn particles_scroll_with_the_world() {
        let mut p = Particles::new();
        p.spawn(100 * FP, 50 * FP, 0, 0, 100, 3, 2, false);
        // 3.5 px of world travel per update for 10 updates.
        for _ in 0..10 {
            p.update(56);
        }
        let q = p.pool.iter().find(|q| q.life > 0).unwrap();
        assert_eq!(q.x, 100 * FP - 560);
        assert_eq!(q.y, 50 * FP);
    }

    #[test]
    fn pool_recycles_instead_of_overflowing() {
        let mut p = Particles::new();
        let mut rng = Rng::new(2);
        for _ in 0..(MAX_PARTICLES / 100 + 2) {
            p.burst(0, 0, &B, &mut rng);
        }
        assert_eq!(p.alive(), MAX_PARTICLES);
    }
}
