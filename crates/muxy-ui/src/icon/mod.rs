#[cfg(target_os = "macos")]
mod raster;
#[cfg(target_os = "macos")]
mod sfsymbol;

#[cfg(target_os = "macos")]
pub use raster::{tinted, tinted_symbol};

use gpui::SharedString;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    AppWindow,
    Archive,
    ArrowUpDown,
    Bell,
    BellDot,
    Bug,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    Check,
    CircleX,
    Code,
    Columns,
    Cpu,
    Folder,
    GitBranch,
    Globe,
    Grid,
    House,
    Eye,
    LayoutSplit,
    Lightbulb,
    Maximize,
    Network,
    Palette,
    PanelLeft,
    Pin,
    Plus,
    Puzzle,
    Restore,
    Rows,
    Search,
    Settings,
    Terminal,
    Trash,
    X,
}

impl Icon {
    pub fn path(self) -> SharedString {
        let name = match self {
            Self::AppWindow => "app-window",
            Self::Archive => "archive",
            Self::ArrowUpDown => "arrow-up-down",
            Self::Bell => "bell",
            Self::BellDot => "bell-dot",
            Self::Bug => "bug",
            Self::ChevronDown => "chevron-down",
            Self::ChevronLeft => "chevron-left",
            Self::ChevronRight => "chevron-right",
            Self::Check => "check",
            Self::CircleX => "circle-x",
            Self::Code => "code",
            Self::Columns => "columns-2",
            Self::Cpu => "cpu",
            Self::Folder => "folder",
            Self::GitBranch => "git-branch",
            Self::Globe => "globe",
            Self::Grid => "grid-2x2",
            Self::LayoutSplit => "layout-split-2x2",
            Self::House => "house",
            Self::Eye => "eye",
            Self::Lightbulb => "lightbulb",
            Self::Maximize => "maximize",
            Self::Network => "network",
            Self::Palette => "palette",
            Self::PanelLeft => "panel-left",
            Self::Pin => "pin",
            Self::Plus => "plus",
            Self::Puzzle => "puzzle",
            Self::Restore => "restore",
            Self::Rows => "rows-2",
            Self::Search => "search",
            Self::Settings => "settings",
            Self::Terminal => "terminal",
            Self::Trash => "trash",
            Self::X => "x",
        };
        SharedString::from(format!("icons/{name}.svg"))
    }

    pub fn from_symbol(symbol: &str) -> Option<Self> {
        Some(match symbol {
            "house.fill" | "house" => Self::House,
            "folder" => Self::Folder,
            "arrow.triangle.branch" => Self::GitBranch,
            "network" => Self::Network,
            "bell" => Self::Bell,
            "puzzlepiece.extension" => Self::Puzzle,
            "paintpalette" => Self::Palette,
            "lightbulb" => Self::Lightbulb,
            "magnifyingglass" => Self::Search,
            "gearshape" => Self::Settings,
            "plus" => Self::Plus,
            "pin.fill" | "pin" => Self::Pin,
            "terminal" => Self::Terminal,
            "archivebox" => Self::Archive,
            "eye" => Self::Eye,
            "trash" => Self::Trash,
            "xmark" => Self::X,
            "arrow.up.left.and.arrow.down.right" => Self::Maximize,
            "arrow.down.right.and.arrow.up.left" => Self::Restore,
            "cpu" => Self::Cpu,
            "ladybug" => Self::Bug,
            "square.grid.2x2" => Self::Grid,
            "square.split.2x1" => Self::Columns,
            "square.split.1x2" => Self::Rows,
            "globe" => Self::Globe,
            "sidebar.left" => Self::PanelLeft,
            "arrow.up.arrow.down" => Self::ArrowUpDown,
            "macwindow.badge.plus" => Self::AppWindow,
            "chevron.down" => Self::ChevronDown,
            "chevron.left" => Self::ChevronLeft,
            "chevron.right" => Self::ChevronRight,
            "checkmark" => Self::Check,
            _ => return None,
        })
    }
}

#[cfg(target_os = "macos")]
impl Icon {
    pub fn sf_symbol(self) -> &'static str {
        match self {
            Self::AppWindow => "macwindow.badge.plus",
            Self::Archive => "archivebox",
            Self::ArrowUpDown => "arrow.up.arrow.down",
            Self::Bell => "bell",
            Self::BellDot => "bell.badge",
            Self::Bug => "ladybug",
            Self::ChevronDown => "chevron.down",
            Self::ChevronLeft => "chevron.left",
            Self::ChevronRight => "chevron.right",
            Self::Check => "checkmark",
            Self::CircleX => "xmark.circle.fill",
            Self::Code => "chevron.left.forwardslash.chevron.right",
            Self::Columns => "square.split.2x1",
            Self::Cpu => "cpu",
            Self::Folder => "folder",
            Self::GitBranch => "arrow.triangle.branch",
            Self::Globe => "globe",
            Self::Grid => "square.grid.2x2",
            Self::LayoutSplit => "rectangle.split.2x2",
            Self::House => "house.fill",
            Self::Eye => "eye",
            Self::Lightbulb => "lightbulb",
            Self::Maximize => "arrow.up.left.and.arrow.down.right",
            Self::Network => "network",
            Self::Palette => "paintpalette",
            Self::PanelLeft => "sidebar.left",
            Self::Pin => "pin.fill",
            Self::Plus => "plus",
            Self::Puzzle => "puzzlepiece.extension",
            Self::Restore => "arrow.down.right.and.arrow.up.left",
            Self::Rows => "square.split.1x2",
            Self::Search => "magnifyingglass",
            Self::Settings => "gearshape",
            Self::Terminal => "terminal",
            Self::Trash => "trash",
            Self::X => "xmark",
        }
    }
}
