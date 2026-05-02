package com.muxy.net

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkRequest
import androidx.annotation.RequiresPermission
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner

class MuxyLifecycleBinder(
    private val client: MuxyClient,
    private val connectivityManager: ConnectivityManager?,
) : DefaultLifecycleObserver {
    private var networkCallback: ConnectivityManager.NetworkCallback? = null

    override fun onStart(owner: LifecycleOwner) {
        client.setBackgrounded(false)
        client.verifyConnectionOrReconnect()
        registerNetworkCallback()
    }

    override fun onStop(owner: LifecycleOwner) {
        client.setBackgrounded(true)
        client.suspendForBackground()
        unregisterNetworkCallback()
    }

    @SuppressLint("MissingPermission")
    @RequiresPermission(Manifest.permission.ACCESS_NETWORK_STATE)
    private fun registerNetworkCallback() {
        val cm = connectivityManager ?: return
        if (networkCallback != null) return
        val request = NetworkRequest.Builder().build()
        val callback =
            object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    client.verifyConnectionOrReconnect()
                }
            }
        runCatching { cm.registerNetworkCallback(request, callback) }
            .onSuccess { networkCallback = callback }
    }

    private fun unregisterNetworkCallback() {
        val cm = connectivityManager ?: return
        val callback = networkCallback ?: return
        runCatching { cm.unregisterNetworkCallback(callback) }
        networkCallback = null
    }

    companion object {
        fun systemConnectivityManager(context: Context): ConnectivityManager? =
            context.applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
    }
}
