use iced::Settings;
use iced::keyboard;
use iced::time;
use iced::widget::{column, text, text_editor};
use iced::window;
use iced::window::Id;
use iced::window::Mode;
use iced::{Element, Fill, Font, Subscription, Task, Theme};

use chrono::Local;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

pub fn main() -> iced::Result {
    iced::application("Distraction-Free Editor", Editor::update, Editor::view)
        .theme(Editor::theme)
        .default_font(Font::MONOSPACE)
        .subscription(Editor::subscription)
        .settings(Settings::default())
        .window(window::Settings {
            size: iced::Size::new(800.0, 600.0),
            resizable: true,
            visible: false,
            decorations: false,
            transparent: false,
            ..Default::default()
        })
        .run_with(Editor::new)
}

#[derive(Debug)]
struct Editor {
    file: Option<PathBuf>,
    content: text_editor::Content,
    is_loading: bool,
    is_dirty: bool,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            file: None,
            content: text_editor::Content::new(),
            is_loading: true,
            is_dirty: false,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    ActionPerformed(text_editor::Action),
    FileCreated(Result<PathBuf, Error>),
    AutoSave,
    FileSaved(Result<PathBuf, Error>),
    WindowOpened(Id),
    WindowClosed,
    OpenPreviousFile,
    OpenNextFile,
    CreateNewFile,
    FileLoaded(Result<(PathBuf, String), Error>),
}

impl Editor {
    fn new() -> (Self, Task<Message>) {
        let now = Local::now();
        let filename = format!("{}.txt", now.format("%Y-%m-%d_%H-%M-%S"));
        let file_path = PathBuf::from(filename);

        let editor = Self {
            file: None,
            content: text_editor::Content::new(),
            is_loading: true,
            is_dirty: false,
        };

        let tasks = vec![
            Task::perform(create_new_file(file_path), Message::FileCreated),
            iced::widget::focus_next(),
        ];

        (editor, Task::batch(tasks))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ActionPerformed(action) => {
                self.is_dirty = self.is_dirty || action.is_edit();
                self.content.perform(action);
                Task::none()
            }
            Message::FileCreated(result) => {
                self.is_loading = false;
                if let Ok(path) = result {
                    self.file = Some(path);
                }
                Task::none()
            }
            Message::AutoSave => {
                if self.is_loading {
                    Task::none()
                } else {
                    let text = self.content.text();
                    Task::perform(save_file(self.file.clone(), text), Message::FileSaved)
                }
            }
            Message::FileSaved(result) => {
                self.is_loading = false;
                if let Ok(path) = result {
                    self.file = Some(path);
                    self.is_dirty = false;
                }
                Task::none()
            }
            Message::WindowClosed => {
                // Save before closing if there are unsaved changes
                if self.is_dirty && !self.is_loading {
                    let text = self.content.text();
                    Task::perform(save_file(self.file.clone(), text), Message::FileSaved)
                } else {
                    Task::none()
                }
            }
            Message::OpenPreviousFile => {
                if self.is_loading {
                    Task::none()
                } else {
                    // Save current file first if dirty
                    let save_task = if self.is_dirty {
                        let text = self.content.text();
                        Task::perform(save_file(self.file.clone(), text), Message::FileSaved)
                    } else {
                        Task::none()
                    };

                    let current_file = self.file.clone();
                    let load_task =
                        Task::perform(find_and_load_file(current_file, true), Message::FileLoaded);

                    Task::batch([save_task, load_task])
                }
            }
            Message::OpenNextFile => {
                if self.is_loading {
                    Task::none()
                } else {
                    // Save current file first if dirty
                    let save_task = if self.is_dirty {
                        let text = self.content.text();
                        Task::perform(save_file(self.file.clone(), text), Message::FileSaved)
                    } else {
                        Task::none()
                    };

                    let current_file = self.file.clone();
                    let load_task =
                        Task::perform(find_and_load_file(current_file, false), Message::FileLoaded);

                    Task::batch([save_task, load_task])
                }
            }
            Message::CreateNewFile => {
                if self.is_loading {
                    Task::none()
                } else {
                    // Save current file first if dirty
                    let save_task = if self.is_dirty {
                        let text = self.content.text();
                        Task::perform(save_file(self.file.clone(), text), Message::FileSaved)
                    } else {
                        Task::none()
                    };

                    let now = Local::now();
                    let filename = format!("{}.txt", now.format("%Y-%m-%d_%H-%M-%S"));
                    let file_path = PathBuf::from(filename);
                    let create_task =
                        Task::perform(create_new_file(file_path), Message::FileCreated);

                    // Clear current content
                    self.content = text_editor::Content::new();
                    self.is_dirty = false;

                    Task::batch([save_task, create_task])
                }
            }
            Message::FileLoaded(result) => {
                self.is_loading = false;
                if let Ok((path, contents)) = result {
                    self.file = Some(path);
                    self.content = text_editor::Content::with_text(&contents);
                    self.is_dirty = false;
                }
                Task::none()
            }
            Message::WindowOpened(id) => {
                Task::batch(vec![window::change_mode(id, Mode::Fullscreen)])
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let status_text = if let Some(path) = &self.file {
            format!(
                "File: {} | {}:{} | Cmd+L: Prev | Cmd+P: Next | Cmd+N: New | Cmd+S: Save | ESC: Exit",
                path.display(),
                self.content.cursor_position().0 + 1,
                self.content.cursor_position().1 + 1
            )
        } else {
            format!(
                "New file | {}:{} | Cmd+L: Prev | Cmd+P: Next | Cmd+N: New | Cmd+S: Save | ESC: Exit",
                self.content.cursor_position().0 + 1,
                self.content.cursor_position().1 + 1
            )
        };

        column![
            text_editor(&self.content)
                .height(Fill)
                .on_action(Message::ActionPerformed)
                .wrapping(text::Wrapping::Word)
                .key_binding(|key_press| {
                    match key_press.key.as_ref() {
                        keyboard::Key::Character("s") if key_press.modifiers.command() => {
                            Some(text_editor::Binding::Custom(Message::AutoSave))
                        }
                        keyboard::Key::Character("l") if key_press.modifiers.command() => {
                            Some(text_editor::Binding::Custom(Message::OpenPreviousFile))
                        }
                        keyboard::Key::Character("p") if key_press.modifiers.command() => {
                            Some(text_editor::Binding::Custom(Message::OpenNextFile))
                        }
                        keyboard::Key::Character("n") if key_press.modifiers.command() => {
                            Some(text_editor::Binding::Custom(Message::CreateNewFile))
                        }
                        keyboard::Key::Named(keyboard::key::Named::Escape) => {
                            Some(text_editor::Binding::Custom(Message::WindowClosed))
                        }
                        _ => text_editor::Binding::from_key_press(key_press),
                    }
                }),
            text(status_text),
        ]
        .spacing(10)
        .padding(10)
        .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark // Always dark theme for distraction-free
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            time::every(Duration::from_secs(10)).map(|_| Message::AutoSave),
            window::close_events().map(|_| Message::WindowClosed),
            window::open_events().map(Message::WindowOpened),
        ])
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    IoError(io::ErrorKind),
}

async fn create_new_file(path: PathBuf) -> Result<PathBuf, Error> {
    tokio::fs::write(&path, "")
        .await
        .map_err(|error| Error::IoError(error.kind()))?;
    Ok(path)
}

async fn save_file(path: Option<PathBuf>, contents: String) -> Result<PathBuf, Error> {
    let path = if let Some(path) = path {
        path
    } else {
        let now = Local::now();
        let filename = format!("{}.txt", now.format("%Y-%m-%d_%H-%M-%S"));
        PathBuf::from(filename)
    };

    tokio::fs::write(&path, contents)
        .await
        .map_err(|error| Error::IoError(error.kind()))?;
    Ok(path)
}

async fn find_and_load_file(
    current_file: Option<PathBuf>,
    find_previous: bool,
) -> Result<(PathBuf, String), Error> {
    use std::fs;

    // Get current directory
    let current_dir =
        std::env::current_dir().map_err(|_| Error::IoError(io::ErrorKind::NotFound))?;

    // Get all .txt files in current directory
    let mut txt_files: Vec<PathBuf> = fs::read_dir(&current_dir)
        .map_err(|_| Error::IoError(io::ErrorKind::NotFound))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("txt") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    // Sort files by name
    txt_files.sort();

    if txt_files.is_empty() {
        return Err(Error::IoError(io::ErrorKind::NotFound));
    }

    // Find current file index
    let current_index = if let Some(current) = current_file {
        txt_files.iter().position(|p| p == &current)
    } else {
        None
    };

    // Find target file
    let target_path = match current_index {
        Some(index) => {
            if find_previous {
                // Get previous file (wrap around to last if at first)
                if index == 0 {
                    if txt_files.len() > 1 {
                        txt_files[txt_files.len() - 1].clone()
                    } else {
                        return Err(Error::IoError(io::ErrorKind::NotFound));
                    }
                } else {
                    txt_files[index - 1].clone()
                }
            } else {
                // Get next file (wrap around to first if at last)
                if index >= txt_files.len() - 1 {
                    if txt_files.len() > 1 {
                        txt_files[0].clone()
                    } else {
                        return Err(Error::IoError(io::ErrorKind::NotFound));
                    }
                } else {
                    txt_files[index + 1].clone()
                }
            }
        }
        None => {
            // No current file, just pick the first one
            txt_files[0].clone()
        }
    };

    // Load the target file
    let contents = tokio::fs::read_to_string(&target_path)
        .await
        .map_err(|error| Error::IoError(error.kind()))?;

    Ok((target_path, contents))
}
