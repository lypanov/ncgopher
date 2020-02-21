use cursive::Cursive;
use cursive::menu::MenuTree;
use cursive::views::{Dialog, SelectView, EditView, TextView, LinearLayout};
use cursive::utils::markup::StyledString;
use cursive::theme::Effect;
use cursive::event::Key; 
use cursive::traits::*;
use std::str;
use std::sync::{Arc, RwLock};
use std::sync::mpsc;
use url::Url;
use crate::controller::ControllerMessage;
use crate::gophermap::{GopherMapEntry, ItemType};
use crate::history::{HistoryEntry};
use crate::bookmarks::{Bookmark};
use crate::ui::layout::Layout;
use crate::ui::statusbar::StatusBar;
use crate::ui;

extern crate chrono;
extern crate url;
extern crate log;


/// Messages sent between Controller and UI
pub enum UiMessage {
    AddToBookmarkMenu(Bookmark),
    AddToHistoryMenu(HistoryEntry),
    BinaryWritten(String, usize),
    ClearHistoryMenu,
    OpenQueryDialog(Url),
    OpenQueryUrl(Url, String),
    OpenUrl(Url, ContentType),
    OpenURL(String),
    PageSaved(Url, ContentType, String), 
    ShowAddBookmarkDialog(Url),
    ShowContent(Url, String, ContentType),
    ShowMessage(String),
    ShowURLDialog,
    ShowSaveAsDialog(Url),
    ShowSearchDialog(Url),
}

#[derive(Clone)]
pub enum ContentType {
    Gophermap,
    Text,
    Binary
}


/// UserData is stored inside the cursive object (with set_user_data).
/// This makes the contained data available without the use of closures.
#[derive(Clone)]
pub struct UserData {
    pub ui_tx: Arc<RwLock<mpsc::Sender<UiMessage>>>,
    pub controller_tx: Arc<RwLock<mpsc::Sender<ControllerMessage>>>,
}

impl UserData {
    pub fn new(ui_tx: Arc<RwLock<mpsc::Sender<UiMessage>>>,
               controller_tx: Arc<RwLock<mpsc::Sender<ControllerMessage>>>) -> UserData {
        UserData {
            ui_tx,
            controller_tx,
        }
    }
}

/// Struct representing the visible part of NcGopher (=the UI).
#[derive(Clone)]
pub struct NcGopher {
    app: Arc<RwLock<Cursive>>,
    ui_rx: Arc<mpsc::Receiver<UiMessage>>,
    pub ui_tx: Arc<RwLock<mpsc::Sender<UiMessage>>>,
    pub controller_tx: Arc<RwLock<mpsc::Sender<ControllerMessage>>>,
    /// Message shown in statusbar
    message: Arc<RwLock<String>>,
}


impl NcGopher {
    pub fn new(cursive: Cursive, controller_tx: mpsc::Sender<ControllerMessage>) -> NcGopher {
        let (ui_tx, ui_rx) = mpsc::channel::<UiMessage>();
        let ncgopher = NcGopher {
            app: Arc::new(RwLock::new(cursive)),
            ui_tx: Arc::new(RwLock::new(ui_tx)),
            ui_rx: Arc::new(ui_rx),
            controller_tx: Arc::new(RwLock::new(controller_tx)),
            message: Arc::new(RwLock::new(String::new())),
        };
        // Make channels available from callbacks
        let userdata = UserData::new(ncgopher.ui_tx.clone(), ncgopher.controller_tx.clone());
        ncgopher.app.write().unwrap().set_user_data(userdata);

        ncgopher
    }

    /// Used by statusbar to get current message
    pub fn get_message(&self) -> String {
        return self.message.read().unwrap().clone();
    }

    /// Sets message for statusbar
    fn set_message(&mut self, msg: &str) {
        let mut message = self.message.write().unwrap();
        message.clear();
        message.push_str(msg);
        self.trigger();
    }

    /// Setup of UI, register global keys
    pub fn setup_ui(&mut self) {
        cursive::logger::init();
        info!("NcGopher::setup_ui()");
        self.create_menubar();
        let mut app = self.app.write().unwrap();

        app.set_autohide_menu(false);

        // TODO: Make keys configurable
        app.add_global_callback('q', |s| s.quit());
        app.add_global_callback('g', |app| {
            app.with_user_data(|userdata: &mut UserData|
                userdata.ui_tx.read().unwrap().clone().send(UiMessage::ShowURLDialog).unwrap()
            );
        });
        app.add_global_callback('b', |app| {
            app.with_user_data(|userdata: &mut UserData|
                userdata.controller_tx.read().unwrap().send(ControllerMessage::NavigateBack)
            );
        });
        app.add_global_callback('s', |app| {
            app.with_user_data(|userdata: &mut UserData|
                userdata.controller_tx.read().unwrap().clone().send(ControllerMessage::RequestSaveAsDialog).unwrap()
            );
        });
        app.add_global_callback(Key::Esc, |s| s.select_menubar());

        let view: SelectView<GopherMapEntry> = SelectView::new();
        let textview: SelectView = SelectView::new();
        let status = StatusBar::new(Arc::new(self.clone())).with_name("statusbar");
        let mut layout = Layout::new(status/*, theme*/)
            .view("text", textview.with_name("text").scrollable(), "Textfile")
            .view("content", view.with_name("content").scrollable(), "Gophermap");
        layout.set_view("content");
        app.add_fullscreen_layer(layout.with_name("main"));

        app.add_global_callback('~', Cursive::toggle_debug_console);
    }


    fn fetch_binary_file(&mut self, url: Url, local_path: String) {
        let filename = self.get_filename_from_url(url.clone());
        let path = format!("{}/{}", local_path, filename);
        self.controller_tx.read().unwrap()
            .send(ControllerMessage::FetchBinaryUrl(url, path)).unwrap();
    }


    fn get_filename_from_url(&mut self, url: Url) -> String {
        let mut segments = url.path_segments().map(|c| c.collect::<Vec<_>>()).unwrap();
        let last_seg = segments.pop();
        match last_seg {
            Some(filename) => return filename.to_string(),
            None => ()
        }
        "download.bin".to_string()
    }

    fn binary_written(&mut self, filename: String, bytes: usize) {
        self.set_message(format!("File downloaded: {} ({} bytes)", filename, bytes).as_str());
    }

    pub fn create_menubar(&mut self) {
        let mut app = self.app.write().unwrap();
        let menubar = app.menubar();
        menubar.add_subtree(
                "File",
                MenuTree::new()
                    .leaf("Open URL...", |app| {
                        app.with_user_data(|userdata: &mut UserData|
                             userdata.ui_tx.read().unwrap().clone().send(UiMessage::ShowURLDialog).unwrap()
                        );
                })
                .delimiter()
                .leaf("Save page as...", |app|{
                    app.with_user_data(|userdata: &mut UserData|
                        userdata.controller_tx.read().unwrap().clone().send(ControllerMessage::RequestSaveAsDialog).unwrap()
                    );
                })
                .leaf("Settings...", |s| {
                    s.add_layer(Dialog::info("Settings not implemented"))
                })
                .delimiter()
                .leaf("Quit", |s| s.quit())
        );
        menubar.add_subtree(
            "History",
            MenuTree::new()
                .leaf("Show all history...", |s| {
                    s.add_layer(Dialog::info("Show history not implemented"))
                }).
                leaf("Clear history", |app| {
                    app.add_layer(Dialog::around(TextView::new("Do you want to delete the history?"))
                        .button("Cancel", |app| { app.pop_layer();})
                        .button("Ok", |app| {
                            app.pop_layer();
                            app.with_user_data(|userdata: &mut UserData| {
                                userdata.controller_tx.read().unwrap().send(ControllerMessage::ClearHistory).unwrap()
                            });

                        })
                    );
                })
                .delimiter()
        );
        menubar.add_subtree(
            "Bookmarks",
            MenuTree::new()
                .leaf("Edit...", |s| {
                    s.add_layer(Dialog::info("Edit bookmarks not implemented"))
                }).
                leaf("Add bookmark", |app| {
                    //app.add_layer(Dialog::info("Add bookmark not implemented"))
                    app.with_user_data(|userdata: &mut UserData|
                        userdata.controller_tx.read().unwrap().clone().send(ControllerMessage::RequestAddBookmarkDialog).unwrap()
                    );
                })
                .delimiter()
        );
        menubar.add_subtree(
            "Search",
            MenuTree::new()
                .leaf("Veronica/2...", |app| {
                    let url = Url::parse("gopher://gopher.floodgap.com:70/v2/vs").unwrap();
                    app.with_user_data(|userdata: &mut UserData|
                        userdata.ui_tx.read().unwrap().send(UiMessage::ShowSearchDialog(url)).unwrap()
                    );
                }).
                leaf("Gopherpedia...", |app| {
                    // FIXME Add Url to gopherpedia
                    let url = Url::parse("gopher://gopher.floodgap.com:70/v2/vs").unwrap();
                    app.with_user_data(|userdata: &mut UserData|
                        userdata.ui_tx.read().unwrap().send(UiMessage::ShowSearchDialog(url)).unwrap()
                    );
                })
                .leaf("Gopher Movie Database...", |app| {
                    let url = Url::parse("gopher://jan.bio:70/cgi-bin/gmdb.py").unwrap();
                    app.with_user_data(|userdata: &mut UserData|
                        userdata.ui_tx.read().unwrap().send(UiMessage::ShowSearchDialog(url)).unwrap()
                    );
                })
        );
        menubar.add_subtree(
            "Help",
            MenuTree::new()
                .subtree(
                    "Help",
                    MenuTree::new()
                        .leaf("General", |s| {
                            s.add_layer(Dialog::info("Help message!"))
                        })
                        .leaf("Online", |s| {
                            let text = "Google it yourself!\n\
                                        Kids, these days...";
                            s.add_layer(Dialog::info(text))
                        }),
                )
                .leaf("About", |s| {
                    s.add_layer(Dialog::info(format!(
                        ";               ncgopher v{}\n\
                         ;     A Gopher client for the modern internet\n\
                         ; (c) 2019-2020 by Jan Schreiber <jan@mecinus.com>\n\
                         ;               gopher://jan.bio", env!("CARGO_PKG_VERSION"))))
                }),
        );
    }

    pub fn open_gopher_url_string(&mut self, url: String) {
        // TODO: Allow other types of Urls
        let mut url = url;
        if !url.starts_with("gopher://") {
            url.insert_str(0, "gopher://");
        }
        let res = Url::parse(url.as_str());
        let url : Url;
        match res {
            Ok(res) => {
                url = res;
                self.open_gopher_address(url, ContentType::Gophermap);
            },
            Err(e) => {
                self.set_message(format!("Invalid URL: {}", e).as_str());
            }
        }
    }

    pub fn open_gopher_url(&mut self, url: Url) {
        self.open_gopher_url_string(url.to_string());
    }

    pub fn open_gopher_address(&mut self, url: Url, content_type: ContentType) {
        self.set_message("Loading ...");
        let mut app = self.app.write().unwrap();
        app.call_on_name("main", |v: &mut ui::layout::Layout| {
            v.set_view("content");
        });
        self.controller_tx.read().unwrap()
            .send(ControllerMessage::FetchUrl(url, content_type, String::new())).unwrap();
    }

    fn open_query_dialog(&mut self, url: Url) {
        {
            let mut app = self.app.write().unwrap();
            app.add_layer(
                Dialog::new()
                    .title("Enter query:")
                    .content(
                        EditView::new()
                        // Call `show_popup` when the user presses `Enter`
                        //FIXME: create closure with url: .on_submit(search)
                            .with_name("query")
                            .fixed_width(30),
                    )
                    .button("Ok", move |app| {
                        let name = app.call_on_name("query", |view: &mut EditView| {
                            view.get_content()
                        });
                        if let Some(n) = name {
                            app.pop_layer(); // Close search dialog
                            app.with_user_data(|userdata: &mut UserData| {
                                userdata.ui_tx.write().unwrap().send(
                                    UiMessage::OpenQueryUrl(url.clone(), n.to_string()))
                                    .unwrap();
                            });
                        } else {
                            app.pop_layer();
                        }
                    }),
            );
        }
        self.trigger();
    }

    fn query(&mut self, url: Url, query: String) {
        self.set_message("Loading ...");
        self.controller_tx.read().unwrap()
            .send(ControllerMessage::FetchUrl(url, ContentType::Gophermap, query)).unwrap();
    }

    /// Renders a gophermap in a cursive::TextView
    fn show_gophermap(&mut self, content: String) {
        let mut title : String = "".to_string();
        let mut app = self.app.write().unwrap();
        app.call_on_name("content", |view: &mut SelectView<GopherMapEntry>| {
            view.clear();
            let lines = content.lines();
            let mut gophermap = Vec::new();
            let mut first = true;
            for l in lines {
                if first {
                    if l.starts_with('/')  {
                        title = l.to_string();
                    }
                    first = false;
                }
                if l != "." {
                    let gophermap_line = GopherMapEntry::parse(l.to_string());
                    gophermap.push(gophermap_line);
                }
            }
            for l in gophermap {
                let entry = l.clone();
                match entry.item_type {
                    ItemType::Dir => {
                        let mut formatted = StyledString::new();
                        let dir_label = format!("[MAP]  {}", entry.label());
                        formatted.append(StyledString::styled(dir_label, Effect::Italic));
                        view.add_item(formatted, l.clone());
                    }
                    ItemType::File => {
                        let mut formatted = StyledString::new();
                        let file_label = format!("[FILE] {}", entry.label());
                        formatted.append(StyledString::styled(file_label, Effect::Italic));
                        view.add_item(formatted, l.clone());
                    }
                    ItemType::Binary => {
                        let mut formatted = StyledString::new();
                        let bin_label = format!("[BIN]  {}", entry.label());
                        formatted.append(StyledString::styled(bin_label, Effect::Bold));
                        view.add_item(formatted, l.clone());
                    }
                    ItemType::Gif => {
                        let mut formatted = StyledString::new();
                        let gif_label = format!("[GIF]  {}", entry.label());
                        formatted.append(StyledString::styled(gif_label, Effect::Bold));
                        view.add_item(formatted, l.clone());
                    }
                    ItemType::Html => {
                        let mut formatted = StyledString::new();
                        let www_label = format!("[WWW]  {}", entry.label());
                        formatted.append(StyledString::styled(www_label, Effect::Italic));
                        view.add_item(formatted, l.clone());
                    }
                    ItemType::IndexServer => {
                        let mut formatted = StyledString::new();
                        let query_label = format!("[QRY]  {}", entry.label());
                        formatted.append(StyledString::styled(query_label, Effect::Italic));
                        view.add_item(formatted, l.clone());
                    }
                    ItemType::Telnet => {
                        let mut formatted = StyledString::new();
                        let telnet_label = format!("[TEL]  {}", entry.label());
                        formatted.append(StyledString::styled(telnet_label, Effect::Italic));
                        view.add_item(formatted, l.clone());
                    }
                    ItemType::Image => {
                        let mut formatted = StyledString::new();
                        let gif_label = format!("[IMG]  {}", entry.label());
                        formatted.append(StyledString::styled(gif_label, Effect::Bold));
                        view.add_item(formatted, l.clone());
                    }
                    /*ItemType::CsoServer => '2',
                    ItemType::Error => '3',
                    ItemType::BinHex => '4',
                    ItemType::Dos => '5',
                    ItemType::Uuencoded => '6',
                    ItemType::Telnet => '8',
                    ItemType::RedundantServer => '+',
                    ItemType::Tn3270 => 'T',
                     */
                    _ => {
                        let info_label = format!("       {}", entry.label());
                        view.add_item(info_label, l.clone());
                    }
                }
            }
            view.set_on_submit(|app, entry| {
                app.with_user_data(|userdata: &mut UserData| {
                    match entry.item_type {
                        ItemType::Dir => {
                            userdata.ui_tx.write().unwrap().send(
                                UiMessage::OpenUrl(entry.url.clone(), ContentType::Gophermap))
                                .unwrap();
                        }
                        ItemType::File => {
                            userdata.ui_tx.write().unwrap().send(
                                UiMessage::OpenUrl(entry.url.clone(), ContentType::Text))
                                .unwrap();
                        }
                        ItemType::Binary | ItemType::BinHex | ItemType::Dos | ItemType::Image=> {
                            userdata.ui_tx.write().unwrap().send(
                                UiMessage::OpenUrl(entry.url.clone(), ContentType::Binary))
                                .unwrap();
                        }
                        ItemType::IndexServer => {
                            userdata.ui_tx.write().unwrap().send(
                                UiMessage::OpenQueryDialog(entry.url.clone()))
                                .unwrap();
                        }
                        _ => {
                            
                        }
                    }
                });
            });
        });

        // FIXME: Call this from the previous callback
        if !title.is_empty() {
            app.call_on_name("main", |v: &mut ui::layout::Layout| {
                v.set_title("content".to_string(), title);
            });
        }
    }

    /// Renders a text file in a cursive::TextView
    fn show_text_file(&mut self, content: String) {
        let mut app = self.app.write().unwrap();
        app.call_on_name("main", |v: &mut ui::layout::Layout| {
            v.set_view("text");
        });
        app.call_on_name("text", |v: &mut SelectView| {
            v.clear();
            let lines = content.lines();
            for l in lines {
                v.add_item_str(format!("  {}", l.to_string()));
            }
            // TODO: on_submit-handler to open URLs in text
        });
    }

    fn show_add_bookmark_dialog(&mut self, url: Url) {
        {
            let mut app = self.app.write().unwrap();
            let newurl = url.clone();
            app.add_layer(
                Dialog::new()
                    .title("Add Bookmark")
                    .content(
                        LinearLayout::vertical()
                            .child(TextView::new("URL:"))
                            .child(TextView::new(newurl.clone().into_string().as_str()))
                            .child(TextView::new("\nTitle:"))
                            .child(EditView::new().with_name("title").fixed_width(30))
                            .child(TextView::new("Tags (comma separated):"))
                            .child(EditView::new().with_name("tags").fixed_width(30)
                                )
                    )
                    .button("Ok", move |app| {
                        let sometitle = app.call_on_name("title", |view: &mut EditView| {
                            view.get_content()
                        });
                        let sometags = app.call_on_name("tags", |view: &mut EditView| {
                            view.get_content()
                        });
                        app.pop_layer(); // Close edit bookmark
                        let title: String;
                        let tags: String;
                        if let Some(n) = sometitle {
                            title = n.to_string();
                        } else {
                            title = String::new()
                        }
                        if let Some(n) = sometags {
                            tags = n.to_string();
                        } else {
                            tags = String::new()
                        }
                        app.with_user_data(|userdata: &mut UserData|
                                           userdata.controller_tx.read().unwrap().clone().send(
                                               ControllerMessage::AddBookmark(newurl.clone(),
                                                   title.to_string(), tags.to_string()))
                                           .unwrap()
                                           );
                    })
                    .button("Cancel", |app| {
                        app.pop_layer(); // Close edit bookmark
                    })
            );
        }
        self.trigger();
    }

    fn show_search_dialog(&mut self, url: Url) {
        let ui_tx_clone = self.ui_tx.read().unwrap().clone();
        {
            let mut app = self.app.write().unwrap();
            app.add_layer(
                Dialog::new()
                    .title("Enter search term")
                    .content(
                        EditView::new()
                        // Call `show_popup` when the user presses `Enter`
                        //FIXME: create closure with url: .on_submit(search)
                            .with_name("search")
                            .fixed_width(30),
                    )
                    .button("Ok", move |app| {
                        let name = app.call_on_name("search", |view: &mut EditView| {
                            view.get_content()
                        });
                        if let Some(n) = name {
                            app.pop_layer();
                            ui_tx_clone.send(
                                UiMessage::OpenQueryUrl(url.clone(), n.to_string()))
                                .unwrap();
                        } else {
                            app.pop_layer(); // Close search dialog
                            app.add_layer(Dialog::info("No search parameter!"))
                        }
                    }),
            );
        }
        self.trigger();
    }


    pub fn show_url_dialog(&mut self) {
        {
            let mut app = self.app.write().unwrap();
            app.add_layer(
                Dialog::new()
                    .title("Enter gopher URL:")
                    .content(
                        EditView::new()
                            .on_submit(NcGopher::open_url_action)
                            .with_name("name")
                            .fixed_width(50)
                    )
                    .button("Cancel", move |app| {
                        app.pop_layer();
                    })
                    .button("Ok", |app| {
                        let name = app.call_on_name("name", |view: &mut EditView| {
                            view.get_content()
                        }).unwrap();
                        NcGopher::open_url_action(app, name.as_str())
                    })
            );
        } // drop lock on app before calling trigger:
        self.trigger();
    }

    fn open_url_action(app: &mut Cursive, name: &str) {
        app.pop_layer();
        app.with_user_data(|userdata: &mut UserData|
            userdata.ui_tx.read().unwrap()
            .send(UiMessage::OpenURL(name.to_string())).unwrap()
        );
    }


    fn show_save_as_dialog(&mut self, url: Url) {
        {
            let mut filename = self.get_filename_from_url(url.clone());
            if filename.is_empty() {
                filename.push_str("download");
            }
            if !filename.ends_with(".txt") {
                filename.push_str(".txt");
            }
            let mut app = self.app.write().unwrap();
            app.add_layer(
                Dialog::new()
                    .title("Enter filename:")
                    .content(
                        EditView::new()
                            .on_submit(NcGopher::save_as_action)
                            .with_name("name")
                            .fixed_width(50),
                    )
                    .button("Cancel", move |app| {
                        app.pop_layer();
                    })
                    .button("Ok", move |app| {
                        let name = app.call_on_name("name", |view: &mut EditView| {
                            view.get_content()
                        }).unwrap();
                        NcGopher::save_as_action(app, name.as_str())
                    }),
            );
            app.call_on_name("name", |v: &mut EditView| {
                v.set_content(filename);
            });
        }
        self.trigger();
    }

    fn save_as_action(app: &mut Cursive, name: &str) {
        app.pop_layer();
        if !name.is_empty() {
            app.with_user_data(|userdata: &mut UserData|
                               userdata.controller_tx.read().unwrap()
                               .send(ControllerMessage::SavePageAs(name.to_string())).unwrap()
            );
        } else {
            app.add_layer(Dialog::info("No filename given!"))
        }
    }

    fn add_to_bookmark_menu(&mut self, b: Bookmark) {
        info!("add_to_bookmark_menu()");
        let mut app = self.app.write().unwrap();
        let menutree = app.menubar().find_subtree("Bookmarks");
        if let Some(tree) = menutree {
            let b2 = b.clone();
            tree.insert_leaf(3, b.title.as_str(), move |app| {
                info!("Adding bm to bookmark menu");
                let b3 = b2.clone();
                app.with_user_data(|userdata: &mut UserData|
                    userdata.ui_tx.read().unwrap().clone().send(UiMessage::OpenURL(b3.url.to_string())).unwrap()
                );
            });
        }
    }


    fn add_to_history_menu(&mut self, h: HistoryEntry) {
        const HISTORY_LEN: usize = 10;
        let mut app = self.app.write().unwrap();
        let menutree = app.menubar().find_subtree("History");
        if let Some(tree) = menutree {
            // Add 3 to account for the two first menuitems + separator
            if tree.len() > HISTORY_LEN + 3 {
                tree.remove(tree.len() - 1);
            }
            // TODO: Refactor.
            // There must be a more ideomatic way than h->h2->h3
            let h2 = h.clone();
            tree.insert_leaf(3, h.title.as_str(), move |app| {
                let h3 = h2.clone();
                app.with_user_data(|userdata: &mut UserData|
                    userdata.ui_tx.read().unwrap().clone().send(UiMessage::OpenURL(h3.url.to_string())).unwrap()
                );
            });
        }
    }


    fn clear_history_menu(&mut self) {
        let mut app = self.app.write().unwrap();
        let menutree = app.menubar().find_subtree("History");
        if let Some(tree) = menutree {
            while tree.len() > 3 {
                tree.remove(tree.len() - 1);
            }
        }
    }

    /// Triggers a rerendring of the UI
    pub fn trigger(&self) {
        // send a no-op to trigger event loop processing
        let app = self.app.read().unwrap();
        app.cb_sink()
            .send(Box::new(Cursive::noop))
            .expect("could not send no-op event to cursive");
    }

    /// Step the UI by calling into Cursive's step function, then
    /// processing any UI messages.
    pub fn step(&mut self) -> bool {
        {
            let app = self.app.write().unwrap();
            if !app.is_running() {
                return false;
            }
        }

        // Process any pending UI messages
        while let Some(message) = self.ui_rx.try_iter().next() {
            match message {
                UiMessage::AddToBookmarkMenu(bookmark) => {
                    self.add_to_bookmark_menu(bookmark);
                },
                UiMessage::AddToHistoryMenu(history_entry) => {
                    self.add_to_history_menu(history_entry);
                },
                UiMessage::BinaryWritten(filename, bytes_written) => {
                    self.binary_written(filename, bytes_written);
                },
                UiMessage::ClearHistoryMenu => {
                    self.clear_history_menu();
                },
                UiMessage::PageSaved(_url, _content_type, filename) => {
                    self.set_message(format!("Page saved as '{}'.", filename).as_str());
                },
                UiMessage::ShowAddBookmarkDialog(url) => {
                    self.show_add_bookmark_dialog(url);
                },
                UiMessage::ShowContent(url, content, content_type) => {
                    match content_type {
                        ContentType::Gophermap => self.show_gophermap(content),
                        ContentType::Text => self.show_text_file(content),
                        ContentType::Binary => (),
                    }
                    self.set_message(url.as_str());
                },
                UiMessage::OpenQueryDialog(url) => {
                    self.open_query_dialog(url);
                },
                UiMessage::OpenQueryUrl(url, query) => {
                    self.query(url, query);
                },
                UiMessage::OpenUrl(url, content_type) => {
                    match content_type {
                        ContentType::Binary => {
                            match dirs::home_dir() {
                                Some(dir) => {
                                    self.fetch_binary_file(url, dir.into_os_string().into_string().unwrap());
                                },
                                None => {
                                    self.set_message("Could not find download dir");
                                }
                            };
                        },
                        _ => {
                            self.open_gopher_address(url, content_type);
                        }
                    }
                },
                UiMessage::OpenURL(url) => {
                    self.open_gopher_url_string(url);
                },
                UiMessage::ShowMessage(msg) => {
                    self.set_message(msg.as_str());
                },
                UiMessage::ShowURLDialog => {
                    self.show_url_dialog();
                },
                UiMessage::ShowSaveAsDialog(url) => {
                    self.show_save_as_dialog(url);
                },
                UiMessage::ShowSearchDialog(url) => {
                    self.show_search_dialog(url);
                }
            }
        }

        // Step the UI
        let mut app = self.app.write().unwrap();
        app.step();

        true
    }
}