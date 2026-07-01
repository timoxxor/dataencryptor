use std::path::PathBuf;

use super::event::PasswordEvent;
use super::password::PasswordModal;

pub struct PasswordPopup<'a> {
    should_show: bool,
    is_opening: bool,
    password_buffer: &'a mut String,
}

impl<'a> PasswordPopup<'a> {
    pub fn new(
        pending_save_path: Option<&PathBuf>,
        pending_open_path: Option<&PathBuf>,
        password_buffer: &'a mut String,
    ) -> Self {
        let should_show = pending_save_path.is_some() || pending_open_path.is_some();
        let is_opening = pending_open_path.is_some();
        Self {
            should_show,
            is_opening,
            password_buffer,
        }
    }

    pub fn show(self, ctx: &egui::Context) -> Option<PasswordEvent> {
        if !self.should_show {
            return None;
        }
        PasswordModal::new(self.is_opening, self.password_buffer).show(ctx)
    }
}
