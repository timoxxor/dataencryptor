use egui::{Button, Context, Key, TextEdit, Window};
use zeroize::{Zeroize, Zeroizing};

pub enum PasswordResult {
    None,
    Submit(Zeroizing<String>),
    Cancel,
}

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

    pub fn show(self, ctx: &Context) -> PasswordResult {
        let mut result = PasswordResult::None;

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
                            self.password_buffer.clear();

                            result = PasswordResult::Cancel;
                            return;
                        }

                        ui.add_space(10.0);

                        let submit =
                            ui.add_sized([100.0, 30.0], Button::new("OK")).clicked()
                                || ui.input(|i| i.key_pressed(Key::Enter));

                        if submit && !self.password_buffer.is_empty() {
                            // Забираем строку БЕЗ clone().
                            let password = std::mem::take(self.password_buffer);

                            // На месте старой строки остается пустая.
                            result = PasswordResult::Submit(
                                Zeroizing::new(password),
                            );
                        }
                    });
                });
            });

        if !open && matches!(result, PasswordResult::None) {
            self.password_buffer.zeroize();
            self.password_buffer.clear();

            return PasswordResult::Cancel;
        }

        result
    }
}