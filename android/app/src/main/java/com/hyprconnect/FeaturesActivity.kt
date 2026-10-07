package com.hyprconnect

import android.os.Bundle
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.lifecycleScope
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.hyprconnect_core.removeTrustedDevice

class FeaturesActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_features)

        val deviceName = intent.getStringExtra("device_name")
            ?: getString(R.string.app_name)
        findViewById<TextView>(R.id.deviceNameText).text = deviceName
        findViewById<TextView>(R.id.trustedText).setText(R.string.trusted_status_trusted)
        findViewById<TextView>(R.id.featureStatusText).text = ""

        findViewById<android.view.View>(R.id.backButton).setOnClickListener { finish() }

        val features = mapOf(
            R.id.clipboardCard to R.string.feature_clipboard,
            R.id.filesCard to R.string.feature_files,
            R.id.notificationsCard to R.string.feature_notifications,
            R.id.commandsCard to R.string.feature_commands,
            R.id.deviceInfoCard to R.string.feature_device_info,
            R.id.settingsCard to R.string.feature_settings,
        )
        val status = findViewById<TextView>(R.id.featureStatusText)
        features.forEach { (viewId, labelId) ->
            findViewById<android.view.View>(viewId).setOnClickListener {
                val label = getString(labelId)
                status.text = getString(R.string.feature_not_implemented, label)
                Toast.makeText(this, status.text, Toast.LENGTH_SHORT).show()
            }
        }
        findViewById<android.view.View>(R.id.removeTrustButton).setOnClickListener {
            showRemoveTrustDialog()
        }
    }

    private fun showRemoveTrustDialog() {
        MaterialAlertDialogBuilder(this)
            .setTitle(R.string.remove_trust_title)
            .setMessage(getString(R.string.remove_trust_message,
                intent.getStringExtra("device_name") ?: getString(R.string.app_name)))
            .setPositiveButton(R.string.remove_trust_action) { _, _ -> removeTrust() }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun removeTrust() {
        val deviceId = intent.getStringExtra("device_id") ?: return
        lifecycleScope.launch {
            try {
                withContext(Dispatchers.IO) { removeTrustedDevice(deviceId) }
                Toast.makeText(this@FeaturesActivity, R.string.remove_trust_complete, Toast.LENGTH_SHORT).show()
                finish()
            } catch (error: Exception) {
                Toast.makeText(
                    this@FeaturesActivity,
                    getString(R.string.remove_trust_failed, error.message ?: getString(R.string.unknown_error)),
                    Toast.LENGTH_LONG,
                ).show()
            }
        }
    }
}
