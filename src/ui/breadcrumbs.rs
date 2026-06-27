use egui::{Align, Layout, RichText, Ui};

#[derive(Debug, Clone)]
pub enum BreadcrumbAction {
    None,
    Exit,
    NavigateTo(String),
}

pub fn render_breadcrumb_bar(
    ui: &mut Ui,
    current_vfs_dir: &str,
) -> BreadcrumbAction {
    let mut action = BreadcrumbAction::None;

    ui.horizontal_wrapped(|ui| {
        // Иконка хранилища
        ui.label(RichText::new("📦").size(20.0));

        // Root
        if ui.small_button("Root").clicked() {
            action = BreadcrumbAction::NavigateTo(String::new());
        }

        // Хлебные крошки
        let mut accum = String::new();

        for component in current_vfs_dir
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

        // Кнопка выхода справа
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Exit").clicked() {
                action = BreadcrumbAction::Exit;
            }
        });
    });

    action
}