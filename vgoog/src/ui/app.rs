use crate::client::GoogleClient;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Gmail,
    Calendar,
    Drive,
    Sheets,
    Docs,
    Slides,
    Forms,
    Tasks,
    People,
    AppsScript,
}

impl Service {
    pub const ALL: [Service; 10] = [
        Service::Gmail,
        Service::Calendar,
        Service::Drive,
        Service::Sheets,
        Service::Docs,
        Service::Slides,
        Service::Forms,
        Service::Tasks,
        Service::People,
        Service::AppsScript,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Service::Gmail => "Gmail",
            Service::Calendar => "Calendar",
            Service::Drive => "Drive",
            Service::Sheets => "Sheets",
            Service::Docs => "Docs",
            Service::Slides => "Slides",
            Service::Forms => "Forms",
            Service::Tasks => "Tasks",
            Service::People => "Contacts",
            Service::AppsScript => "Apps Script",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Service::Gmail => "📧",
            Service::Calendar => "📅",
            Service::Drive => "📁",
            Service::Sheets => "📊",
            Service::Docs => "📄",
            Service::Slides => "📽",
            Service::Forms => "📝",
            Service::Tasks => "✅",
            Service::People => "👥",
            Service::AppsScript => "⚡",
        }
    }

    pub fn actions(&self) -> &'static [&'static str] {
        match self {
            Service::Gmail => &[
                "Inbox", "Search", "Compose", "Labels", "Drafts", "Threads",
                "Filters", "Settings", "Forwarding", "Send-As", "Delegates",
                "Unified Search",
            ],
            Service::Calendar => &[
                "Today", "Week View", "Events", "Quick Add", "Calendars",
                "Create Event", "ACL/Sharing", "Settings", "Free/Busy",
            ],
            Service::Drive => &[
                "My Files", "Search", "Upload", "Create Folder", "Shared",
                "Recent", "Starred", "Trash", "Storage Info", "Shared Drives",
            ],
            Service::Sheets => &[
                "Open Sheet", "Create Sheet", "Read Range", "Write Range",
                "Append Data", "Manage Sheets", "Named Ranges", "Sort",
            ],
            Service::Docs => &[
                "Open Doc", "Create Doc", "Insert Text", "Replace Text",
                "Formatting", "Headers/Footers", "Tables",
            ],
            Service::Slides => &[
                "Open Presentation", "Create Presentation", "Add Slide",
                "Edit Text", "Shapes", "Images", "Tables", "Notes",
            ],
            Service::Forms => &[
                "Open Form", "Create Form", "Add Question", "Responses",
                "Watches", "Settings", "Grid Questions",
            ],
            Service::Tasks => &[
                "Task Lists", "View Tasks", "Create Task", "Complete/Toggle",
                "Move Task", "Clear Completed",
            ],
            Service::People => &[
                "Contacts", "Search", "Create Contact", "Groups",
                "Other Contacts", "Directory",
            ],
            Service::AppsScript => &[
                "Projects", "Create Project", "Edit Code", "Versions",
                "Deployments", "Run Function", "Processes",
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    ServiceSelect,
    ActionSelect,
    ActionView,
    Input,
    Confirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputTarget {
    Search,
    Compose,
    Field(usize),
}

pub struct App {
    pub client: GoogleClient,
    pub screen: Screen,
    pub selected_service: usize,
    pub selected_action: usize,
    pub service: Option<Service>,

    // Data
    pub items: Vec<ListItem>,
    pub item_cursor: usize,
    pub detail: Option<Value>,
    pub status_message: String,
    pub loading: bool,

    // Input
    pub input_buffer: String,
    pub input_target: InputTarget,
    pub input_fields: Vec<InputField>,
    pub input_field_cursor: usize,

    // Pagination
    pub next_page_token: Option<String>,
    pub page_info: String,

    // Confirm dialog
    pub confirm_message: String,
    pub confirm_action: Option<Box<dyn std::any::Any + Send>>,

    pub should_quit: bool,
    pub scroll_offset: usize,

    // Account switcher
    pub account_name: String,
    pub account_label: String,
    pub account_list: Vec<String>,
    pub account_cursor: usize,
    pub show_account_switcher: bool,
}

#[derive(Debug, Clone)]
pub struct ListItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct InputField {
    pub label: String,
    pub value: String,
    pub placeholder: String,
    pub required: bool,
    pub multiline: bool,
}

impl InputField {
    pub fn new(label: &str, placeholder: &str, required: bool) -> Self {
        Self {
            label: label.to_string(),
            value: String::new(),
            placeholder: placeholder.to_string(),
            required,
            multiline: false,
        }
    }

    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }
}

impl App {
    pub fn new(client: GoogleClient) -> Self {
        Self {
            client,
            screen: Screen::ServiceSelect,
            selected_service: 0,
            selected_action: 0,
            service: None,
            items: Vec::new(),
            item_cursor: 0,
            detail: None,
            status_message: "Welcome to vgoog! Select a service.".to_string(),
            loading: false,
            input_buffer: String::new(),
            input_target: InputTarget::Search,
            input_fields: Vec::new(),
            input_field_cursor: 0,
            next_page_token: None,
            page_info: String::new(),
            should_quit: false,
            scroll_offset: 0,
            confirm_message: String::new(),
            confirm_action: None,
            account_name: String::new(),
            account_label: String::new(),
            account_list: Vec::new(),
            account_cursor: 0,
            show_account_switcher: false,
        }
    }

    pub fn current_service(&self) -> Service {
        Service::ALL[self.selected_service]
    }

    pub fn current_actions(&self) -> &'static [&'static str] {
        Service::ALL[self.selected_service].actions()
    }

    pub fn move_up(&mut self) {
        match self.screen {
            Screen::ServiceSelect => {
                if self.selected_service > 0 {
                    self.selected_service -= 1;
                }
            }
            Screen::ActionSelect => {
                if self.selected_action > 0 {
                    self.selected_action -= 1;
                }
            }
            Screen::ActionView => {
                if self.item_cursor > 0 {
                    self.item_cursor -= 1;
                }
            }
            Screen::Input => {
                if self.input_field_cursor > 0 {
                    self.input_field_cursor -= 1;
                }
            }
            Screen::Confirm => {}
        }
    }

    pub fn move_down(&mut self) {
        match self.screen {
            Screen::ServiceSelect => {
                if self.selected_service < Service::ALL.len() - 1 {
                    self.selected_service += 1;
                }
            }
            Screen::ActionSelect => {
                let max = self.current_actions().len().saturating_sub(1);
                if self.selected_action < max {
                    self.selected_action += 1;
                }
            }
            Screen::ActionView => {
                if self.item_cursor < self.items.len().saturating_sub(1) {
                    self.item_cursor += 1;
                }
            }
            Screen::Input => {
                if self.input_field_cursor < self.input_fields.len().saturating_sub(1) {
                    self.input_field_cursor += 1;
                }
            }
            Screen::Confirm => {}
        }
    }

    pub fn go_back(&mut self) {
        match self.screen {
            Screen::ServiceSelect => self.should_quit = true,
            Screen::ActionSelect => {
                self.screen = Screen::ServiceSelect;
                self.selected_action = 0;
            }
            Screen::ActionView => {
                self.screen = Screen::ActionSelect;
                self.items.clear();
                self.item_cursor = 0;
                self.detail = None;
                self.scroll_offset = 0;
            }
            Screen::Input => {
                self.screen = Screen::ActionSelect;
                self.input_fields.clear();
                self.input_field_cursor = 0;
            }
            Screen::Confirm => {
                self.screen = Screen::ActionView;
                self.confirm_action = None;
            }
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
    }

    pub fn set_items(&mut self, items: Vec<ListItem>) {
        self.items = items;
        self.item_cursor = 0;
        self.scroll_offset = 0;
    }

    pub fn scroll_detail_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_detail_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn current_item(&self) -> Option<&ListItem> {
        self.items.get(self.item_cursor)
    }
}
