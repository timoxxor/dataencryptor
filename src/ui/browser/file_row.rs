use std::path::Path;

use egui::{Id, Key, TextEdit, Ui};
use egui::text::{CCursor, CCursorRange};

#[derive(Debug, Clone, PartialEq)]
pub enum RenameAction {
    Submit(String),
    Cancel,
}

pub struct FileRow<'a> {
    name: &'a str,
    selected: bool,
    show_icon: bool,
    is_renaming: bool,
    rename_buffer: &'a mut String,
}

impl<'a> FileRow<'a> {
    pub fn new(name: &'a str, rename_buffer: &'a mut String) -> Self {
        Self {
            name,
            selected: false,
            show_icon: true,
            is_renaming: false,
            rename_buffer,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn show_icon(mut self, show: bool) -> Self {
        self.show_icon = show;
        self
    }

    pub fn renaming(mut self, active: bool) -> Self {
        self.is_renaming = active;
        self
    }

    pub fn show(self, ui: &mut Ui) -> (egui::Response, Option<RenameAction>) {
        if self.is_renaming {
            let stem_len = Path::new(self.rename_buffer.as_str())
                .file_stem()
                .map(|s| s.len())
                .unwrap_or(self.rename_buffer.len());

            let selection_key = Id::new("rename_sel_done").with(self.name);
            let needs_sel = !ui.ctx().data_mut(|d| {
                d.get_persisted::<bool>(selection_key).unwrap_or(false)
            });

            let focus_key = Id::new("rename_focus").with(self.name);
            let had_focus = ui.ctx().data_mut(|d| {
                d.get_persisted::<bool>(focus_key).unwrap_or(false)
            });

            let inner = ui.horizontal(|ui| {
                if self.show_icon {
                    ui.label("📄");
                }
                let te = TextEdit::singleline(self.rename_buffer);
                let mut output = te.show(ui);

                if needs_sel {
                    output.response.request_focus();
                    if stem_len > 0 {
                        let sel_end = if stem_len < self.rename_buffer.len() {
                            stem_len
                        } else {
                            self.rename_buffer.len()
                        };
                        let range = CCursorRange::two(CCursor::new(0), CCursor::new(sel_end));
                        output.state.cursor.set_char_range(Some(range));
                        output.state.store(ui.ctx(), output.response.id);
                    }
                    ui.ctx().data_mut(|d| d.insert_persisted(selection_key, true));
                }

                let has_focus = output.response.has_focus();
                ui.ctx().data_mut(|d| d.insert_persisted(focus_key, has_focus));

                let submit = ui.input(|i| i.key_pressed(Key::Enter));
                let lost_focus = had_focus && !has_focus;

                if submit {
                    if self.rename_buffer.as_str() == self.name {
                        Some(RenameAction::Cancel)
                    } else {
                        Some(RenameAction::Submit(self.rename_buffer.clone()))
                    }
                } else if ui.input(|i| i.key_pressed(Key::Escape)) || lost_focus {
                    Some(RenameAction::Cancel)
                } else {
                    None
                }
            });
            (inner.response, inner.inner)
        } else {
            let text = if self.show_icon {
                format!("📄   {}", self.name)
            } else {
                self.name.to_owned()
            };
            let resp = ui.selectable_label(self.selected, text);
            (resp, None)
        }
    }
}
