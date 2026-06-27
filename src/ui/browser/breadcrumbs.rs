use egui::{Align, Layout, RichText, Ui};

#[derive(Debug, Clone, PartialEq)]
pub enum BreadcrumbAction {
    None,
    Exit,
    NavigateTo(String),
}

pub struct BreadcrumbBar<'a> {
    current_vfs_dir: &'a str,
}

impl<'a> BreadcrumbBar<'a> {
    pub fn new(current_vfs_dir: &'a str) -> Self {
        Self { current_vfs_dir }
    }

    pub fn show(self, ui: &mut Ui) -> BreadcrumbAction {
        let mut action = BreadcrumbAction::None;

        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("📦").size(20.0));

            if ui.small_button("Root").clicked() {
                action = BreadcrumbAction::NavigateTo(String::new());
            }

            let mut accum = String::new();

            for component in self
                .current_vfs_dir
                .split('/')
                .filter(|s| !s.is_empty())
            {
                ui.label(RichText::new("›").weak().size(16.0));

                if accum.is_empty() {
                    accum.push_str(component);
                } else {
                    accum.push('/');
                    accum.push_str(component);
                }

                if ui.small_button(component).clicked() {
                    action = BreadcrumbAction::NavigateTo(accum.clone());
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Exit").clicked() {
                    action = BreadcrumbAction::Exit;
                }
            });
        });

        action
    }
}
