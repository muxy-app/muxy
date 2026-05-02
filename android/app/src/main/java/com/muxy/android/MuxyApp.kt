package com.muxy.android

import android.app.Application
import androidx.lifecycle.ProcessLifecycleOwner
import com.muxy.net.ConnectionState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

class MuxyApp : Application() {
    lateinit var container: AppContainer
        private set

    private val applicationScope = CoroutineScope(Dispatchers.Default + SupervisorJob())

    override fun onCreate() {
        super.onCreate()
        container = AppContainer(this)
        ProcessLifecycleOwner.get().lifecycle.addObserver(container.lifecycleBinder)
        observeConnectionForLastSession()
    }

    private fun observeConnectionForLastSession() {
        applicationScope.launch {
            container.muxyClient.state.collectLatest { state ->
                if (state is ConnectionState.Connected) {
                    container.lastSessionStore.saveTarget(state.target)
                }
            }
        }
        applicationScope.launch {
            container.muxyClient.activeProjectID.collectLatest { projectID ->
                container.lastSessionStore.saveActiveProject(projectID)
            }
        }
    }
}
