use crate::config;
use crate::webserver::{self, Webserver};
use muda::{CheckMenuItem, MenuId, MenuItem, Submenu};
use std::process::Child;

/// One tray-managed web server with its submenu and the process it spawned.
pub struct WebserverSlot {
    server: Webserver,
    child: Option<Child>,
    submenu: Submenu,
    enabled_item: CheckMenuItem,
    restart_id: MenuId,
    open_id: MenuId,
    logs_id: MenuId,
}

impl WebserverSlot {
    pub fn new(server: Webserver) -> Self {
        let submenu = Submenu::new(server.menu_label(), true);
        let enabled_item = CheckMenuItem::new("Enabled", true, server.enabled, None);
        let restart_item = MenuItem::new("Restart", true, None);
        let open_item = MenuItem::new("Open in browser", true, None);
        let logs_item = MenuItem::new("View logs", true, None);
        submenu
            .append(&enabled_item)
            .expect("Failed to add webserver item");
        submenu
            .append(&restart_item)
            .expect("Failed to add webserver item");
        submenu
            .append(&open_item)
            .expect("Failed to add webserver item");
        submenu
            .append(&logs_item)
            .expect("Failed to add webserver item");
        Self {
            server,
            child: None,
            submenu,
            enabled_item,
            restart_id: restart_item.id().clone(),
            open_id: open_item.id().clone(),
            logs_id: logs_item.id().clone(),
        }
    }

    pub fn submenu(&self) -> &Submenu {
        &self.submenu
    }

    /// Replace any instance left over from a previous tray run.
    pub fn start_if_enabled(&mut self) {
        if self.server.enabled {
            webserver::stop_webserver(None, &self.server);
            self.spawn();
        }
    }

    fn spawn(&mut self) -> bool {
        match webserver::spawn_webserver(&self.server) {
            Ok(child) => {
                self.child = Some(child);
                true
            }
            Err(e) => {
                eprintln!("Failed to start webserver {}: {e}", self.server.name);
                false
            }
        }
    }

    pub fn stop(&mut self) {
        webserver::stop_webserver(self.child.take(), &self.server);
    }

    /// Handle a menu event aimed at this server; returns whether it was one.
    pub fn handle(&mut self, id: &MenuId) -> bool {
        if id == self.enabled_item.id() {
            let enabled = config::toggle_webserver_enabled(&self.server.source);
            self.enabled_item.set_checked(enabled);
            if enabled {
                self.spawn();
            } else {
                self.stop();
            }
        } else if id == &self.restart_id {
            self.stop();
            if self.spawn() {
                self.enabled_item.set_checked(true);
            }
        } else if id == &self.open_id {
            webserver::open_webserver_url(self.server.port);
        } else if id == &self.logs_id {
            webserver::launch_log_tail(&self.server);
        } else {
            return false;
        }
        true
    }
}
