package com.muxy.android.projects

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.muxy.net.MuxyClient
import com.muxy.protocol.dto.ProjectDTO
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.util.UUID

class ProjectListViewModel(private val muxyClient: MuxyClient) : ViewModel() {
    val projects: StateFlow<List<ProjectDTO>> = muxyClient.projects
    val projectLogos: StateFlow<Map<UUID, ByteArray>> = muxyClient.projectLogos

    private val _isLoading = MutableStateFlow(false)
    val isLoading: StateFlow<Boolean> = _isLoading.asStateFlow()

    private val _errorMessage = MutableStateFlow<String?>(null)
    val errorMessage: StateFlow<String?> = _errorMessage.asStateFlow()

    private val _pendingNavigationID = MutableStateFlow<UUID?>(null)
    val pendingNavigationID: StateFlow<UUID?> = _pendingNavigationID.asStateFlow()

    fun refresh() {
        viewModelScope.launch {
            _isLoading.value = true
            _errorMessage.value = null
            val ok = muxyClient.refreshProjects()
            if (!ok) _errorMessage.value = "Could not load projects"
            _isLoading.value = false
        }
    }

    fun selectProject(projectID: UUID) {
        viewModelScope.launch {
            _errorMessage.value = null
            val ok = muxyClient.selectProject(projectID)
            if (!ok) {
                _errorMessage.value = "Could not open project session"
                return@launch
            }
            _pendingNavigationID.value = projectID
        }
    }

    fun clearPendingNavigation() {
        _pendingNavigationID.value = null
    }

    fun disconnect() {
        muxyClient.disconnect()
    }

    companion object {
        fun factory(muxyClient: MuxyClient): ViewModelProvider.Factory = viewModelFactory {
            initializer { ProjectListViewModel(muxyClient = muxyClient) }
        }
    }
}
