use std::time::{Duration, Instant};

use crate::{config::LiftUiConfig, model::LiftAction};

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

pub struct Sample {
    pub offset: f32,
    pub reveal: f32,
    pub next: Option<Duration>,
}

impl Sample {
    fn resting() -> Self {
        Self {
            offset: 0.0,
            reveal: 0.0,
            next: None,
        }
    }
}

struct Visibility {
    from: f32,
    to: f32,
    started: Instant,
}

impl Visibility {
    fn value(&self, now: Instant, transition: Duration) -> f32 {
        let progress = (now.saturating_duration_since(self.started).as_secs_f64()
            / transition.as_secs_f64())
        .clamp(0.0, 1.0);
        self.from + (self.to - self.from) * ease(progress)
    }

    fn retarget(&mut self, to: f32, now: Instant, transition: Duration) {
        if self.to != to {
            self.from = self.value(now, transition);
            self.to = to;
            self.started = now;
        }
    }

    fn moving(&self, now: Instant, transition: Duration) -> bool {
        self.from != self.to && now.saturating_duration_since(self.started) < transition
    }
}

struct RowScroll {
    action: LiftAction,
    entered: Instant,
    frozen: Option<Duration>,
    visibility: Visibility,
}

#[derive(Default)]
pub struct HoverScroll {
    rows: Vec<RowScroll>,
}

impl HoverScroll {
    pub fn clear(&mut self) {
        self.rows.clear();
    }

    pub fn reconcile(
        &mut self,
        hovered: Option<&LiftAction>,
        visible: &[LiftAction],
        now: Instant,
        config: &LiftUiConfig,
    ) {
        let transition = Duration::from_millis(config.hover_scroll_transition_ms);
        self.rows.retain(|row| {
            visible.contains(&row.action)
                && (row.visibility.to != 0.0 || row.visibility.moving(now, transition))
        });
        for row in &mut self.rows {
            if hovered == Some(&row.action) {
                if let Some(elapsed) = row.frozen.take() {
                    row.entered = now - elapsed;
                }
                row.visibility.retarget(1.0, now, transition);
            } else {
                row.frozen
                    .get_or_insert_with(|| now.saturating_duration_since(row.entered));
                row.visibility.retarget(0.0, now, transition);
            }
        }
        if let Some(action) = hovered
            && !self.rows.iter().any(|row| &row.action == action)
        {
            self.rows.push(RowScroll {
                action: action.clone(),
                entered: now,
                frozen: None,
                visibility: Visibility {
                    from: 1.0,
                    to: 1.0,
                    started: now,
                },
            });
        }
    }

    pub fn sample(
        &self,
        action: &LiftAction,
        now: Instant,
        distance: f32,
        config: &LiftUiConfig,
    ) -> Sample {
        let Some(row) = self.rows.iter().find(|row| &row.action == action) else {
            return Sample::resting();
        };
        let elapsed = row
            .frozen
            .unwrap_or_else(|| now.saturating_duration_since(row.entered));
        let mut sample = sample_cycle(elapsed, distance, config);
        let transition = Duration::from_millis(config.hover_scroll_transition_ms);
        sample.reveal *= row.visibility.value(now, transition);
        if row.frozen.is_some() {
            sample.next = None;
        }
        if row.visibility.moving(now, transition) && distance > 0.0 {
            sample.next = Some(FRAME_INTERVAL);
        }
        sample
    }
}

fn ease(progress: f64) -> f32 {
    (progress * progress * (3.0 - 2.0 * progress)) as f32
}

// Accelerate briefly, cruise at the configured speed, then decelerate. Very
// short journeys use a triangular velocity profile and never overshoot.
fn movement(distance: f64, speed: f64, ramp: f64) -> (f64, f64) {
    let acceleration = speed / ramp;
    let accelerate_for = ramp.min((distance / acceleration).sqrt());
    let peak_speed = acceleration * accelerate_for;
    (distance / peak_speed + accelerate_for, accelerate_for)
}

fn travelled(time: f64, distance: f64, speed: f64, ramp: f64) -> f32 {
    let (duration, accelerate_for) = movement(distance, speed, ramp);
    let acceleration = speed / ramp;
    let position = if time < accelerate_for {
        acceleration * time * time / 2.0
    } else if time > duration - accelerate_for {
        let remaining = (duration - time).max(0.0);
        distance - acceleration * remaining * remaining / 2.0
    } else {
        acceleration * accelerate_for * (time - accelerate_for / 2.0)
    };
    position.clamp(0.0, distance) as f32
}

fn sample_cycle(elapsed: Duration, distance: f32, config: &LiftUiConfig) -> Sample {
    if distance <= 0.0 || !config.hover_scroll {
        return Sample::resting();
    }
    let distance = f64::from(distance);
    let speed = f64::from(config.hover_scroll_speed.max(1));
    let transition = (config.hover_scroll_transition_ms.max(1) as f64) / 1000.0;
    let delay = config.hover_scroll_delay_ms as f64 / 1000.0;
    let pause = config.hover_scroll_pause_ms as f64 / 1000.0;
    let (duration, _) = movement(distance, speed, transition);
    // Keep each pass in one direction. Retracing a long label at reading speed
    // can take minutes; fade back to the ellipsis instead, then repeat.
    let cycle = delay + transition * 2.0 + duration + pause;
    let mut time = elapsed.as_secs_f64() % cycle;
    if time < delay {
        return Sample {
            offset: 0.0,
            reveal: 0.0,
            next: Some(Duration::from_secs_f64(delay - time)),
        };
    }
    time -= delay;
    let (offset, reveal) = if time < transition {
        (0.0, ease(time / transition))
    } else {
        time -= transition;
        if time < duration {
            (travelled(time, distance, speed, transition), 1.0)
        } else {
            time -= duration;
            if time < pause {
                return Sample {
                    offset: distance as f32,
                    reveal: 1.0,
                    next: Some(Duration::from_secs_f64(pause - time)),
                };
            }
            time -= pause;
            (distance as f32, 1.0 - ease(time / transition))
        }
    };
    Sample {
        offset,
        reveal,
        next: Some(FRAME_INTERVAL),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pauses_eases_and_fades_before_repeating_forward() {
        let config = LiftUiConfig::default();
        let distance = 90.0;
        let transition = config.hover_scroll_transition_ms as f64 / 1000.0;
        let delay = config.hover_scroll_delay_ms as f64 / 1000.0;
        let (duration, _) = movement(distance, config.hover_scroll_speed as f64, transition);
        let sample =
            |seconds| sample_cycle(Duration::from_secs_f64(seconds), distance as f32, &config);
        assert_eq!(sample(0.0).reveal, 0.0);
        assert_eq!(sample(delay / 2.0).offset, 0.0);
        assert!(sample(delay + transition / 2.0).reveal > 0.0);
        let start = delay + transition;
        assert!(sample(start + 0.01).offset < 0.1);
        assert!((sample(start + duration / 2.0).offset - 45.0).abs() < 0.01);
        assert_eq!(sample(start + duration + 0.1).offset, 90.0);
        let end = start + duration + config.hover_scroll_pause_ms as f64 / 1000.0 + transition;
        assert_eq!(sample(end - 0.001).offset, 90.0);
        assert!(sample(end - 0.001).reveal < 0.01);
        assert_eq!(sample(end + 0.001).offset, 0.0);
        assert_eq!(sample(end + 0.001).reveal, 0.0);
        assert!((sample(end + start + duration / 2.0).offset - 45.0).abs() < 0.01);
        assert!(
            sample_cycle(Duration::from_secs(10), 0.0, &config)
                .next
                .is_none()
        );
    }

    #[test]
    fn scheduled_deadlines_repeat_without_pointer_events() {
        let config = LiftUiConfig::default();
        let start = Instant::now();
        let action = LiftAction::FocusNode { id: 7 };
        let visible = vec![action.clone()];
        let mut scroll = HoverScroll::default();
        scroll.reconcile(Some(&action), &visible, start, &config);
        // Fractional endpoints cover deadline rounding; tiny overflow covers
        // the triangular acceleration profile as well as longer labels.
        for distance in [0.3, 90.7, 1250.25] {
            let mut now = start;
            let mut previous = 0.0;
            let mut previous_reveal = 0.0;
            let mut resets = 0;
            let mut passes = 0;
            for _ in 0..20_000 {
                scroll.reconcile(Some(&action), &visible, now, &config);
                let sample = scroll.sample(&action, now, distance, &config);
                assert!((0.0..=distance).contains(&sample.offset));
                if sample.offset < previous {
                    // Position resets only after the full text has faded out.
                    assert!(previous_reveal < 0.03);
                    assert_eq!(sample.reveal, 0.0);
                    resets += 1;
                }
                if sample.offset == distance && previous < distance {
                    passes += 1;
                }
                if resets == 3 {
                    break;
                }
                previous = sample.offset;
                previous_reveal = sample.reveal;
                let next = sample
                    .next
                    .expect("hover must always schedule another pass");
                // A real clock advances even when sub-nanosecond boundaries
                // round down. Simulate at least a microsecond of dispatch time.
                now += next.max(Duration::from_micros(1));
            }
            assert_eq!(resets, 3, "distance={distance}");
            assert_eq!(passes, 3, "distance={distance}");
        }
    }

    #[test]
    fn leaving_fades_at_a_fixed_position_and_reentry_preserves_progress() {
        let config = LiftUiConfig::default();
        let start = Instant::now();
        let action = LiftAction::FocusNode { id: 7 };
        let visible = vec![action.clone()];
        let mut scroll = HoverScroll::default();
        scroll.reconcile(Some(&action), &visible, start, &config);
        let leave = start + Duration::from_secs(3);
        let before = scroll.sample(&action, leave, 100.0, &config);
        scroll.reconcile(None, &visible, leave, &config);
        let return_at = leave + Duration::from_millis(90);
        let fading = scroll.sample(&action, return_at, 100.0, &config);
        assert_eq!(fading.offset, before.offset);
        assert!(fading.reveal > 0.0 && fading.reveal < before.reveal);
        scroll.reconcile(Some(&action), &visible, return_at, &config);
        let reentered = scroll.sample(&action, return_at, 100.0, &config);
        assert_eq!(reentered.offset, fading.offset);
        assert_eq!(reentered.reveal, fading.reveal);
        scroll.reconcile(None, &visible, return_at, &config);
        let finished = return_at + Duration::from_millis(config.hover_scroll_transition_ms);
        scroll.reconcile(None, &visible, finished, &config);
        assert!(
            scroll
                .sample(&action, finished, 100.0, &config)
                .next
                .is_none()
        );
        assert!(scroll.rows.is_empty());
    }

    #[test]
    fn disappearing_targets_release_animation_state() {
        let config = LiftUiConfig::default();
        let now = Instant::now();
        let action = LiftAction::FocusNode { id: 7 };
        let mut scroll = HoverScroll::default();
        scroll.reconcile(Some(&action), std::slice::from_ref(&action), now, &config);
        scroll.reconcile(None, &[], now, &config);
        assert!(scroll.rows.is_empty());
    }
}
