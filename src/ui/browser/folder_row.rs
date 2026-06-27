use egui::Ui;

pub struct FolderRow<'a> {
    name: &'a str,
    selected: bool,
    show_icon: bool,
}

impl<'a> FolderRow<'a> {
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            selected: false,
            show_icon: true,
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

    pub fn show(self, ui: &mut Ui) -> egui::Response {
        let text = if self.show_icon {
            format!("📁   {}", self.name)
        } else {
            self.name.to_owned()
        };
        ui.selectable_label(self.selected, text)
    }
}
