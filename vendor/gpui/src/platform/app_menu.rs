use crate::{Action, App, Platform, SharedString};
use util::ResultExt;

pub struct Menu {
    pub name: SharedString,

    pub items: Vec<MenuItem>,
}

impl Menu {
    pub fn owned(self) -> OwnedMenu {
        OwnedMenu {
            name: self.name.to_string().into(),
            items: self.items.into_iter().map(|item| item.owned()).collect(),
        }
    }
}

pub struct OsMenu {
    pub name: SharedString,

    pub menu_type: SystemMenuType,
}

impl OsMenu {
    pub fn owned(self) -> OwnedOsMenu {
        OwnedOsMenu {
            name: self.name.to_string().into(),
            menu_type: self.menu_type,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SystemMenuType {
    Services,
}

pub enum MenuItem {
    Separator,

    Submenu(Menu),

    SystemMenu(OsMenu),

    Action {
        name: SharedString,

        action: Box<dyn Action>,

        os_action: Option<OsAction>,
    },
}

impl MenuItem {
    pub fn separator() -> Self {
        Self::Separator
    }

    pub fn submenu(menu: Menu) -> Self {
        Self::Submenu(menu)
    }

    pub fn os_submenu(name: impl Into<SharedString>, menu_type: SystemMenuType) -> Self {
        Self::SystemMenu(OsMenu {
            name: name.into(),
            menu_type,
        })
    }

    pub fn action(name: impl Into<SharedString>, action: impl Action) -> Self {
        Self::Action {
            name: name.into(),
            action: Box::new(action),
            os_action: None,
        }
    }

    pub fn os_action(
        name: impl Into<SharedString>,
        action: impl Action,
        os_action: OsAction,
    ) -> Self {
        Self::Action {
            name: name.into(),
            action: Box::new(action),
            os_action: Some(os_action),
        }
    }

    pub fn owned(self) -> OwnedMenuItem {
        match self {
            MenuItem::Separator => OwnedMenuItem::Separator,
            MenuItem::Submenu(submenu) => OwnedMenuItem::Submenu(submenu.owned()),
            MenuItem::Action {
                name,
                action,
                os_action,
            } => OwnedMenuItem::Action {
                name: name.into(),
                action,
                os_action,
            },
            MenuItem::SystemMenu(os_menu) => OwnedMenuItem::SystemMenu(os_menu.owned()),
        }
    }
}

#[derive(Clone)]
pub struct OwnedOsMenu {
    pub name: SharedString,

    pub menu_type: SystemMenuType,
}

#[derive(Clone)]
pub struct OwnedMenu {
    pub name: SharedString,

    pub items: Vec<OwnedMenuItem>,
}

pub enum OwnedMenuItem {
    Separator,

    Submenu(OwnedMenu),

    SystemMenu(OwnedOsMenu),

    Action {
        name: String,

        action: Box<dyn Action>,

        os_action: Option<OsAction>,
    },
}

impl Clone for OwnedMenuItem {
    fn clone(&self) -> Self {
        match self {
            OwnedMenuItem::Separator => OwnedMenuItem::Separator,
            OwnedMenuItem::Submenu(submenu) => OwnedMenuItem::Submenu(submenu.clone()),
            OwnedMenuItem::Action {
                name,
                action,
                os_action,
            } => OwnedMenuItem::Action {
                name: name.clone(),
                action: action.boxed_clone(),
                os_action: *os_action,
            },
            OwnedMenuItem::SystemMenu(os_menu) => OwnedMenuItem::SystemMenu(os_menu.clone()),
        }
    }
}


#[derive(Copy, Clone, Eq, PartialEq)]
pub enum OsAction {
    Cut,

    Copy,

    Paste,

    SelectAll,

    Undo,

    Redo,
}

pub(crate) fn init_app_menus(platform: &dyn Platform, cx: &App) {
    platform.on_will_open_app_menu(Box::new({
        let cx = cx.to_async();
        move || {
            cx.update(|cx| cx.clear_pending_keystrokes()).ok();
        }
    }));

    platform.on_validate_app_menu_command(Box::new({
        let cx = cx.to_async();
        move |action| {
            cx.update(|cx| cx.is_action_available(action))
                .unwrap_or(false)
        }
    }));

    platform.on_app_menu_action(Box::new({
        let cx = cx.to_async();
        move |action| {
            cx.update(|cx| cx.dispatch_action(action)).log_err();
        }
    }));
}
