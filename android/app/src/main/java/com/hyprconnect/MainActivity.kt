package com.hyprconnect

import android.content.Intent
import android.net.wifi.WifiManager
import android.os.Bundle
import android.widget.Button
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.hyprconnect_core.DeviceEventCallback
import uniffi.hyprconnect_core.DeviceInfo
import uniffi.hyprconnect_core.confirmPairing
import uniffi.hyprconnect_core.listDevices
import uniffi.hyprconnect_core.pair
import uniffi.hyprconnect_core.registerCallback
import uniffi.hyprconnect_core.setConfigDir
import uniffi.hyprconnect_core.startDiscovery

class MainActivity : AppCompatActivity() {
    private lateinit var statusText: TextView
    private lateinit var deviceAdapter: DeviceAdapter
    private var multicastLock: WifiManager.MulticastLock? = null
    private var deviceRefreshJob: Job? = null

    private val pairingCallback = object : DeviceEventCallback {
        override fun onPairingCode(deviceId: String, code: String) {
            runOnUiThread { showVerificationDialog(deviceId, code) }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        statusText = findViewById(R.id.statusText)
        val recyclerView: RecyclerView = findViewById(R.id.deviceRecyclerView)
        recyclerView.layoutManager = LinearLayoutManager(this)
        deviceAdapter = DeviceAdapter { device -> requestPairing(device) }
        recyclerView.adapter = deviceAdapter

        findViewById<Button>(R.id.refreshButton).setOnClickListener { refreshDevices() }

        val wifiManager = getSystemService(WIFI_SERVICE) as WifiManager
        multicastLock = wifiManager.createMulticastLock("hyprconnect-mdns").apply {
            setReferenceCounted(false)
            acquire()
        }

        startDiscovery()
    }

    override fun onDestroy() {
        deviceRefreshJob?.cancel()
        multicastLock?.let { if (it.isHeld) it.release() }
        multicastLock = null
        super.onDestroy()
    }

    private fun startDiscovery() {
        statusText.setText(R.string.status_starting_discovery)
        lifecycleScope.launch {
            try {
                withContext(Dispatchers.IO) {
                    // Android has no desktop config directory; keep identity and trust
                    // data inside this app's private files directory.
                    setConfigDir(filesDir.resolve("hyprconnect").absolutePath)
                    registerCallback(pairingCallback)
                    // Automatically reconnect only to devices already trusted;
                    // unknown devices still require the manual pairing flow.
                    startDiscovery("HyprConnect Android", "phone", autoPair = true)
                }
                refreshDevices()
                startDeviceRefreshLoop()
            } catch (error: Exception) {
                val errorMessage = error.message ?: getString(R.string.unknown_error)
                statusText.text = getString(R.string.status_discovery_failed, errorMessage)
            }
        }
    }

    private fun startDeviceRefreshLoop() {
        deviceRefreshJob?.cancel()
        deviceRefreshJob = lifecycleScope.launch {
            while (true) {
                delay(2_000)
                refreshDevices(updateStatus = false)
            }
        }
    }

    private fun requestPairing(device: DeviceInfo) {
        if (device.trusted) {
            if (device.online) {
                openFeatures(device)
            } else {
                statusText.setText(R.string.status_device_offline)
            }
            return
        }

        MaterialAlertDialogBuilder(this)
            .setTitle(getString(R.string.pair_device_title, device.deviceName))
            .setMessage(R.string.pair_device_message)
            .setPositiveButton(R.string.pair_action) { _, _ -> beginPairing(device) }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun beginPairing(device: DeviceInfo) {
        statusText.setText(R.string.status_pairing)
        lifecycleScope.launch {
            try {
                withContext(Dispatchers.IO) { pair(device.deviceId) }
                statusText.setText(R.string.status_pairing_complete)
                refreshDevices()
                openFeatures(device.copy(trusted = true, online = true))
            } catch (error: Exception) {
                val message = error.message ?: getString(R.string.unknown_error)
                statusText.text = getString(R.string.status_pairing_failed, message)
            }
        }
    }

    private fun openFeatures(device: DeviceInfo) {
        startActivity(Intent(this, FeaturesActivity::class.java).apply {
            putExtra("device_id", device.deviceId)
            putExtra("device_name", device.deviceName)
        })
    }

    private fun showVerificationDialog(deviceId: String, code: String) {
        MaterialAlertDialogBuilder(this)
            .setTitle(R.string.verify_pairing_title)
            .setMessage(getString(R.string.verify_pairing_message, code))
            .setPositiveButton(R.string.accept_pairing) { _, _ ->
                confirmPairing(deviceId, accepted = true)
            }
            .setNegativeButton(R.string.reject_pairing) { _, _ ->
                confirmPairing(deviceId, accepted = false)
            }
            .setOnCancelListener { confirmPairing(deviceId, accepted = false) }
            .show()
    }

    private fun refreshDevices(updateStatus: Boolean = true) {
        if (updateStatus) {
            statusText.setText(R.string.status_loading_devices)
        }
        lifecycleScope.launch {
            val devices = withContext(Dispatchers.IO) { listDevices() }
            deviceAdapter.submitList(devices)
            if (updateStatus) {
                statusText.text = if (devices.isEmpty()) {
                    getString(R.string.empty_devices)
                } else {
                    getString(R.string.status_devices_found, devices.size)
                }
            }
        }
    }
}
