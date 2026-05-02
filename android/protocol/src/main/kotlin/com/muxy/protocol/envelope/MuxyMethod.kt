package com.muxy.protocol.envelope

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
enum class MuxyMethod {
    @SerialName("listProjects")
    LIST_PROJECTS,

    @SerialName("selectProject")
    SELECT_PROJECT,

    @SerialName("listWorktrees")
    LIST_WORKTREES,

    @SerialName("selectWorktree")
    SELECT_WORKTREE,

    @SerialName("getWorkspace")
    GET_WORKSPACE,

    @SerialName("createTab")
    CREATE_TAB,

    @SerialName("closeTab")
    CLOSE_TAB,

    @SerialName("selectTab")
    SELECT_TAB,

    @SerialName("splitArea")
    SPLIT_AREA,

    @SerialName("closeArea")
    CLOSE_AREA,

    @SerialName("focusArea")
    FOCUS_AREA,

    @SerialName("terminalInput")
    TERMINAL_INPUT,

    @SerialName("terminalResize")
    TERMINAL_RESIZE,

    @SerialName("terminalScroll")
    TERMINAL_SCROLL,

    @SerialName("getTerminalContent")
    GET_TERMINAL_CONTENT,

    @SerialName("registerDevice")
    REGISTER_DEVICE,

    @SerialName("pairDevice")
    PAIR_DEVICE,

    @SerialName("authenticateDevice")
    AUTHENTICATE_DEVICE,

    @SerialName("takeOverPane")
    TAKE_OVER_PANE,

    @SerialName("releasePane")
    RELEASE_PANE,

    @SerialName("getVCSStatus")
    GET_VCS_STATUS,

    @SerialName("vcsCommit")
    VCS_COMMIT,

    @SerialName("vcsPush")
    VCS_PUSH,

    @SerialName("vcsPull")
    VCS_PULL,

    @SerialName("vcsStageFiles")
    VCS_STAGE_FILES,

    @SerialName("vcsUnstageFiles")
    VCS_UNSTAGE_FILES,

    @SerialName("vcsDiscardFiles")
    VCS_DISCARD_FILES,

    @SerialName("vcsListBranches")
    VCS_LIST_BRANCHES,

    @SerialName("vcsSwitchBranch")
    VCS_SWITCH_BRANCH,

    @SerialName("vcsCreateBranch")
    VCS_CREATE_BRANCH,

    @SerialName("vcsCreatePR")
    VCS_CREATE_PR,

    @SerialName("vcsAddWorktree")
    VCS_ADD_WORKTREE,

    @SerialName("vcsRemoveWorktree")
    VCS_REMOVE_WORKTREE,

    @SerialName("getProjectLogo")
    GET_PROJECT_LOGO,

    @SerialName("listNotifications")
    LIST_NOTIFICATIONS,

    @SerialName("markNotificationRead")
    MARK_NOTIFICATION_READ,

    @SerialName("subscribe")
    SUBSCRIBE,

    @SerialName("unsubscribe")
    UNSUBSCRIBE,
}
