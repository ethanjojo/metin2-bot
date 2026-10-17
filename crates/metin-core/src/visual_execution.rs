use super::*;
use crate::{AfterSuccess, SafetyRule, Verification};
#[derive(Debug, Clone)]
pub(super) struct VisualExecution {
    pub(super) rule: SafetyRule,
    pub(super) sent_at: Option<f64>,
    attempts: u32,
    absent: u32,
    last_sample: Option<f64>,
}
impl Controller {
    pub(super) fn rule_event(&mut self, rule: &SafetyRule, message: &str, warning: bool) {
        self.event("Visual rule", message, warning);
        if let Some(e) = self.events.back_mut() {
            e.subject = Some(rule.id);
            e.notify = warning && rule.reaction.notify;
        }
    }
    pub(super) fn begin_visual(&mut self, now: f64, rule: SafetyRule) -> Option<Action> {
        if self.phase == Phase::Break {
            self.pause(
                now,
                "Visual action detected during a scheduled break; check the game before resuming.",
            );
            return None;
        }
        // Discard interrupted sequences; never resume their stale input or target.
        if self.phase == Phase::Buffing {
            self.buffs.mounted = None;
        }
        self.buffs.pause(now);
        self.buffs.resume(now);
        self.puller.cancel();
        self.out_of_target = None;
        self.scheduled = None;
        self.loot_left = 0;
        self.pending_action = None;
        self.phase = Phase::VisualRule;
        self.since = now;
        self.rule_event(
            &rule,
            &format!(
                "{}: releasing input before checking a fresh frame.",
                rule.name
            ),
            false,
        );
        self.status(
            "Visual action",
            format!("{}: checking the event before input.", rule.name),
        );
        self.visual = Some(VisualExecution {
            rule,
            sent_at: None,
            attempts: 0,
            absent: 0,
            last_sample: None,
        });
        self.issue(ActionKind::ReleaseAttack)
    }
    pub(super) fn visual_ack(&mut self, now: f64) {
        if let Some(v) = self.visual.as_mut() {
            v.sent_at = Some(now);
            v.attempts += 1;
            v.absent = 0;
            v.last_sample = None;
            let rule = v.rule.clone();
            self.rule_event(
                &rule,
                &format!(
                    "{}: input sent; waiting for {}.",
                    rule.name,
                    if rule.execution.verification == Verification::ReferenceGone {
                        "confirmed reference disappearance"
                    } else {
                        "the configured delay (outcome is not verified)"
                    }
                ),
                false,
            );
        }
    }
    pub(super) fn visual_step(&mut self, now: f64, o: &Observation) -> Option<Action> {
        if self.pending_action.is_some() {
            return None;
        }
        let v = self.visual.as_mut()?;
        let score = o
            .safety_scores
            .iter()
            .find(|(id, _)| *id == v.rule.id)
            .map(|(_, s)| *s)?;
        let matched = score >= v.rule.threshold;
        let gap = 1.0_f64.max(self.profile.timing.sample_interval * 1.5);
        if v.last_sample
            .is_none_or(|last| now <= last || now - last > gap)
        {
            v.absent = 0;
        }
        v.last_sample = Some(now);
        v.absent = if matched { 0 } else { v.absent + 1 };
        let gone = v.absent >= self.profile.panel.stable_frames.max(2);
        let rule = v.rule.clone();
        let mut complete = false;
        if let Some(sent) = v.sent_at {
            let elapsed = now - sent;
            if elapsed >= rule.execution.wait_seconds
                && (rule.execution.verification == Verification::Delay || gone)
            {
                complete = true;
            } else if elapsed >= rule.execution.wait_seconds + rule.execution.timeout_seconds {
                if gone {
                    complete = true;
                } else if matched && v.attempts < rule.execution.attempts {
                    v.sent_at = None;
                    self.rule_event(
                        &rule,
                        &format!(
                            "{}: result not confirmed; retrying ({}/{}).",
                            rule.name,
                            self.visual.as_ref().unwrap().attempts + 1,
                            rule.execution.attempts
                        ),
                        true,
                    );
                    return self.issue(ActionKind::Configured {
                        input: rule.execution.input.clone(),
                    });
                } else {
                    let attempts = v.attempts;
                    self.pause(
                        now,
                        format!(
                            "{}: result was not confirmed after {} attempts.",
                            rule.name, attempts
                        ),
                    );
                    self.rule_event(
                        &rule,
                        &format!("{}: verification failed; automation paused.", rule.name),
                        true,
                    );
                    return None;
                }
            }
        } else if v.attempts == 0 {
            if gone {
                // Event disappeared before input. Do not click the old button.
                self.visual = None;
                self.fresh_visual_state(now);
                self.rule_event(
                    &rule,
                    &format!("{} disappeared before input; action cancelled.", rule.name),
                    false,
                );
                return None;
            }
            if now - self.since >= rule.execution.timeout_seconds {
                self.pause(
                    now,
                    format!(
                        "{}: event could not be reconfirmed before input.",
                        rule.name
                    ),
                );
                self.rule_event(
                    &rule,
                    "Visual action cancelled before input; confirmation timed out.",
                    true,
                );
                return None;
            }
            if matched {
                return self.issue(ActionKind::Configured {
                    input: rule.execution.input.clone(),
                });
            }
        }
        if complete {
            let verified = rule.execution.verification == Verification::ReferenceGone;
            self.visual = None;
            self.rule_event(
                &rule,
                &format!(
                    "{}: {}.",
                    rule.name,
                    if verified {
                        "reference disappearance confirmed"
                    } else {
                        "delay completed; outcome not verified"
                    }
                ),
                false,
            );
            if rule.execution.after_success == AfterSuccess::Pause
                || self.buffs.mounted.is_none() && self.profile.buffs.enabled
            {
                self.pause(
                    now,
                    format!("{} finished. Check the game before resuming.", rule.name),
                );
                self.rule_event(&rule, &format!("{}: finished and paused.", rule.name), true);
            } else {
                self.fresh_visual_state(now);
            }
        }
        None
    }
    fn fresh_visual_state(&mut self, now: f64) {
        self.phase = Phase::Initial;
        self.since = now;
        self.present = 0;
        self.absent = 0;
        self.last_observation = None;
        self.clicked = None;
        self.failed.clear();
        self.uncertain_since = None;
        self.attempted.clear();
        self.rotations = 0;
        self.failures = 0;
        self.search_budget.reset();
        self.status(
            "Checking target",
            "Visual action finished; checking fresh game state.",
        );
    }
}
