package com.muxy.android.projects

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.muxy.net.MuxyClient
import com.muxy.protocol.dto.GetProjectLogoParams
import com.muxy.protocol.dto.ProjectDTO
import com.muxy.protocol.dto.SelectProjectParams
import com.muxy.protocol.envelope.MuxyMethod
import com.muxy.protocol.envelope.MuxyParams
import com.muxy.protocol.envelope.MuxyResult
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.util.Base64
import java.util.UUID

class ProjectListViewModel(private val muxyClient: MuxyClient) : ViewModel() {
    private val _projects = MutableStateFlow<List<ProjectDTO>>(emptyList())
    val projects: StateFlow<List<ProjectDTO>> = _projects.asStateFlow()

    private val _projectLogos = MutableStateFlow<Map<UUID, ByteArray>>(emptyMap())
    val projectLogos: StateFlow<Map<UUID, ByteArray>> = _projectLogos.asStateFlow()

    private val _isLoading = MutableStateFlow(false)
    val isLoading: StateFlow<Boolean> = _isLoading.asStateFlow()

    private val _errorMessage = MutableStateFlow<String?>(null)
    val errorMessage: StateFlow<String?> = _errorMessage.asStateFlow()

    private val _activeProjectID = MutableStateFlow<UUID?>(null)
    val activeProjectID: StateFlow<UUID?> = _activeProjectID.asStateFlow()

    fun refresh() {
        viewModelScope.launch { loadProjects() }
    }

    fun selectProject(projectID: UUID) {
        viewModelScope.launch {
            val params = MuxyParams.SelectProject(SelectProjectParams(projectID = projectID))
            val response = muxyClient.send(MuxyMethod.SELECT_PROJECT, params)
            if (response == null || response.error != null) {
                _errorMessage.value = response?.error?.message ?: "Could not open project session"
                return@launch
            }
            _activeProjectID.value = projectID
        }
    }

    fun clearActiveProject() {
        _activeProjectID.value = null
    }

    fun disconnect() {
        muxyClient.disconnect()
    }

    private suspend fun loadProjects() {
        _isLoading.value = true
        _errorMessage.value = null
        val response = muxyClient.send(MuxyMethod.LIST_PROJECTS)
        if (response == null) {
            _errorMessage.value = "Could not load projects"
            _isLoading.value = false
            return
        }
        val error = response.error
        if (error != null) {
            _errorMessage.value = "Could not load projects (${error.code})"
            _isLoading.value = false
            return
        }
        val result = response.result
        if (result !is MuxyResult.Projects) {
            _errorMessage.value = "Unexpected response loading projects"
            _isLoading.value = false
            return
        }
        _projects.value = result.value
        _isLoading.value = false
        result.value
            .filter { it.logo != null }
            .forEach { fetchLogo(it.id) }
    }

    private suspend fun fetchLogo(projectID: UUID) {
        if (_projectLogos.value.containsKey(projectID)) return
        val params = MuxyParams.GetProjectLogo(GetProjectLogoParams(projectID = projectID))
        val response = muxyClient.send(MuxyMethod.GET_PROJECT_LOGO, params) ?: return
        val result = response.result as? MuxyResult.ProjectLogo ?: return
        val data = runCatching { Base64.getDecoder().decode(result.value.pngData) }.getOrNull()
            ?: return
        _projectLogos.value = _projectLogos.value + (projectID to data)
    }

    companion object {
        fun factory(muxyClient: MuxyClient): ViewModelProvider.Factory = viewModelFactory {
            initializer { ProjectListViewModel(muxyClient = muxyClient) }
        }
    }
}
