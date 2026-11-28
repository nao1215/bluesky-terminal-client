//! A selection must rest before a background read starts.
use std::time::{Duration, Instant};

/// Tracks one selection, firing once after it rests for the given delay.
#[derive(Debug, Clone, Default)]
pub struct Dwell(Option<(String, Instant, bool)>);

impl Dwell {
    /// Observe a selection; `None` resets it. Returns true once per rest.
    pub fn ready(&mut self, id: Option<&str>, now: Instant, delay: Duration) -> bool {
        let Some(id) = id else {
            self.0 = None;
            return false;
        };
        match &mut self.0 {
            Some((on, since, fired)) if on == id => {
                if *fired || now.saturating_duration_since(*since) < delay {
                    return false;
                }
                *fired = true;
                true
            }
            _ => {
                self.0 = Some((id.into(), now, false));
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn movement_and_absence_restart_the_delay() {
        let mut d = Dwell::default();
        let now = Instant::now();
        let delay = Duration::from_millis(400);
        assert!(!d.ready(Some("a"), now, delay));
        assert!(!d.ready(Some("a"), now + delay / 2, delay));
        assert!(!d.ready(Some("b"), now + delay, delay));
        assert!(d.ready(Some("b"), now + delay * 2, delay));
        assert!(!d.ready(Some("b"), now + delay * 3, delay));
        assert!(!d.ready(None, now + delay * 3, delay));
        assert!(!d.ready(Some("b"), now + delay * 4, delay));
    }
}
