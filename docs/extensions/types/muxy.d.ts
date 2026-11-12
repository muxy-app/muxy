// Types for the `muxy` object Muxy gives extension code.
//
// Copy this file into an extension (for example `src/muxy.d.ts`) or point to
// it with `/// <reference path="…/muxy.d.ts" />`. Pages get `muxy` (also
// `window.muxy`), whose calls return Promises. Scripts and background scripts
// get the same calls returning their values directly; type them with
// `const api = (globalThis as any).muxy as Muxy.ScriptAPI;` or
// `Muxy.BackgroundAPI`.
//
// See api.md for what each call does and the permission it needs.

declare namespace Muxy {
  type JSONValue = string | number | boolean | null | JSONValue[] | { [key: string]: JSONValue };
  type Unsubscribe = () => void;

  interface ExecOptions {
    cwd?: string;
    env?: Record<string, string>;
    stdin?: string;
    /** Up to 300000; 30000 by default. */
    timeoutMs?: number;
  }
  interface ExecResult {
    stdout: string;
    stderr: string;
    exitCode: number;
    timedOut: boolean;
    truncated: boolean;
  }
  interface ExecJob {
    id: string;
    result: Promise<ExecResult>;
    cancel(): void;
  }

  interface Notice {
    title?: string;
    body?: string;
  }

  interface ConfirmOptions {
    title?: string;
    message?: string;
    buttons?: string[];
    default?: string;
    cancel?: string;
    style?: "informational" | "warning" | "critical";
  }

  interface ItemUpdate {
    id: string;
    /** An SF Symbol name or an SVG path inside the extension. */
    icon?: string;
    /** Status bar items only. */
    text?: string | null;
    visible?: boolean;
  }

  interface TabInfo {
    index: number;
    id: string;
    kind: string;
    title: string;
    isActive: boolean;
    isPinned: boolean;
    color: string | null;
    customTitle: string | null;
    isDetached: boolean;
    paneIDs: string[];
  }

  type TabRequest =
    | {
        kind: "extensionWebView";
        extension: { id: string; tabType: string; data?: JSONValue; singleton?: boolean };
      }
    | { kind: "terminal"; directory?: string; command?: string }
    | { kind: "browser"; url?: string };

  type Edge = "left" | "right" | "up" | "down";

  interface PaneInfo {
    id: string;
    title: string;
    workingDirectory: string;
    isFocused: boolean;
    tabID: string;
    projectID: string;
    worktreeID: string;
    isZoomed: boolean;
  }

  interface ProjectInfo {
    id: string;
    name: string;
    path: string;
    isActive: boolean;
    sortOrder: number;
    iconColor: string | null;
    icon: string | null;
    logo: string | null;
    worktreesEnabled: boolean;
  }

  interface WorkspaceInfo {
    id: string;
    name: string;
    projectCount: number;
    isActive: boolean;
  }

  interface WorktreeInfo {
    id: string;
    name: string;
    path: string;
    branch: string | null;
    isActive: boolean;
  }

  interface FileOptions {
    /** A project ID; the current project by default. */
    project?: string;
  }

  interface Frame {
    x?: number;
    y?: number;
    width?: number;
    height?: number;
  }

  interface Rect {
    x: number;
    y: number;
    width: number;
    height: number;
  }

  interface PanelOptions {
    instance?: string;
    title?: string | null;
    mode?: "pinned" | "floating" | "detached";
    position?: "right" | "bottom";
    frame?: Frame;
    [option: string]: unknown;
  }

  interface PanelState {
    panelID: string;
    instance: string | null;
    mode: "pinned" | "floating" | "detached";
    position: "right" | "bottom";
    visible: boolean;
    frame: Rect;
    bounds: Rect;
    [field: string]: unknown;
  }

  interface UIState {
    sidebar: boolean;
    agentsPanel: boolean;
    agentSessions: boolean;
    composer: boolean;
    quickTerminal: boolean;
    floatsHidden: boolean;
    layout: "project" | "tab" | "agents";
  }

  interface ModalItem {
    id: string;
    title: string;
    subtitle?: string | null;
  }

  type ModalFeed = (items: ModalItem[] | { items: ModalItem[] }) => unknown;

  interface ModalOptions {
    items?: ModalItem[] | ((feed: ModalFeed) => ModalItem[] | void | Promise<ModalItem[] | void>);
    placeholder?: string;
    emptyLabel?: string;
    noMatchLabel?: string;
    onSelect?: (item: ModalItem | null) => void;
    onQuery?: (
      query: string,
      feed: ModalFeed,
      options: Record<string, unknown>,
    ) => ModalItem[] | void | Promise<ModalItem[] | void>;
    onQueryChange?: (query: string, options: Record<string, unknown>) => unknown;
  }

  interface ModalWebviewOptions {
    entry: string;
    width?: number;
    height?: number;
    dismissOnOutsideClick?: boolean;
    data?: JSONValue;
  }

  interface GitOptions {
    project?: string;
    fresh?: boolean;
  }

  interface Theme {
    background: string;
    foreground: string;
    foregroundMuted: string;
    surface: string;
    surfaceSolid: string;
    border: string;
    hover: string;
    accent: string;
    accentForeground: string;
    accentSoft: string;
    diffAdd: string;
    diffRemove: string;
    diffHunk: string;
    colorScheme: "light" | "dark";
    topbarHeight: string;
  }

  interface HttpResponse {
    status: number;
    headers: Record<string, string>;
    body: string;
    truncated: boolean;
  }

  /** Workspace events an extension lists in its manifest's `events`. */
  type EventName =
    | "tab.created"
    | "tab.closed"
    | "tab.updated"
    | "tab.focused"
    | "pane.created"
    | "pane.closed"
    | "pane.focused"
    | "project.switched"
    | "worktree.switched"
    | "projects.changed"
    | "worktree.headChanged"
    | "agent.status"
    | "file.changed"
    | "notification.posted"
    | "panel.opened"
    | "panel.closed"
    | "popover.opened"
    | "popover.closed"
    | "modal.opened"
    | "modal.closed"
    | "ui.changed"
    | "theme.changed"
    | "preferences.changed"
    | "settings.changed"
    | `command.${string}`
    | `extension.${string}`;

  /** The calls every surface has, as pages see them. */
  interface SharedAPI {
    readonly extensionID: string;
    notifications: { notify(notice: Notice): Promise<void> };
    exec(argv: string[], options?: ExecOptions): Promise<ExecResult>;
    exec(options: ExecOptions & ({ shell: string } | { argv: string[] })): Promise<ExecResult>;
    dialog: {
      confirm(options: ConfirmOptions): Promise<string | null>;
      alert(options: { title?: string; message?: string; style?: string }): Promise<void>;
      prompt(options: {
        title?: string;
        message?: string;
        default?: string;
        placeholder?: string;
        confirm?: string;
        cancel?: string;
      }): Promise<string | null>;
      pickFolder(options?: { title?: string; message?: string; default?: string }): Promise<string | null>;
    };
    storage: {
      get<T = JSONValue>(key: string): Promise<T | null>;
      set(key: string, value: JSONValue): Promise<void>;
      delete(key: string): Promise<void>;
      keys(): Promise<string[]>;
    };
    shortcuts: {
      register(shortcut: { id: string; combo: string }): Promise<void>;
      unregister(id: string): Promise<void>;
      list(): Promise<{ id: string; combo: string }[]>;
    };
    topbar: {
      set(item: Omit<ItemUpdate, "text">): Promise<void>;
      show(id: string): Promise<void>;
      hide(id: string): Promise<void>;
    };
    statusbar: {
      set(item: ItemUpdate): Promise<void>;
      show(id: string): Promise<void>;
      hide(id: string): Promise<void>;
    };
    tabs: { open(request: TabRequest): Promise<string> };
    panels: {
      open(panelID: string, data?: JSONValue, options?: PanelOptions): Promise<PanelState | null>;
      toggle(panelID: string, data?: JSONValue, options?: PanelOptions): Promise<PanelState | null>;
      close(panelID: string, instance?: string): Promise<void>;
      update(panelID: string, options: PanelOptions, instance?: string): Promise<PanelState>;
      list(): Promise<PanelState[]>;
    };
    agents: {
      list(): Promise<
        { projectID: string; worktreeID: string; paneID: string; providerID: string; status: "working" | "waiting" | "idle" }[]
      >;
      focus(paneID: string): Promise<void>;
    };
    app: {
      info(): Promise<{
        name: string;
        version: string;
        platform: string;
        arch: string;
        language: string;
        theme: { name: string; colorScheme: "light" | "dark" };
      }>;
      openExternal(url: string): Promise<void>;
    };
    ui: {
      state(): Promise<UIState>;
      setSidebar(visible?: boolean): Promise<void>;
      setAgentsPanel(visible?: boolean): Promise<void>;
      setAgentSessions(visible?: boolean): Promise<void>;
      setLayout(layout: UIState["layout"]): Promise<void>;
      openCommandPalette(): Promise<void>;
      openSettings(section?: string): Promise<void>;
      back(): Promise<boolean>;
      forward(): Promise<boolean>;
      toggleFloats(): Promise<void>;
      quickTerminal: {
        show(): Promise<void>;
        hide(): Promise<void>;
        toggle(): Promise<void>;
        pin(pinned?: boolean): Promise<void>;
      };
    };
    commands: {
      list(): Promise<{ id: string; kind: "app" | "custom"; title: string; enabled: boolean; shortcut: string | null }[]>;
      run(id: string): Promise<string | null>;
    };
    composer: {
      open(): Promise<void>;
      close(): Promise<void>;
      getDraft(): Promise<string>;
      setDraft(text: string): Promise<void>;
      submit(): Promise<void>;
      insert(): Promise<void>;
    };
    layouts: {
      list(options?: { project?: string }): Promise<{ name: string; path: string }[]>;
      apply(name: string, options?: { project?: string }): Promise<void>;
    };
    preferences: {
      get(): Promise<Record<string, JSONValue>>;
      themes(): Promise<string[]>;
      set(key: string, value: JSONValue): Promise<void>;
    };
    settings: {
      get<T = JSONValue>(key: string): Promise<T>;
      all(): Promise<Record<string, JSONValue>>;
      set(key: string, value: JSONValue): Promise<void>;
      reset(key: string): Promise<void>;
    };
    clipboard: {
      readText(): Promise<string | null>;
      writeText(text: string): Promise<void>;
    };
    gh: { user(): Promise<Record<string, JSONValue> | null> };
    git: {
      status(options?: GitOptions & { local?: boolean }): Promise<JSONValue>;
      diff(
        options: GitOptions & { filePath?: string; raw?: boolean; staged?: boolean; lineLimit?: number },
      ): Promise<JSONValue>;
      repoInfo(options?: GitOptions): Promise<JSONValue>;
      log(options?: GitOptions & { maxCount?: number; skip?: number }): Promise<JSONValue>;
      branches(options?: GitOptions): Promise<JSONValue>;
      remoteBranches(options?: GitOptions): Promise<JSONValue>;
      currentBranch(options?: GitOptions): Promise<string | null>;
      aheadBehind(options?: GitOptions): Promise<JSONValue>;
      init(options?: GitOptions): Promise<void>;
      worktrees(options?: GitOptions): Promise<JSONValue>;
      stage(options: GitOptions & { paths: string[] }): Promise<void>;
      unstage(options: GitOptions & { paths: string[] }): Promise<void>;
      discard(options: GitOptions & { paths: string[]; untrackedPaths?: string[] }): Promise<void>;
      commit(options: GitOptions & { message: string; stageAll?: boolean }): Promise<JSONValue>;
      push(options?: GitOptions & { setUpstream?: boolean }): Promise<void>;
      pull(options?: GitOptions): Promise<void>;
      checkout(options: GitOptions & { hash: string }): Promise<void>;
      cherryPick(options: GitOptions & { hash: string }): Promise<void>;
      revert(options: GitOptions & { hash: string }): Promise<void>;
      branch: {
        create(options: GitOptions & { name: string }): Promise<void>;
        switchTo(options: GitOptions & { branch: string }): Promise<void>;
        delete(options: GitOptions & { name: string; force?: boolean }): Promise<void>;
        deleteRemote(options: GitOptions & { branch: string }): Promise<void>;
      };
      tag: { create(options: GitOptions & { name: string; hash: string }): Promise<void> };
      pr: {
        info(options?: GitOptions): Promise<JSONValue>;
        number(options?: GitOptions): Promise<number | null>;
        diff(options: GitOptions & { number: number; lineLimit?: number }): Promise<JSONValue>;
        checkout(options: GitOptions & { number: number }): Promise<void>;
        checkoutWorktree(options: GitOptions & { path: string; number: number }): Promise<void>;
        list(options?: GitOptions & { filter?: string; limit?: number; checks?: boolean }): Promise<JSONValue>;
        create(
          options: GitOptions & { title: string; body?: string; baseBranch?: string; draft?: boolean },
        ): Promise<JSONValue>;
        merge(
          options: GitOptions & { number: number; method?: string; deleteBranch?: boolean },
        ): Promise<void>;
        close(options: GitOptions & { number: number }): Promise<void>;
      };
      worktree: {
        add(
          options: GitOptions & { path: string; branch: string; createBranch?: boolean; baseBranch?: string },
        ): Promise<void>;
        remove(options: GitOptions & { path: string; force?: boolean; timeoutMs?: number }): Promise<void>;
        switchTo(options: GitOptions & { identifier: string }): Promise<void>;
      };
    };
    modal: {
      open(options: ModalOptions): Promise<ModalItem | null>;
      feed(items: ModalItem[] | { items: ModalItem[] }): Promise<void>;
      finish(): Promise<void>;
      openWebview(options: ModalWebviewOptions): Promise<JSONValue | null>;
      closeWebview(): Promise<void>;
    };
  }

  /** What pages and scripts have that background scripts don't. */
  interface WorkspaceAPI {
    toast(notice: Notice): Promise<void>;
    execAsync(argv: string[], options?: ExecOptions): ExecJob;
    execAsync(options: ExecOptions & ({ shell: string } | { argv: string[] })): ExecJob;
    tabs: {
      open(request: TabRequest): Promise<string>;
      list(options?: { project?: string }): Promise<TabInfo[]>;
      switchTo(indexOrIdOrTitle: string | number): Promise<void>;
      "new"(options?: { beside?: string; side?: "left" | "right" }): Promise<string>;
      next(): Promise<void>;
      previous(): Promise<void>;
      split(tabID: string, besideID: string, edge?: Edge): Promise<void>;
      close(tabID: string, options?: { scope?: "others" | "left" | "right" }): Promise<void>;
      rename(tabID: string, title: string | null): Promise<void>;
      setColor(tabID: string, color: string | null): Promise<void>;
      pin(tabID: string, pinned?: boolean): Promise<void>;
      move(tabID: string, to: { index?: number; project?: string }): Promise<void>;
      moveToWindow(tabID: string): Promise<void>;
      dock(tabID: string): Promise<void>;
    };
    browser: {
      open(url?: string, options?: { split?: boolean }): Promise<string>;
      navigate(tabID: string, url: string): Promise<void>;
      list(): Promise<{ tabId: string; title: string; url: string; profile: string; active: boolean }[]>;
      read(tabID: string): Promise<{ title: string; url: string; text: string }>;
      close(tabID: string): Promise<void>;
      [action: string]: unknown;
    };
    panes: {
      list(): Promise<PaneInfo[]>;
      send(paneID: string, text: string): Promise<void>;
      sendKeys(paneID: string, key: string): Promise<void>;
      readScreen(paneID: string, lines?: number): Promise<string>;
      close(paneID: string): Promise<void>;
      rename(paneID: string, title: string): Promise<void>;
      split(paneID: string, direction?: Edge): Promise<string>;
      focus(paneID: string): Promise<void>;
      zoom(paneID: string, zoomed?: boolean): Promise<void>;
      equalize(paneID: string): Promise<void>;
      detach(paneID: string): Promise<void>;
      restart(paneID: string, options?: { proxy?: boolean }): Promise<void>;
      scrollToBottom(paneID: string): Promise<void>;
      float(paneID: string, options?: { scope?: "project" | "allProjects" }): Promise<void>;
      moveToTab(paneID: string, options?: { project?: string }): Promise<void>;
      moveToWindow(paneID: string): Promise<void>;
    };
    projects: {
      list(): Promise<ProjectInfo[]>;
      switchTo(id: string): Promise<void>;
      delete(id: string): Promise<void>;
      add(path: string): Promise<string>;
      create(path: string, options?: { name?: string; workspace?: string; createIfMissing?: boolean }): Promise<string>;
      attach(id: string, workspace: string): Promise<void>;
      detach(id: string): Promise<void>;
      rename(id: string, name: string): Promise<void>;
      setColor(id: string, color: string | null): Promise<void>;
      setIcon(id: string, icon: string | null): Promise<void>;
      setLogo(id: string, logo: string | null): Promise<void>;
      reorder(ids: string[]): Promise<void>;
      next(): Promise<void>;
      previous(): Promise<void>;
      focus(id: string | null): Promise<void>;
      sort(order: "manual" | "name" | "nameDescending" | "recentlyActive" | "dateAdded"): Promise<void>;
      removed(): Promise<{ index: number; name: string; path: string; isLocal: boolean }[]>;
      restore(index: number): Promise<void>;
      apps(): Promise<{ id: string; name: string }[]>;
      openIn(id: string, app?: string): Promise<void>;
      reveal(id: string): Promise<void>;
    };
    workspaces: {
      list(): Promise<WorkspaceInfo[]>;
      create(name: string): Promise<string>;
      switchTo(id: string): Promise<void>;
      rename(id: string, name: string): Promise<void>;
      delete(id: string): Promise<void>;
    };
    worktrees: {
      list(project?: string): Promise<WorktreeInfo[]>;
      switchTo(id: string, project?: string): Promise<void>;
      refresh(project?: string): Promise<void>;
    };
    files: {
      list(path: string, options?: FileOptions): Promise<JSONValue>;
      read(path: string, options?: FileOptions): Promise<string>;
      stat(path: string, options?: FileOptions): Promise<JSONValue>;
      write(path: string, contents: string, options?: FileOptions): Promise<void>;
      mkdir(path: string, options?: FileOptions): Promise<void>;
      rename(path: string, newName: string, options?: FileOptions): Promise<void>;
      move(paths: string[], into: string, options?: FileOptions): Promise<void>;
      delete(paths: string[], options?: FileOptions): Promise<void>;
    };
  }

  interface Events {
    subscribe(name: EventName, handler: (payload: Record<string, JSONValue>) => void): Unsubscribe;
    /** Only `extension.*` names; other enabled extensions that list the event receive it. */
    emit(name: `extension.${string}`, payload?: JSONValue): Promise<void>;
  }

  interface PanelSelf {
    readonly instanceID: string | null;
    readonly state: PanelState | null;
    get(): Promise<PanelState>;
    update(options: PanelOptions): Promise<PanelState>;
    setOptions(options: PanelOptions): Promise<PanelState>;
    setFrame(frame: Frame): Promise<PanelState>;
    setTitle(title: string | null): Promise<PanelState>;
    setHitRegions(regions: Rect[] | null): Promise<PanelState>;
    setDragRegions(regions: Rect[] | null): Promise<PanelState>;
    trackHitElements(
      target: string | Element | Iterable<Element> | (() => Iterable<Element>),
      options?: { padding?: number; interval?: number },
    ): Unsubscribe;
    raise(): Promise<void>;
    onChange(callback: (state: PanelState) => void): Unsubscribe;
  }

  /** `window.muxy` on tabs, panels, popovers, the sidebar and modal pages. */
  interface PageAPI extends Omit<SharedAPI, "tabs" | "panels" | "modal">, Omit<WorkspaceAPI, "tabs"> {
    readonly tabInstanceID: string;
    readonly panelID: string | null;
    readonly panelInstanceID: string | null;
    readonly panel: PanelSelf | null;
    readonly data: JSONValue;
    readonly theme: Theme;
    readonly focused: boolean;
    onDataChange(callback: (data: JSONValue) => void): Unsubscribe;
    onThemeChange(callback: (theme: Theme) => void): Unsubscribe;
    onFocus(callback: (focused: boolean) => void): Unsubscribe;
    tabs: WorkspaceAPI["tabs"] & {
      setTitle(title: string): Promise<void>;
      setIcon(icon: string | null): Promise<void>;
    };
    panels: SharedAPI["panels"] & {
      /** Without an ID, closes the calling panel. */
      close(panelID?: string, instance?: string): Promise<void>;
    };
    popover: {
      close(): Promise<void>;
      resize(width: number, height: number): Promise<void>;
    };
    http: {
      fetch(
        url: string,
        options?: { method?: string; headers?: Record<string, string>; body?: string; timeoutMs?: number },
      ): Promise<HttpResponse>;
    };
    events: Events;
    modal: SharedAPI["modal"] & {
      /** From a modal page: resolves the `openWebview` that opened it. */
      submitWebview(result?: JSONValue): Promise<void>;
    };
    lifecycle: {
      onBeforeClose(
        callback: (closing: {
          surface: string;
          instanceID: string;
          panelInstanceID: string | null;
        }) => boolean | { prevent: boolean } | Promise<boolean | { prevent: boolean }>,
      ): Unsubscribe;
      close(): Promise<void>;
    };
  }

  /** The same calls, answering directly instead of with a Promise. */
  type Sync<T> = {
    [K in keyof T]: T[K] extends (...args: infer A) => Promise<infer R>
      ? (...args: A) => R
      : T[K] extends (...args: infer A) => infer R
        ? (...args: A) => R
        : T[K] extends object
          ? Sync<T[K]>
          : T[K];
  };

  /** `muxy` in a `runScript` command. */
  type ScriptAPI = Sync<SharedAPI & WorkspaceAPI>;

  /** `muxy` in the background script. */
  type BackgroundAPI = Sync<SharedAPI> & {
    events: {
      subscribe(name: EventName, handler: (payload: Record<string, JSONValue>) => void): Unsubscribe;
      unsubscribe(name: EventName, handler: (payload: Record<string, JSONValue>) => void): void;
      emit(name: `extension.${string}`, payload?: JSONValue): void;
    };
    /** Accepted for 1.x extensions; phone apps can't call remote methods in 2.x. */
    remote: {
      handle(action: string, handler: (...args: JSONValue[]) => unknown): void;
      unhandle(action: string): void;
    };
    modal: Sync<SharedAPI["modal"]> & {
      /** In background scripts, returns right away with the request ID. */
      open(options: ModalOptions): string | null;
      openWebview(options: ModalWebviewOptions): Promise<JSONValue | null>;
    };
  };
}

interface Window {
  readonly muxy: Muxy.PageAPI;
}

declare const muxy: Muxy.PageAPI;
