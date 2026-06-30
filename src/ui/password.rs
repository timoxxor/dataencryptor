use egui::{Button, Context, Key, TextEdit, Window};
use zeroize::{Zeroize, Zeroizing};

use super::event::PasswordEvent;

pub struct PasswordModal<'a> {
    is_opening: bool,
    password_buffer: &'a mut String,
}

impl<'a> PasswordModal<'a> {
    pub fn new(
        is_opening: bool,
        password_buffer: &'a mut String,
    ) -> Self {
        Self {
            is_opening,
            password_buffer,
        }
    }

    pub fn show(self, ctx: &Context) -> Option<PasswordEvent> {
        let mut result = None;

        let title = if self.is_opening {
            "🔐 Enter Password to Decrypt"
        } else {
            "🔒 Set Encryption Password"
        };

        let mut open = true;

        Window::new(title)
            .resizable(false)
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(5.0);

                    ui.label("Please enter secure password for EVFS container:");

                    ui.add_space(5.0);

                    let edit = ui.add(
                        TextEdit::singleline(self.password_buffer)
                            .password(true)
                            .hint_text("Password...")
                            .desired_width(240.0),
                    );

                    edit.request_focus();

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        if ui
                            .add_sized([100.0, 30.0], Button::new("Cancel"))
                            .clicked()
                        {
                            self.password_buffer.zeroize();

                            result = Some(PasswordEvent::Cancelled);
                            return;
                        }

                        ui.add_space(10.0);

                        let submit =
                            ui.add_sized([100.0, 30.0], Button::new("OK")).clicked()
                                || ui.input(|i| i.key_pressed(Key::Enter));

                        if submit && !self.password_buffer.is_empty() {
                            let password = std::mem::take(self.password_buffer);

                            result = Some(PasswordEvent::Submitted(
                                Zeroizing::new(password),
                            ));
                        }
                    });
                });
            });

        if !open && result.is_none() {
            self.password_buffer.zeroize();
            self.password_buffer.clear();

            return Some(PasswordEvent::Cancelled);
        }

        result
    }
}
