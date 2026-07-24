//! When to change the icon and when to speak up — the decisions, without the
//! side effects, so they can be tested.
//!
//! The app answers one question: will the Mac fall asleep if its owner walks
//! away. The icon answers it continuously; a notification answers it at the one
//! moment the answer matters most — when the charger comes out and the machine
//! is about to be left alone on battery.

use crate::sleep::Holder;

/// How long a holder has to sit there before the icon calls it real. Below this
/// it is usually a system sound taking the speakers for a second.
pub const SETTLED_SECS: u64 = 600;

/// At the moment the charger is pulled the bar is lower: the question is "will
/// it sleep *now*", and that question makes no allowance for a young holder.
/// Only the momentary blip is still filtered out.
pub const UNPLUG_SECS: u64 = 30;

/// What the menu-bar icon is saying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mood {
    /// Nothing is holding it — walk away.
    Calm,
    /// Something is holding it.
    Blocked,
    /// Plugged in, where staying awake is the point. Nothing to report.
    Charging,
}

/// On the charger the Mac is meant to stay awake so long jobs can finish, so a
/// holder there is the arrangement working, not a fault.
pub fn mood(on_battery: bool, settled_holders: usize) -> Mood {
    if !on_battery {
        Mood::Charging
    } else if settled_holders == 0 {
        Mood::Calm
    } else {
        Mood::Blocked
    }
}

/// Holders old enough to be worth showing.
pub fn settled(holders: &[Holder], min_age: u64) -> Vec<Holder> {
    holders.iter().filter(|h| h.held >= min_age).cloned().collect()
}

/// The one the notification names: the longest-standing holder is the answer to
/// "why won't it sleep", and the order pmset prints them in is arbitrary.
pub fn worst(holders: &[Holder]) -> Option<&Holder> {
    holders.iter().max_by_key(|h| h.held)
}

/// Did the charger just come out?
///
/// Only this transition warrants a notification: while the state is unchanged
/// the icon has been saying the same thing all along, and a banner repeating it
/// every minute is how a useful signal becomes noise.
pub fn just_unplugged(was_on_battery: Option<bool>, on_battery: bool) -> bool {
    matches!(was_on_battery, Some(false)) && on_battery
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sleep::{Blocks, Holder};

    fn holder(app: &str, held: u64) -> Holder {
        Holder {
            app: app.to_string(),
            via: None,
            pid: 1,
            blocks: Blocks::Sleep,
            label: String::new(),
            held,
        }
    }

    #[test]
    fn the_charger_silences_everything() {
        assert_eq!(mood(false, 3), Mood::Charging);
    }

    #[test]
    fn on_battery_the_icon_follows_the_holders() {
        assert_eq!(mood(true, 0), Mood::Calm);
        assert_eq!(mood(true, 1), Mood::Blocked);
    }

    #[test]
    fn a_young_holder_waits_for_the_icon_but_counts_at_the_unplug() {
        let sound = [holder("Google Chrome", 120)];
        assert!(settled(&sound, SETTLED_SECS).is_empty(), "still might let go on its own");
        assert_eq!(settled(&sound, UNPLUG_SECS).len(), 1, "the cord is out — it counts now");
    }

    #[test]
    fn the_notification_names_the_oldest_holder() {
        let holders = [holder("Google Chrome", 720), holder("Transmission", 3900)];
        assert_eq!(worst(&holders).unwrap().app, "Transmission");
    }

    #[test]
    fn only_the_transition_to_battery_speaks() {
        assert!(just_unplugged(Some(false), true), "charger pulled");
        assert!(!just_unplugged(Some(true), true), "already on battery, the icon said it");
        assert!(!just_unplugged(Some(true), false), "plugged back in");
        assert!(!just_unplugged(None, true), "first tick after launch is not an event");
    }
}
