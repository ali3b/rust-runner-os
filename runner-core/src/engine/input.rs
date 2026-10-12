/// Logical keys the game cares about; the platform layer maps scan codes onto these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Jump,
    Restart,
    Escape,
    Slide,
    Kick,
    /// Toggles the CRT scanline effect.
    Crt,
}

const KEYS: usize = 6;

impl Key {
    const fn index(self) -> usize {
        self as usize
    }
}

/// Input for one fixed update. `*_pressed` fields are edges: true for exactly one
/// update per physical key press.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputState {
    pub jump_pressed: bool,
    pub restart_pressed: bool,
    pub escape_pressed: bool,
    pub slide_pressed: bool,
    pub kick_pressed: bool,
    pub crt_pressed: bool,
}

/// Turns raw key down/up events (including typematic repeats) into edge-triggered
/// presses, latched until the next update consumes them.
#[derive(Debug, Default)]
pub struct InputLatch {
    held: [bool; KEYS],
    pending: InputState,
}

impl InputLatch {
    pub const fn new() -> Self {
        Self {
            held: [false; KEYS],
            pending: InputState {
                jump_pressed: false,
                restart_pressed: false,
                escape_pressed: false,
                slide_pressed: false,
                kick_pressed: false,
                crt_pressed: false,
            },
        }
    }

    pub fn key_event(&mut self, key: Key, down: bool) {
        let was_held = core::mem::replace(&mut self.held[key.index()], down);
        if down && !was_held {
            match key {
                Key::Jump => self.pending.jump_pressed = true,
                Key::Restart => self.pending.restart_pressed = true,
                Key::Escape => self.pending.escape_pressed = true,
                Key::Slide => self.pending.slide_pressed = true,
                Key::Kick => self.pending.kick_pressed = true,
                Key::Crt => self.pending.crt_pressed = true,
            }
        }
    }

    pub fn is_held(&self, key: Key) -> bool {
        self.held[key.index()]
    }

    /// Returns the presses since the last call and clears them.
    pub fn take(&mut self) -> InputState {
        core::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_is_reported_once() {
        let mut l = InputLatch::new();
        l.key_event(Key::Jump, true);
        assert!(l.take().jump_pressed);
        assert!(!l.take().jump_pressed);
        assert!(l.is_held(Key::Jump));
    }

    #[test]
    fn typematic_repeat_does_not_retrigger() {
        let mut l = InputLatch::new();
        l.key_event(Key::Jump, true);
        l.take();
        for _ in 0..20 {
            l.key_event(Key::Jump, true);
            assert!(!l.take().jump_pressed);
        }
        l.key_event(Key::Jump, false);
        l.key_event(Key::Jump, true);
        assert!(l.take().jump_pressed);
    }

    #[test]
    fn quick_tap_between_updates_is_not_lost() {
        let mut l = InputLatch::new();
        l.key_event(Key::Jump, true);
        l.key_event(Key::Jump, false);
        let i = l.take();
        assert!(i.jump_pressed);
        assert!(!l.is_held(Key::Jump));
    }

    #[test]
    fn keys_are_independent() {
        let mut l = InputLatch::new();
        l.key_event(Key::Restart, true);
        l.key_event(Key::Escape, true);
        let i = l.take();
        assert_eq!(i, InputState { restart_pressed: true, escape_pressed: true, ..Default::default() });
        l.key_event(Key::Slide, true);
        l.key_event(Key::Kick, true);
        let i = l.take();
        assert!(i.slide_pressed && i.kick_pressed && !i.jump_pressed);
    }
}
