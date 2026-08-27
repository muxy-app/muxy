use super::*;

impl MainWindow {
    pub(crate) fn open_sort_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items =
            crate::views::workspace_switcher::sort_menu_items(self.state.workspace.sort_mode());
        self.open_menu(items, position, cx);
        window.focus(&self.view.menu_focus);
    }

    pub(crate) fn open_status_path_menu(
        &mut self,
        path: String,
        remote: bool,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = crate::views::status_bar::path_menu_items(path, remote);
        self.open_menu(items, position, cx);
        window.focus(&self.view.menu_focus);
    }

    pub(crate) fn open_project_menu(
        &mut self,
        project_id: &str,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.state.workspace.project(project_id) else {
            return;
        };
        let groups: Vec<&muxy_core::store::Group> =
            self.state.workspace.groups.all().iter().collect();
        let worktrees = self
            .state
            .worktrees
            .get(project_id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let active = self
            .state
            .prefs
            .active_worktree_ids
            .get(project_id)
            .map(String::as_str);
        let items = project_menu::items(project, &groups, worktrees, active);
        self.open_menu(items, position, cx);
        window.focus(&self.view.menu_focus);
    }

    pub(crate) fn open_worktree_menu(
        &mut self,
        project_id: &str,
        worktree_id: &str,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.state.workspace.project(project_id) else {
            return;
        };
        let Some(worktree) = self.state.worktrees.get(project_id).and_then(|worktrees| {
            worktrees
                .iter()
                .find(|worktree| worktree.id.eq_ignore_ascii_case(worktree_id))
        }) else {
            return;
        };
        let mut items = vec![Item::action(
            "Switch Worktree",
            Command::SelectWorktree {
                project_id: project_id.to_owned(),
                worktree_id: worktree.id.clone(),
            },
        )];
        if project.can_remove_worktree(worktree) {
            items.push(Item::Separator);
            items.push(
                Item::action(
                    "Remove Worktree…",
                    Command::RemoveWorktree {
                        project_id: project_id.to_owned(),
                        worktree_id: worktree.id.clone(),
                    },
                )
                .destructive(),
            );
        }
        self.open_menu(items, position, cx);
        window.focus(&self.view.menu_focus);
    }

    pub(crate) fn open_menu(
        &mut self,
        items: Vec<Item>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.view.subscriptions.clear();
        self.view.overlay = Overlay::Menu(Menu::new(items, position));
        cx.notify();
    }

    pub(crate) fn move_menu_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        if let Overlay::Menu(menu) = &mut self.view.overlay {
            menu.move_highlight(delta);
            cx.notify();
        }
    }

    pub(crate) fn confirm_menu_highlight(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Overlay::Menu(menu) = &self.view.overlay else {
            return;
        };
        let Some(command) = menu.highlighted_command() else {
            return;
        };
        self.perform(command, window, cx);
    }

    pub(crate) fn open_submenu(&mut self, index: Option<usize>) {
        if let Overlay::Menu(menu) = &mut self.view.overlay {
            menu.open_submenu = index;
        }
    }

    pub(crate) fn dismiss_overlay(&mut self, cx: &mut Context<Self>) {
        if matches!(self.view.overlay, Overlay::TerminalConfirm { .. }) {
            self.resolve_terminal_confirmation(false, cx);
            return;
        }
        if let Overlay::CreateWorktree(modal) = &self.view.overlay
            && !modal.read(cx).dismissible()
        {
            return;
        }
        if matches!(self.view.overlay, Overlay::CreateWorktree(_)) {
            self.view.worktrees.clear_create();
        }
        self.view.subscriptions.clear();
        self.view.overlay = Overlay::None;
        cx.notify();
    }

    pub(super) fn menu_anchor(&self) -> Point<Pixels> {
        match &self.view.overlay {
            Overlay::Menu(menu) => menu.position,
            Overlay::Rename { anchor, .. }
            | Overlay::GroupRename { anchor, .. }
            | Overlay::Symbols { anchor, .. }
            | Overlay::Colors { anchor, .. }
            | Overlay::TabColors { anchor, .. } => *anchor,
            Overlay::TabRename { bounds, .. } => bounds.origin,
            Overlay::None
            | Overlay::Picker(_)
            | Overlay::Omnibox(_)
            | Overlay::Settings(_)
            | Overlay::ThemePicker(_)
            | Overlay::CreateWorktree(_)
            | Overlay::TerminalConfirm { .. } => gpui::point(px(0.0), px(0.0)),
        }
    }

    pub(super) fn input_style(&self) -> InputStyle {
        let theme = &self.state.theme;
        let metrics = &self.state.metrics;
        InputStyle::compact(theme, metrics)
    }

    pub(crate) fn set_icon(
        &mut self,
        project_id: &str,
        icon: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.state.workspace.update(project_id, |project| {
            project.icon = icon.clone();
        });
        self.dismiss_overlay(cx);
    }

    pub(crate) fn set_icon_color(
        &mut self,
        project_id: &str,
        color: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.state.workspace.update(project_id, |project| {
            project.icon_color = color.clone();
        });
        self.dismiss_overlay(cx);
    }

    pub(crate) fn open_terminal_menu(
        &mut self,
        tab_id: &str,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let has_selection = self
            .terminal_runtime
            .surfaces
            .handle(tab_id)
            .is_some_and(|handle| handle.has_selection());
        let items = vec![
            Item::action("Copy", Command::TerminalCopy(tab_id.to_owned())).disabled(!has_selection),
            Item::action("Paste", Command::TerminalPaste(tab_id.to_owned())),
        ];
        self.open_menu(items, position, cx);
        window.focus(&self.view.menu_focus);
    }

    pub(super) fn open_project_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let root = self
            .state
            .prefs
            .project_search_root
            .clone()
            .unwrap_or_else(|| muxy_core::prefs::home_dir().to_string_lossy().into_owned());
        let paths = self.state.workspace.project_paths();
        let theme = self.state.theme.clone();
        let metrics = self.state.metrics;

        let search = self.picker_search.clone();
        let picker = cx.new(|cx| ProjectPicker::new(search, root, paths, theme, metrics, cx));
        let subscription = cx.subscribe(&picker, |window: &mut Self, _, event, cx| match event {
            PickerEvent::Dismiss => window.dismiss_overlay(cx),
            PickerEvent::Confirm {
                path,
                create_if_missing,
            } => window.confirm_project_path(path, *create_if_missing, cx),
            PickerEvent::ChooseFinder { directory } => {
                window.choose_project_with_finder(directory.clone(), cx)
            }
            PickerEvent::EditSearchLocation { directory } => {
                window.edit_search_location(directory.clone(), cx)
            }
        });

        window.focus(&picker.focus_handle(cx));
        self.view.subscriptions = vec![subscription];
        self.view.overlay = Overlay::Picker(picker);
        cx.notify();
    }

    pub(crate) fn open_omnibox(
        &mut self,
        scope: omnibox::Scope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Overlay::Omnibox(existing) = &self.view.overlay {
            let existing = existing.clone();
            if existing.read(cx).scope() == scope {
                self.dismiss_overlay(cx);
                return;
            }
            existing.update(cx, |view, cx| view.apply_scope(scope, cx));
            self.refresh_omnibox(true, cx);
            cx.notify();
            return;
        }

        let theme = self.state.theme.clone();
        let metrics = self.state.metrics;
        let view = cx.new(|cx| omnibox::Omnibox::new(scope, theme, metrics, cx));
        let subscription = cx.subscribe(&view, |window: &mut Self, _, event, cx| match event {
            omnibox::OmniboxEvent::Dismiss => window.dismiss_overlay(cx),
            omnibox::OmniboxEvent::QueryChanged => window.refresh_omnibox(true, cx),
            omnibox::OmniboxEvent::Confirm(action) => {
                let action = action.clone();
                window.perform_omnibox(action, cx);
            }
        });

        window.focus(&view.focus_handle(cx));
        self.view.subscriptions = vec![subscription];
        self.view.overlay = Overlay::Omnibox(view);
        self.refresh_omnibox(true, cx);
        cx.notify();
    }

    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let theme = self.state.theme.clone();
        let metrics = self.state.metrics;
        let appearance = self.state.appearance;
        let modal = cx.new(|cx| settings::SettingsModal::new(theme, metrics, appearance, cx));
        let subscription = cx.subscribe(&modal, |window: &mut Self, _, event, cx| match event {
            settings::SettingsEvent::Dismiss => window.dismiss_overlay(cx),
            settings::SettingsEvent::Applied(effect) => window.apply_settings(*effect, cx),
        });

        window.focus(&modal.focus_handle(cx));
        self.view.subscriptions = vec![subscription];
        self.view.overlay = Overlay::Settings(modal);
        cx.notify();
    }

    pub(crate) fn open_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.view.overlay, Overlay::ThemePicker(_)) {
            self.dismiss_overlay(cx);
            return;
        }
        let theme = self.state.theme.clone();
        let metrics = self.state.metrics;
        let appearance = self.state.appearance;
        let browser = cx.new(|cx| {
            settings::theme_picker::ThemeBrowser::new(
                settings::theme_picker::ThemeMode::CurrentAppearance,
                appearance,
                theme,
                metrics,
                cx,
            )
        });
        let subscription = cx.subscribe(&browser, |window: &mut Self, _, event, cx| match event {
            settings::theme_picker::ThemeBrowserEvent::Applied => window.apply_theme_setting(cx),
            settings::theme_picker::ThemeBrowserEvent::Dismiss => window.dismiss_overlay(cx),
        });

        window.focus(&browser.focus_handle(cx));
        self.view.subscriptions = vec![subscription];
        self.view.overlay = Overlay::ThemePicker(browser);
        cx.notify();
    }

    pub(crate) fn open_create_worktree(
        &mut self,
        project_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.view.overlay, Overlay::CreateWorktree(_))
            || self.view.worktrees.create_request().is_some()
            || self.state.project_operations.is_mutating(project_id)
        {
            return;
        }
        let Some(project) = self.state.workspace.project(project_id).cloned() else {
            return;
        };
        if project.is_home() || project.is_remote() || !project.is_git_repo {
            self.alert(
                "New Worktree".into(),
                "Worktrees require an existing local Git project.".into(),
                cx,
            );
            return;
        }
        let generation = self.state.project_operations.generation(project_id);
        let identity = self.view.worktrees.begin_create_for(project_id, generation);
        let git_options = self.project_runtime.git_options.clone();
        let home = muxy_core::prefs::home_dir();
        let location_context = muxy_api::worktree_location::LocationContext {
            home: home.clone(),
            profile_worktree_root: muxy_core::prefs::app_support_dir().join("worktree-checkouts"),
            default_path_template: self.state.prefs.default_worktree_path_template.clone(),
            default_parent_path: self.state.prefs.default_worktree_parent_path.clone(),
        };
        let global_config = muxy_api::worktree_config::global_config_path(
            &home,
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        );
        let project_path = project.path.clone();
        let expected_path = project.path.clone();
        cx.spawn(async move |window, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    let deadline = muxy_api::subprocess::Deadline::new(Duration::from_secs(30));
                    let branches = muxy_api::git::list_local_branches(
                        &git_options,
                        std::path::Path::new(&project_path),
                        &deadline,
                    );
                    let current = muxy_api::git::current_branch(
                        &git_options,
                        std::path::Path::new(&project_path),
                        &deadline,
                    );
                    let commands = muxy_api::worktree_config::resolved_commands(
                        muxy_api::worktree_config::HookKind::Setup,
                        std::path::Path::new(&project_path),
                        &global_config,
                        true,
                    );
                    (branches, current, commands)
                })
                .await;
            let _ = window.update(cx, |window, cx| {
                if !window.view.worktrees.matches_create(&identity)
                    || window
                        .state
                        .project_operations
                        .generation(&identity.project_id)
                        != identity.generation
                    || window
                        .state
                        .workspace
                        .project(&identity.project_id)
                        .is_none_or(|current| current.path != expected_path)
                {
                    return;
                }
                let (branches, current, commands) = loaded;
                let mut initial_error = None;
                let branches = branches.unwrap_or_else(|error| {
                    initial_error = Some(error.to_string());
                    Vec::new()
                });
                let current = current.unwrap_or_else(|error| {
                    initial_error.get_or_insert_with(|| error.to_string());
                    None
                });
                let commands = commands.unwrap_or_else(|error| {
                    initial_error.get_or_insert_with(|| error.to_string());
                    Vec::new()
                });
                let mut form = create_worktree_overlay::CreateWorktreeForm::new(
                    project,
                    location_context,
                    commands,
                );
                form.set_error(initial_error);
                let theme = window.state.theme.clone();
                let metrics = window.state.metrics;
                let modal = cx.new(|cx| {
                    create_worktree_overlay::CreateWorktreeModal::new(
                        form, branches, current, theme, metrics, cx,
                    )
                });
                let subscription =
                    cx.subscribe(&modal, |window: &mut Self, modal, event, cx| match event {
                        create_worktree_overlay::CreateWorktreeEvent::Dismiss => {
                            window.view.worktrees.clear_create();
                            window.dismiss_overlay(cx);
                        }
                        create_worktree_overlay::CreateWorktreeEvent::ChooseFolder => {
                            window.choose_worktree_folder(modal, cx);
                        }
                        create_worktree_overlay::CreateWorktreeEvent::Submit {
                            request,
                            path_template,
                            parent_path,
                        } => window.request_native_worktree_creation(
                            *request.clone(),
                            path_template.clone(),
                            parent_path.clone(),
                            modal,
                            cx,
                        ),
                    });
                window.view.pending_focus = Some(modal.focus_handle(cx));
                window.view.subscriptions = vec![subscription];
                window.view.overlay = Overlay::CreateWorktree(modal);
                cx.notify();
            });
        })
        .detach();
        window.focus(&self.view.workspace_focus);
    }

    fn choose_worktree_folder(
        &mut self,
        modal: Entity<create_worktree_overlay::CreateWorktreeModal>,
        cx: &mut Context<Self>,
    ) {
        let Some(identity) = self.view.worktrees.create_request().cloned() else {
            return;
        };
        let directory = self
            .state
            .workspace
            .project(&identity.project_id)
            .map(|project| project.path.clone());
        cx.spawn(async move |window, cx| {
            let selected =
                crate::views::file_dialog::pick_folder(crate::views::file_dialog::FolderRequest {
                    message: "Select where new worktrees for this project should be created",
                    directory,
                })
                .await;
            let _ = window.update(cx, |window, cx| {
                if !window.view.worktrees.matches_create(&identity)
                    || window
                        .state
                        .project_operations
                        .generation(&identity.project_id)
                        != identity.generation
                {
                    return;
                }
                if let Some(path) = selected {
                    modal.update(cx, |modal, cx| modal.set_folder(path, cx));
                }
            });
        })
        .detach();
    }

    pub(super) fn refresh_omnibox(&mut self, reset_highlight: bool, cx: &mut Context<Self>) {
        let Overlay::Omnibox(view) = &self.view.overlay else {
            return;
        };
        let view = view.clone();
        let (scope, query) = {
            let read = view.read(cx);
            (read.scope(), read.query(cx))
        };
        let rows = omnibox::items::ranked(
            omnibox::items::items(&self.state, &self.terminal_runtime.surfaces, scope),
            scope,
            &query,
        );
        view.update(cx, |view, cx| view.set_rows(rows, reset_highlight, cx));
    }

    pub(super) fn perform_omnibox(&mut self, action: omnibox::ItemAction, cx: &mut Context<Self>) {
        self.dismiss_overlay(cx);
        match action {
            omnibox::ItemAction::SelectProject(id) => {
                self.state.select_project(&id);
                cx.notify();
            }
            omnibox::ItemAction::RestoreProject(id) => self.restore_removed_project(&id, cx),
            omnibox::ItemAction::SelectWorktree {
                project_id,
                worktree_id,
            } => {
                self.state.select_worktree(&project_id, &worktree_id);
                cx.notify();
            }
            omnibox::ItemAction::SelectGroup(group_id) => {
                self.state.workspace.select_group(group_id);
                if let Some(first) = self
                    .state
                    .workspace
                    .visible_projects()
                    .first()
                    .map(|project| project.id.clone())
                {
                    self.state.select_project(&first);
                }
                cx.notify();
            }
            omnibox::ItemAction::SelectTab {
                project_id,
                worktree_path,
                tab_id,
            } => self.select_omnibox_tab(&project_id, &worktree_path, &tab_id, cx),
            omnibox::ItemAction::RunCommand(id) => self.create_command_tab(&id, cx),
        }
    }

    pub(super) fn restore_removed_project(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(project) = muxy_core::store::load_recently_removed()
            .into_iter()
            .find(|entry| entry.project.id.eq_ignore_ascii_case(id))
            .map(|entry| entry.project)
        else {
            self.alert(
                "Restore Project".to_owned(),
                "This project is no longer available in Recently Removed.".to_owned(),
                cx,
            );
            return;
        };
        let path = project.path.clone();
        match muxy_api::picker::path_service::directory_state(&path) {
            muxy_api::picker::path_service::DirectoryState::Missing => {
                self.alert(
                    "Restore Project".to_owned(),
                    format!("The project folder no longer exists at {path}."),
                    cx,
                );
                return;
            }
            muxy_api::picker::path_service::DirectoryState::NotDirectory => {
                self.alert(
                    "Restore Project".to_owned(),
                    format!("The project path is no longer a folder: {path}."),
                    cx,
                );
                return;
            }
            muxy_api::picker::path_service::DirectoryState::Directory => {}
        }

        if let Some(existing_id) = self
            .state
            .workspace
            .contains_path(&path)
            .map(|project| project.id.clone())
        {
            let _ = muxy_core::store::take_recently_removed(id);
            self.state.select_project(&existing_id);
            cx.notify();
            return;
        }

        let restored_id = project.id.clone();
        if !self.state.workspace.restore(project) {
            self.alert(
                "Restore Project".to_owned(),
                "Muxy could not save the restored project.".to_owned(),
                cx,
            );
            return;
        }
        if muxy_core::store::take_recently_removed(id).is_none() {
            log::warn!("restored project but failed to update recently removed projects");
        }
        if let Some(group_id) = self.state.workspace.active_group_id.clone() {
            self.state
                .workspace
                .groups
                .add_project(&restored_id, &group_id);
        }
        self.state.select_project(&restored_id);
        self.refresh_project_truth(None, cx);
        cx.notify();
    }

    pub(super) fn select_omnibox_tab(
        &mut self,
        project_id: &str,
        worktree_path: &str,
        tab_id: &str,
        cx: &mut Context<Self>,
    ) {
        let worktree_id = self.state.worktrees.get(project_id).and_then(|list| {
            list.iter()
                .find(|worktree| worktree.path == worktree_path)
                .map(|worktree| worktree.id.clone())
        });
        match worktree_id {
            Some(worktree_id) => self.state.select_worktree(project_id, &worktree_id),
            None => self.state.select_project(project_id),
        }
        let target = self.state.active_tab_workspace().and_then(|workspace| {
            let root = workspace.root.as_ref()?;
            let area_id = root.area_ids().into_iter().find(|area_id| {
                root.area_by_id(area_id)
                    .is_some_and(|area| area.tabs.iter().any(|tab| tab.id == tab_id))
            })?;
            Some((
                area_id,
                workspace.root_id_for_tab(tab_id).map(str::to_owned),
            ))
        });
        if let Some((area_id, root_id)) = target
            && let Some(workspace) = self.state.active_tab_workspace_mut()
        {
            if let Some(root_id) = root_id {
                workspace.select_root_tab(&root_id);
            }
            workspace.select_tab(&area_id, tab_id);
            let _ = self.state.persist_tab_workspaces();
        }
        cx.notify();
    }

    pub(super) fn choose_project_with_finder(&mut self, directory: String, cx: &mut Context<Self>) {
        self.dismiss_overlay(cx);
        cx.spawn(async move |window, cx| {
            let request = crate::views::file_dialog::FolderRequest {
                message: "Select a project folder",
                directory: Some(directory),
            };
            let Some(path) = crate::views::file_dialog::pick_folder(request).await else {
                return;
            };
            let path = path.to_string_lossy().into_owned();
            let _ = window.update(cx, |window, cx| {
                window.confirm_project_path(&path, false, cx);
            });
        })
        .detach();
    }

    pub(super) fn edit_search_location(&mut self, directory: String, cx: &mut Context<Self>) {
        self.dismiss_overlay(cx);
        cx.spawn(async move |window, cx| {
            let request = crate::views::file_dialog::FolderRequest {
                message: "Select where Muxy searches for project folders",
                directory: Some(directory),
            };
            let Some(path) = crate::views::file_dialog::pick_folder(request).await else {
                return;
            };
            let path = path.to_string_lossy().into_owned();
            Prefs::store_settings_value(
                "muxy.projectPicker.defaultDirectory",
                Value::String(path.clone()),
            );
            let _ = window.update(cx, |window, cx| {
                window.state.prefs.project_search_root = Some(path);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn confirm_project_path(
        &mut self,
        path: &str,
        create_if_missing: bool,
        cx: &mut Context<Self>,
    ) {
        let standardized = muxy_api::picker::path_service::standardize(path);
        let missing = matches!(
            muxy_api::picker::path_service::directory_state(&standardized),
            muxy_api::picker::path_service::DirectoryState::Missing
        );

        if missing && create_if_missing {
            let message = format!("Muxy will create \"{standardized}\" and add it as a project.");
            let answer = self.ask(
                "Create Project Folder?".to_owned(),
                message,
                &["Create & Add", "Cancel"],
                cx,
            );
            cx.spawn(async move |window, cx| {
                if answer.await != Some(0) {
                    return;
                }
                let _ = window.update(cx, |window, cx| {
                    window.add_project_path(&standardized, true, cx);
                });
            })
            .detach();
            return;
        }

        self.add_project_path(&standardized, create_if_missing, cx);
    }

    pub(super) fn add_project_path(
        &mut self,
        path: &str,
        create_if_missing: bool,
        cx: &mut Context<Self>,
    ) {
        let result = muxy_api::picker::confirm::ensure_directory(path, create_if_missing);
        if result != muxy_api::picker::confirm::ConfirmResult::Success {
            self.alert(result.title().to_owned(), result.message(path), cx);
            return;
        }

        let existing = self
            .state
            .workspace
            .contains_path(path)
            .map(|project| project.id.clone());
        let id = match existing {
            Some(id) => Some(id),
            None => {
                let name = muxy_api::picker::path_service::last_component(path);
                self.state.workspace.add(name, path.to_owned())
            }
        };

        if let Some(id) = id {
            if let Some(group_id) = self.state.workspace.active_group_id.clone() {
                self.state.workspace.groups.add_project(&id, &group_id);
            }
            self.state.workspace.sort();
            self.state.select_project(&id);
            self.refresh_project_truth(None, cx);
        }

        self.dismiss_overlay(cx);
    }

    pub(super) fn confirm_delete_group(&mut self, group_id: String, cx: &mut Context<Self>) {
        let name = self.state.workspace.groups.name_for(Some(&group_id));
        self.dismiss_overlay(cx);

        let answer = self.ask(
            format!("Delete \"{name}\"?"),
            "Projects in this workspace will not be deleted.".to_owned(),
            &["Delete", "Cancel"],
            cx,
        );

        cx.spawn(async move |window, cx| {
            if answer.await != Some(0) {
                return;
            }
            let _ = window.update(cx, |window, cx| {
                if window.state.workspace.active_group_id.as_deref() == Some(group_id.as_str()) {
                    window.state.workspace.select_group(None);
                }
                window.state.workspace.groups.remove(&group_id);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn confirm_remove(&mut self, project_id: String, cx: &mut Context<Self>) {
        let Some(project) = self.state.workspace.project(&project_id) else {
            return;
        };
        let title = format!("Remove \"{}\"?", project.name);
        self.dismiss_overlay(cx);

        let answer = self.ask(
            title,
            "This will remove the project from Muxy. Project files on disk will not be deleted."
                .to_owned(),
            &["Remove", "Cancel"],
            cx,
        );

        cx.spawn(async move |window, cx| {
            if answer.await != Some(0) {
                return;
            }
            let _ = window.update(cx, |window, cx| {
                window.remove_project(&project_id, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn remove_project(&mut self, project_id: &str, cx: &mut Context<Self>) {
        if self.state.project_operations.is_mutating(project_id) {
            self.alert(
                "Project Is Busy".to_owned(),
                "Wait for the active worktree operation to finish before removing this project."
                    .to_owned(),
                cx,
            );
            return;
        }
        self.state.remove_project(project_id);
    }

    pub(super) fn ask(
        &self,
        title: String,
        message: String,
        buttons: &'static [&'static str],
        cx: &mut Context<Self>,
    ) -> gpui::Task<Option<usize>> {
        let handle = cx.active_window();
        cx.spawn(async move |_, cx| {
            let handle = handle?;
            let answer = cx
                .update(|cx| {
                    handle
                        .update(cx, |_, window, cx| {
                            window.prompt(
                                gpui::PromptLevel::Warning,
                                &title,
                                Some(&message),
                                buttons,
                                cx,
                            )
                        })
                        .ok()
                })
                .ok()
                .flatten()?;
            answer.await.ok()
        })
    }

    pub(super) fn alert(&mut self, title: String, message: String, cx: &mut Context<Self>) {
        self.ask(title, message, &["OK"], cx).detach();
    }

    pub(super) fn start_rename(&mut self, project_id: String, cx: &mut Context<Self>) {
        let Some(project) = self.state.workspace.project(&project_id) else {
            return;
        };
        let name = project.name.clone();
        let anchor = self.menu_anchor();
        let style = self.input_style();
        let input = cx.new(|cx| {
            TextInput::new(style, cx)
                .with_placeholder("Project name")
                .with_text(name)
        });
        input.update(cx, |input: &mut TextInput, cx| input.select_all_text(cx));

        let committed = project_id.clone();
        let subscription = cx.subscribe(
            &input,
            move |window: &mut Self, input: Entity<TextInput>, event, cx| match event {
                InputEvent::Submitted => {
                    let name = input.read(cx).text().trim().to_owned();
                    if !name.is_empty() {
                        window.state.workspace.update(&committed, |project| {
                            project.name = name.clone();
                        });
                        window.state.workspace.sort();
                    }
                    window.dismiss_overlay(cx);
                }
                InputEvent::Cancelled => window.dismiss_overlay(cx),
                InputEvent::Changed => {}
            },
        );

        self.view.pending_focus = Some(input.focus_handle(cx));
        self.view.subscriptions = vec![subscription];
        self.view.overlay = Overlay::Rename { input, anchor };
        cx.notify();
    }

    pub(super) fn start_group_rename(&mut self, group_id: Option<String>, cx: &mut Context<Self>) {
        let anchor = self.menu_anchor();
        let style = self.input_style();
        let existing = group_id
            .as_deref()
            .map(|id| self.state.workspace.groups.name_for(Some(id)))
            .unwrap_or_default();
        let input = cx.new(|cx| {
            TextInput::new(style, cx)
                .with_placeholder("Workspace name")
                .with_text(existing)
        });
        if group_id.is_some() {
            input.update(cx, |input: &mut TextInput, cx| input.select_all_text(cx));
        }

        let committed = group_id.clone();
        let subscription = cx.subscribe(
            &input,
            move |window: &mut Self, input: Entity<TextInput>, event, cx| match event {
                InputEvent::Submitted => {
                    let name = input.read(cx).text().trim().to_owned();
                    if !name.is_empty() {
                        match committed.as_deref() {
                            Some(id) => window.state.workspace.groups.rename(id, name),
                            None => {
                                window.state.workspace.groups.add(name);
                            }
                        }
                    }
                    window.dismiss_overlay(cx);
                }
                InputEvent::Cancelled => window.dismiss_overlay(cx),
                InputEvent::Changed => {}
            },
        );

        self.view.pending_focus = Some(input.focus_handle(cx));
        self.view.subscriptions = vec![subscription];
        self.view.overlay = Overlay::GroupRename {
            group_id,
            input,
            anchor,
        };
        cx.notify();
    }

    pub(crate) fn open_add_project_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let removed = muxy_core::store::load_recently_removed();
        if removed.is_empty() {
            self.perform(Command::OpenProjectPicker, window, cx);
            return;
        }
        let mut items = vec![
            Item::action("Local", Command::OpenProjectPicker),
            Item::Separator,
            Item::label("Recently Removed"),
        ];
        items.extend(removed.into_iter().map(|entry| {
            Item::action(
                entry.project.name,
                Command::RestoreRecentlyRemoved(entry.project.id),
            )
        }));
        self.open_menu(items, position, cx);
        window.focus(&self.view.menu_focus);
    }

    pub(crate) fn open_layout_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_menu(app_layout_menu_items(), position, cx);
        window.focus(&self.view.menu_focus);
    }

    pub(crate) fn open_terminal_layout_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items: Vec<Item> = self
            .state
            .layouts()
            .into_iter()
            .map(|descriptor| {
                Item::action(
                    descriptor.name,
                    Command::ApplyLayout(descriptor.path.to_string_lossy().into_owned()),
                )
            })
            .collect();
        if items.is_empty() {
            return;
        }
        self.open_menu(items, position, cx);
        window.focus(&self.view.menu_focus);
    }

    pub(crate) fn open_workspace_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active = self.state.workspace.active_group_id.clone();
        let groups: Vec<(String, String, bool)> = self
            .state
            .workspace
            .groups
            .all()
            .iter()
            .map(|group| (group.id.clone(), group.name.clone(), group.is_local))
            .collect();

        let mut items = vec![
            Item::action("All Projects", Command::SelectWorkspaceGroup(None))
                .checked(active.is_none()),
        ];
        if !groups.is_empty() {
            items.push(Item::Separator);
            for (id, name, _) in &groups {
                items.push(
                    Item::action(name, Command::SelectWorkspaceGroup(Some(id.clone())))
                        .checked(active.as_deref() == Some(id.as_str())),
                );
            }
        }

        let renameable: Vec<Item> = groups
            .iter()
            .filter(|(_, _, is_local)| *is_local)
            .map(|(id, name, _)| Item::action(name, Command::RenameWorkspaceGroup(id.clone())))
            .collect();
        let deletable: Vec<Item> = groups
            .iter()
            .map(|(id, name, _)| {
                Item::action(name, Command::DeleteWorkspaceGroup(id.clone())).destructive()
            })
            .collect();

        items.push(Item::Separator);
        let rename_empty = renameable.is_empty();
        let delete_empty = deletable.is_empty();
        items.push(Item::submenu("Rename Workspace", renameable).disabled(rename_empty));
        items.push(Item::submenu("Delete Workspace", deletable).disabled(delete_empty));
        items.push(Item::Separator);
        items.push(Item::action("New Workspace", Command::CreateWorkspaceGroup));

        self.open_menu(items, position, cx);
        window.focus(&self.view.menu_focus);
    }

    pub(super) fn open_symbol_picker(&mut self, project_id: String, cx: &mut Context<Self>) {
        let anchor = self.menu_anchor();
        let style = self.input_style();
        let search = cx.new(|cx| TextInput::new(style, cx).with_placeholder("Search symbols..."));
        let subscription = cx.subscribe(&search, |window: &mut Self, _, event, cx| match event {
            InputEvent::Cancelled => window.dismiss_overlay(cx),
            _ => cx.notify(),
        });
        self.view.pending_focus = Some(search.focus_handle(cx));
        self.view.subscriptions = vec![subscription];
        self.view.overlay = Overlay::Symbols {
            project_id,
            search,
            anchor,
        };
        cx.notify();
    }

    pub(super) fn pick_logo(&mut self, project_id: String, cx: &mut Context<Self>) {
        let directory = self
            .state
            .workspace
            .project(&project_id)
            .map(|project| project.path.clone());
        self.dismiss_overlay(cx);

        cx.spawn(async move |window, cx| {
            let request = crate::views::file_dialog::ImageRequest {
                title: "Choose a Logo Image",
                directory,
            };
            let Some(source) = crate::views::file_dialog::pick_image(request).await else {
                return;
            };
            let Some(filename) = logo::store(&source, &project_id) else {
                let _ = window.update(cx, |window, cx| {
                    window.alert(
                        "Could Not Read Image".to_owned(),
                        "Muxy couldn't read that image. Choose a different file.".to_owned(),
                        cx,
                    );
                });
                return;
            };
            let _ = window.update(cx, |window, cx| {
                window.state.workspace.update(&project_id, |project| {
                    project.logo = Some(filename.clone());
                });
                cx.notify();
            });
        })
        .detach();
    }
}

fn app_layout_menu_items() -> Vec<Item> {
    vec![
        Item::action("Project Focused", Command::DismissOverlay).checked(true),
        Item::action("Tab Focused", Command::DismissOverlay).disabled(true),
        Item::action("Agents Focused", Command::DismissOverlay).disabled(true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_app_layout_menu_matches_supported_sidebar_layouts() {
        let items = app_layout_menu_items();
        let entries: Vec<(&str, bool, bool)> = items
            .iter()
            .map(|item| match item {
                Item::Action {
                    label,
                    disabled,
                    checked,
                    ..
                } => (label.as_ref(), *disabled, *checked),
                _ => panic!("app layout menu must contain actions only"),
            })
            .collect();

        assert_eq!(
            entries,
            vec![
                ("Project Focused", false, true),
                ("Tab Focused", true, false),
                ("Agents Focused", true, false),
            ]
        );
    }
}
