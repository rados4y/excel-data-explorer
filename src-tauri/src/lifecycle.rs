//! Pure launch/close policy shared by the native shell and regression tests.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub autostart: bool,
    pub close_to_tray: bool,
    pub sync_paused: bool,
    pub selected_workspace: Option<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            autostart: true,
            close_to_tray: true,
            sync_paused: false,
            selected_workspace: None,
        }
    }
}
#[derive(Debug, PartialEq)]
pub enum LaunchAction {
    Hidden,
    Show,
}
pub fn launch_action(args: &[String], second_instance: bool) -> LaunchAction {
    if !second_instance && args.iter().any(|a| a == "--background") {
        LaunchAction::Hidden
    } else {
        LaunchAction::Show
    }
}
#[derive(Debug, PartialEq)]
pub enum CloseAction {
    Hide,
    Exit,
}
pub fn close_action(settings: &Settings, explicit_exit: bool) -> CloseAction {
    if settings.close_to_tray && !explicit_exit {
        CloseAction::Hide
    } else {
        CloseAction::Exit
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_enable_hidden_login_and_tray() {
        let s = Settings::default();
        assert!(s.autostart && s.close_to_tray && !s.sync_paused);
    }
    #[test]
    fn background_does_not_flash() {
        assert_eq!(
            launch_action(&["--background".into()], false),
            LaunchAction::Hidden
        );
    }
    #[test]
    fn manual_and_second_launch_show() {
        assert_eq!(launch_action(&[], false), LaunchAction::Show);
        assert_eq!(
            launch_action(&["--background".into()], true),
            LaunchAction::Show
        );
    }
    #[test]
    fn close_hides_explicit_exit_terminates() {
        let s = Settings::default();
        assert_eq!(close_action(&s, false), CloseAction::Hide);
        assert_eq!(close_action(&s, true), CloseAction::Exit);
    }
}
