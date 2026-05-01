package com.muxy.android.notifications

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.muxy.net.MuxyClient
import com.muxy.protocol.dto.NotificationDTO
import com.muxy.protocol.dto.SplitNodeDTO
import com.muxy.protocol.dto.TabAreaDTO
import com.muxy.protocol.dto.WorkspaceDTO
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.util.UUID

class NotificationsViewModel(private val muxyClient: MuxyClient) : ViewModel() {
    val notifications: StateFlow<List<NotificationDTO>> = muxyClient.notifications

    private val _isLoading = MutableStateFlow(false)
    val isLoading: StateFlow<Boolean> = _isLoading.asStateFlow()

    private val _errorMessage = MutableStateFlow<String?>(null)
    val errorMessage: StateFlow<String?> = _errorMessage.asStateFlow()

    private val _staleNotificationMessage = MutableStateFlow<String?>(null)
    val staleNotificationMessage: StateFlow<String?> = _staleNotificationMessage.asStateFlow()

    private val _pendingNavigationProjectID = MutableStateFlow<UUID?>(null)
    val pendingNavigationProjectID: StateFlow<UUID?> = _pendingNavigationProjectID.asStateFlow()

    fun refresh() {
        viewModelScope.launch {
            _isLoading.value = true
            _errorMessage.value = null
            val ok = muxyClient.refreshNotifications()
            if (!ok) _errorMessage.value = "Could not load notifications"
            _isLoading.value = false
        }
    }

    fun openNotification(notification: NotificationDTO) {
        viewModelScope.launch {
            _staleNotificationMessage.value = null
            _errorMessage.value = null

            val projectExists = muxyClient.projects.value.any { it.id == notification.projectID }
            if (!projectExists) {
                _staleNotificationMessage.value = "This notification points to a closed tab"
                muxyClient.markNotificationRead(notification.id)
                return@launch
            }

            val activeID = muxyClient.activeProjectID.value
            val needsProjectSwitch = activeID != notification.projectID
            if (needsProjectSwitch) {
                val ok = muxyClient.selectProject(notification.projectID)
                if (!ok) {
                    _errorMessage.value = "Could not open project"
                    return@launch
                }
            } else if (muxyClient.workspace.value == null) {
                muxyClient.refreshWorkspace(notification.projectID)
            }

            val workspace = muxyClient.workspace.value
            if (workspace == null) {
                _errorMessage.value = "Could not load workspace"
                return@launch
            }

            if (workspace.worktreeID != notification.worktreeID) {
                val worktreeKnown = muxyClient.projectWorktrees.value[notification.projectID]
                    ?.any { it.id == notification.worktreeID } == true
                if (!worktreeKnown) {
                    _staleNotificationMessage.value = "This notification points to a closed tab"
                    muxyClient.markNotificationRead(notification.id)
                    return@launch
                }
                val ok = muxyClient.selectWorktree(notification.projectID, notification.worktreeID)
                if (!ok) {
                    _errorMessage.value = "Could not switch worktree"
                    return@launch
                }
            }

            val refreshed = muxyClient.workspace.value
            if (refreshed == null || !areaContainsTab(refreshed, notification.areaID, notification.tabID)) {
                _staleNotificationMessage.value = "This notification points to a closed tab"
                muxyClient.markNotificationRead(notification.id)
                return@launch
            }

            muxyClient.focusArea(projectID = notification.projectID, areaID = notification.areaID)
            muxyClient.selectTab(
                projectID = notification.projectID,
                areaID = notification.areaID,
                tabID = notification.tabID,
            )
            muxyClient.markNotificationRead(notification.id)
            _pendingNavigationProjectID.value = notification.projectID
        }
    }

    fun clearPendingNavigation() {
        _pendingNavigationProjectID.value = null
    }

    fun dismissStaleMessage() {
        _staleNotificationMessage.value = null
    }

    private fun areaContainsTab(workspace: WorkspaceDTO, areaID: UUID, tabID: UUID): Boolean {
        val area = findArea(workspace.root, areaID) ?: return false
        return area.tabs.any { it.id == tabID }
    }

    private fun findArea(node: SplitNodeDTO, areaID: UUID): TabAreaDTO? = when (node) {
        is SplitNodeDTO.TabArea -> if (node.tabArea.id == areaID) node.tabArea else null
        is SplitNodeDTO.Split -> findArea(node.split.first, areaID) ?: findArea(node.split.second, areaID)
    }

    companion object {
        fun factory(muxyClient: MuxyClient): ViewModelProvider.Factory = viewModelFactory {
            initializer { NotificationsViewModel(muxyClient = muxyClient) }
        }
    }
}
