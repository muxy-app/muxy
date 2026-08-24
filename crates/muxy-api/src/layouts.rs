use crate::yaml::{self, Yaml};
use muxy_core::workspace::{AreaId, Axis, SplitNode, Tab, TabArea, TabId, TabKind};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutTab {
    pub name: Option<String>,
    pub command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pane {
    Leaf(LayoutTab),
    Branch { layout: Layout, panes: Vec<Pane> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub root: Pane,
    pub legacy_extra_tabs: Vec<LayoutTab>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    pub name: String,
    pub path: PathBuf,
}

pub fn discover(project_path: &str) -> Vec<Descriptor> {
    let directory = Path::new(project_path).join(".muxy").join("layouts");
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return Vec::new();
    };
    let mut descriptors: Vec<Descriptor> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.'))
        })
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .is_some_and(|extension| matches!(extension.as_str(), "yaml" | "yml" | "json"))
        })
        .filter_map(|path| {
            let name = path.file_stem()?.to_str()?.to_owned();
            Some(Descriptor { name, path })
        })
        .collect();
    descriptors.sort_by_key(|descriptor| descriptor.name.to_lowercase());
    descriptors
}

pub fn load(path: &Path) -> Option<Config> {
    let contents = std::fs::read_to_string(path).ok()?;
    let is_json = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
    let value = if is_json {
        from_json(&serde_json::from_str::<serde_json::Value>(&contents).ok()?)
    } else {
        yaml::parse(&contents)?
    };
    parse(&value)
}

fn from_json(value: &serde_json::Value) -> Yaml {
    match value {
        serde_json::Value::Object(map) => Yaml::Map(
            map.iter()
                .map(|(key, value)| (key.clone(), from_json(value)))
                .collect(),
        ),
        serde_json::Value::Array(values) => Yaml::Seq(values.iter().map(from_json).collect()),
        serde_json::Value::String(value) => Yaml::Str(value.clone()),
        serde_json::Value::Null => Yaml::Null,
        other => Yaml::Str(other.to_string()),
    }
}

pub fn parse(value: &Yaml) -> Option<Config> {
    let (root, legacy_extra_tabs) = parse_pane(value)?;
    Some(Config {
        root,
        legacy_extra_tabs,
    })
}

fn parse_pane(value: &Yaml) -> Option<(Pane, Vec<LayoutTab>)> {
    if !value.is_map() {
        return None;
    }
    if let Some(panes) = value.get("panes") {
        let entries = panes.as_seq()?;
        let children: Vec<(Pane, Vec<LayoutTab>)> = entries.iter().filter_map(parse_pane).collect();
        if children.is_empty() {
            return None;
        }
        let layout = parse_layout(value.get("layout"));
        let extras = children
            .iter()
            .flat_map(|(_, extras)| extras.iter().cloned())
            .collect();
        return Some((
            Pane::Branch {
                layout,
                panes: children.into_iter().map(|(pane, _)| pane).collect(),
            },
            extras,
        ));
    }
    if let Some(tab) = value.get("tab") {
        return Some((Pane::Leaf(parse_tab(tab)?), Vec::new()));
    }
    if let Some(tabs) = value.get("tabs") {
        let entries = tabs.as_seq()?;
        let mut parsed: Vec<LayoutTab> = entries.iter().filter_map(parse_tab).collect();
        if parsed.is_empty() {
            return None;
        }
        let first = parsed.remove(0);
        return Some((Pane::Leaf(first), parsed));
    }
    None
}

fn parse_layout(value: Option<&Yaml>) -> Layout {
    match value.and_then(Yaml::as_str).map(str::to_ascii_lowercase) {
        Some(raw) if raw == "vertical" => Layout::Vertical,
        _ => Layout::Horizontal,
    }
}

fn parse_tab(value: &Yaml) -> Option<LayoutTab> {
    if let Some(raw) = value.as_str() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        return Some(LayoutTab {
            name: None,
            command: Some(trimmed.to_owned()),
        });
    }
    if !value.is_map() {
        return None;
    }
    let name = value
        .get("name")
        .and_then(Yaml::as_str)
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty());
    let command = parse_command(value.get("command")).filter(|command| !command.is_empty());
    Some(LayoutTab { name, command })
}

fn parse_command(value: Option<&Yaml>) -> Option<String> {
    match value? {
        Yaml::Str(raw) => Some(raw.trim().to_owned()),
        Yaml::Seq(entries) => Some(
            entries
                .iter()
                .filter_map(Yaml::as_str)
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<&str>>()
                .join(" && "),
        ),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct Built {
    pub root: SplitNode,
    pub focused_area_id: AreaId,
    pub launches: Vec<(TabId, String)>,
}

pub fn build(config: &Config, project_path: &str) -> Option<Built> {
    let mut launches = Vec::new();
    let root = build_node(&config.root, project_path, &mut launches)?;
    let area_ids = root.area_ids();
    let first_area_id = area_ids.first()?.clone();
    let root_tab_id = root
        .area_by_id(&first_area_id)?
        .tabs
        .first()
        .map(|tab| tab.id.clone())?;

    let mut root = root;
    for area_id in area_ids.iter().skip(1) {
        if let Some(area) = root.area_by_id_mut(area_id)
            && let Some(tab) = area.tabs.first_mut()
        {
            tab.parent_id = Some(root_tab_id.clone());
        }
    }

    for extra in &config.legacy_extra_tabs {
        let tab = make_tab(extra, project_path, &mut launches);
        if let Some(area) = root.area_by_id_mut(&first_area_id) {
            area.insert_tab(usize::MAX, tab, false);
        }
    }
    if let Some(area) = root.area_by_id_mut(&first_area_id) {
        area.active_tab_id = Some(root_tab_id.clone());
    }

    Some(Built {
        focused_area_id: root.first_area_id().to_owned(),
        root,
        launches,
    })
}

fn build_node(
    pane: &Pane,
    project_path: &str,
    launches: &mut Vec<(TabId, String)>,
) -> Option<SplitNode> {
    match pane {
        Pane::Leaf(tab) => Some(SplitNode::area(TabArea::from_tab(make_tab(
            tab,
            project_path,
            launches,
        )))),
        Pane::Branch { layout, panes } => {
            let children: Vec<SplitNode> = panes
                .iter()
                .filter_map(|pane| build_node(pane, project_path, launches))
                .collect();
            let mut children = children.into_iter();
            let first = children.next()?;
            let axis = match layout {
                Layout::Horizontal => Axis::Horizontal,
                Layout::Vertical => Axis::Vertical,
            };
            Some(children.fold(first, |accumulated, next| {
                SplitNode::split(axis, 0.5, accumulated, next)
            }))
        }
    }
}

fn make_tab(source: &LayoutTab, project_path: &str, launches: &mut Vec<(TabId, String)>) -> Tab {
    let command = source
        .command
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(str::to_owned);
    let title = source
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| command.as_deref().map(command_title));
    let mut tab = Tab::new(TabKind::Terminal);
    tab.project_path = Some(project_path.to_owned());
    tab.custom_title = title;
    if let Some(command) = command {
        launches.push((tab.id.clone(), command));
    }
    tab
}

fn command_title(command: &str) -> String {
    command
        .trim()
        .split(' ')
        .find(|part| !part.is_empty())
        .unwrap_or("Terminal")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_yaml(source: &str) -> Option<Config> {
        parse(&yaml::parse(source)?)
    }

    fn tab(name: Option<&str>, command: Option<&str>) -> LayoutTab {
        LayoutTab {
            name: name.map(str::to_owned),
            command: command.map(str::to_owned),
        }
    }

    #[test]
    fn discover_sorts_case_insensitively_and_filters_extensions() {
        let directory = std::env::temp_dir().join("muxy-layouts-discover");
        let _ = std::fs::remove_dir_all(&directory);
        let layouts = directory.join(".muxy").join("layouts");
        std::fs::create_dir_all(&layouts).expect("temp dir");
        for name in ["B.yml", "a.yaml", "ignored.txt", ".hidden.yaml", "c.json"] {
            std::fs::write(layouts.join(name), "tab: x\n").expect("write");
        }
        let found = discover(directory.to_str().expect("utf8"));
        let names: Vec<&str> = found
            .iter()
            .map(|descriptor| descriptor.name.as_str())
            .collect();
        assert_eq!(names, vec!["a", "B", "c"]);
    }

    #[test]
    fn parses_the_legacy_tabs_fixture() {
        let config = parse_yaml(
            "tabs:\n  - name: editor\n    command: nvim\n  - name: shell\n    command: npm run dev\n",
        )
        .expect("parses");
        assert_eq!(config.root, Pane::Leaf(tab(Some("editor"), Some("nvim"))));
        assert_eq!(
            config.legacy_extra_tabs,
            vec![tab(Some("shell"), Some("npm run dev"))]
        );
    }

    #[test]
    fn panes_take_precedence_over_a_sibling_tab() {
        let config = parse_yaml("tab:\n  name: ignored\npanes:\n  - tab:\n      name: used\n")
            .expect("parses");
        assert_eq!(
            config.root,
            Pane::Branch {
                layout: Layout::Horizontal,
                panes: vec![Pane::Leaf(tab(Some("used"), None))],
            }
        );
    }

    #[test]
    fn empty_collections_and_scalar_roots_fail() {
        assert!(parse_yaml("panes: []\n").is_none());
        assert!(parse_yaml("tabs: []\n").is_none());
        assert!(parse(&Yaml::Str("nope".to_owned())).is_none());
        assert!(parse(&Yaml::Null).is_none());
    }

    #[test]
    fn a_list_command_joins_with_double_ampersand() {
        let config =
            parse_yaml("tab:\n  name: setup\n  command:\n    - cd src\n    - npm install\n")
                .expect("parses");
        assert_eq!(
            config.root,
            Pane::Leaf(tab(Some("setup"), Some("cd src && npm install")))
        );
    }

    #[test]
    fn a_tab_with_neither_usable_field_still_yields_an_empty_tab() {
        let config = parse_yaml("tab:\n  name: '  '\n  command: []\n").expect("parses");
        assert_eq!(config.root, Pane::Leaf(tab(None, None)));
    }

    #[test]
    fn an_unrecognised_layout_falls_back_to_horizontal() {
        let config = parse_yaml("layout: diagonal\npanes:\n  - tab: a\n").expect("parses");
        assert!(matches!(
            config.root,
            Pane::Branch {
                layout: Layout::Horizontal,
                ..
            }
        ));
    }

    #[test]
    fn unparseable_children_are_dropped_silently() {
        let config =
            parse_yaml("panes:\n  - tab:\n      name: kept\n  - nonsense: true\n").expect("parses");
        assert_eq!(
            config.root,
            Pane::Branch {
                layout: Layout::Horizontal,
                panes: vec![Pane::Leaf(tab(Some("kept"), None))],
            }
        );
    }

    #[test]
    fn the_json_schema_example_parses_through_the_same_walker() {
        let directory = std::env::temp_dir().join("muxy-layouts-json");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("dev.json");
        std::fs::write(
            &path,
            r#"{
  "layout": "horizontal",
  "panes": [
    { "tab": { "name": "editor", "command": "nvim" } },
    {
      "layout": "vertical",
      "panes": [
        { "tab": { "name": "logs", "command": "tail -f log" } },
        { "tab": { "name": "btop", "command": "btop" } }
      ]
    }
  ]
}"#,
        )
        .expect("write");
        let config = load(&path).expect("parses");
        assert_eq!(
            config.root,
            Pane::Branch {
                layout: Layout::Horizontal,
                panes: vec![
                    Pane::Leaf(tab(Some("editor"), Some("nvim"))),
                    Pane::Branch {
                        layout: Layout::Vertical,
                        panes: vec![
                            Pane::Leaf(tab(Some("logs"), Some("tail -f log"))),
                            Pane::Leaf(tab(Some("btop"), Some("btop"))),
                        ],
                    },
                ],
            }
        );
    }

    fn build_from(source: &str) -> Built {
        build(&parse_yaml(source).expect("parses"), "/tmp/project").expect("builds")
    }

    fn area_tabs(built: &Built, index: usize) -> &[Tab] {
        let area_id = &built.root.area_ids()[index];
        &built.root.area_by_id(area_id).expect("area").tabs
    }

    #[test]
    fn a_single_tab_leaf_becomes_one_unparented_tab() {
        let built = build_from("tab:\n  name: dev\n  command: npm run dev\n");
        assert_eq!(built.root.area_ids().len(), 1);
        let tabs = area_tabs(&built, 0);
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].custom_title.as_deref(), Some("dev"));
        assert_eq!(tabs[0].parent_id, None);
        assert_eq!(tabs[0].project_path.as_deref(), Some("/tmp/project"));
        assert_eq!(
            built.launches,
            vec![(tabs[0].id.clone(), "npm run dev".to_owned())]
        );
    }

    #[test]
    fn a_missing_name_falls_back_to_the_first_word_of_the_command() {
        let built = build_from("tab:\n  command: echo hello\n");
        assert_eq!(
            area_tabs(&built, 0)[0].custom_title.as_deref(),
            Some("echo")
        );
        let bare = build_from("tab:\n  name: ''\n");
        assert_eq!(area_tabs(&bare, 0)[0].custom_title, None);
    }

    #[test]
    fn legacy_extra_tabs_land_in_the_first_area_unparented() {
        let built = build_from("tabs:\n  - name: one\n  - command: echo hi\n  - name: three\n");
        assert_eq!(built.root.area_ids().len(), 1);
        let tabs = area_tabs(&built, 0);
        let titles: Vec<Option<&str>> =
            tabs.iter().map(|tab| tab.custom_title.as_deref()).collect();
        assert_eq!(titles, vec![Some("one"), Some("echo"), Some("three")]);
        assert!(tabs.iter().all(|tab| tab.parent_id.is_none()));
        let area_id = &built.root.area_ids()[0];
        assert_eq!(
            built.root.area_by_id(area_id).unwrap().active_tab_id,
            Some(tabs[0].id.clone())
        );
    }

    #[test]
    fn a_two_pane_horizontal_split_parents_the_second_tab_to_the_first() {
        let built = build_from(
            "layout: horizontal\npanes:\n  - tab:\n      name: left\n  - tab:\n      name: right\n",
        );
        assert!(matches!(
            built.root,
            SplitNode::Split {
                axis: Axis::Horizontal,
                ..
            }
        ));
        let left = &area_tabs(&built, 0)[0];
        let right = &area_tabs(&built, 1)[0];
        assert_eq!(left.parent_id, None);
        assert_eq!(right.parent_id.as_deref(), Some(left.id.as_str()));
        assert_eq!(built.focused_area_id, built.root.area_ids()[0]);
    }

    #[test]
    fn three_panes_fold_left_leaning_with_every_tab_parented_to_the_first() {
        let built = build_from(
            "layout: vertical\npanes:\n  - tab:\n      name: a\n  - tab:\n      name: b\n  - tab:\n      name: c\n",
        );
        let SplitNode::Split { axis, first, .. } = &built.root else {
            panic!("expected a split");
        };
        assert_eq!(*axis, Axis::Vertical);
        assert!(matches!(**first, SplitNode::Split { .. }));
        let root_tab = area_tabs(&built, 0)[0].id.clone();
        assert_eq!(area_tabs(&built, 0)[0].parent_id, None);
        assert_eq!(
            area_tabs(&built, 1)[0].parent_id.as_deref(),
            Some(root_tab.as_str())
        );
        assert_eq!(
            area_tabs(&built, 2)[0].parent_id.as_deref(),
            Some(root_tab.as_str())
        );
    }

    #[test]
    fn nested_branches_and_a_one_child_branch_collapse_correctly() {
        let built = build_from(
            "layout: horizontal\npanes:\n  - tab:\n      name: editor\n  - layout: vertical\n    panes:\n      - tab:\n          name: top\n      - tab:\n          name: bottom\n",
        );
        assert_eq!(built.root.area_ids().len(), 3);
        let SplitNode::Split { axis, second, .. } = &built.root else {
            panic!("expected a split");
        };
        assert_eq!(*axis, Axis::Horizontal);
        assert!(matches!(
            **second,
            SplitNode::Split {
                axis: Axis::Vertical,
                ..
            }
        ));

        let single = build_from("layout: horizontal\npanes:\n  - tab:\n      name: only\n");
        assert!(matches!(single.root, SplitNode::Area { .. }));
    }

    #[test]
    fn nested_legacy_extra_tabs_land_in_area_zero() {
        let built = build_from(
            "layout: horizontal\npanes:\n  - tab:\n      name: left\n  - tabs:\n      - name: right\n      - name: extra\n",
        );
        let titles: Vec<Option<&str>> = area_tabs(&built, 0)
            .iter()
            .map(|tab| tab.custom_title.as_deref())
            .collect();
        assert_eq!(titles, vec![Some("left"), Some("extra")]);
        assert_eq!(
            area_tabs(&built, 1)
                .iter()
                .map(|tab| tab.custom_title.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("right")]
        );
    }

    #[test]
    fn an_empty_branch_yields_nothing() {
        let config = Config {
            root: Pane::Branch {
                layout: Layout::Horizontal,
                panes: Vec::new(),
            },
            legacy_extra_tabs: Vec::new(),
        };
        assert!(build(&config, "/tmp/project").is_none());
    }

    #[test]
    fn legacy_extras_are_concatenated_depth_first() {
        let config = parse_yaml(
            "panes:\n  - tabs:\n      - name: a\n      - name: a2\n  - tabs:\n      - name: b\n      - name: b2\n",
        )
        .expect("parses");
        assert_eq!(
            config.legacy_extra_tabs,
            vec![tab(Some("a2"), None), tab(Some("b2"), None)]
        );
    }
}
