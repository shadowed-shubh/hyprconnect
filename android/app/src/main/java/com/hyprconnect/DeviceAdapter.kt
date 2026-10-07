package com.hyprconnect

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import uniffi.hyprconnect_core.DeviceInfo

class DeviceAdapter(
    private val onItemClick: ((DeviceInfo) -> Unit)? = null
) : ListAdapter<DeviceInfo, DeviceAdapter.ViewHolder>(DeviceDiffCallback) {

    class ViewHolder(itemView: View) : RecyclerView.ViewHolder(itemView) {
        val deviceName: TextView = itemView.findViewById(R.id.deviceName)
        val deviceDetails: TextView = itemView.findViewById(R.id.deviceDetails)
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ViewHolder {
        val view = LayoutInflater.from(parent.context)
            .inflate(R.layout.item_device, parent, false)
        return ViewHolder(view)
    }

    override fun onBindViewHolder(holder: ViewHolder, position: Int) {
        val item = getItem(position)
        val context = holder.itemView.context
        holder.deviceName.text = item.deviceName

        val trustedStatus = if (item.trusted) {
            context.getString(R.string.trusted_status_trusted)
        } else {
            context.getString(R.string.trusted_status_untrusted)
        }
        val connectionStatus = if (item.online) {
            context.getString(R.string.online_status)
        } else {
            context.getString(R.string.offline_status)
        }

        holder.deviceDetails.text = context.getString(
            R.string.device_details_format,
            item.deviceType,
            "$trustedStatus • $connectionStatus",
            item.deviceId
        )

        holder.itemView.setOnClickListener {
            onItemClick?.invoke(item)
        }
    }

    private object DeviceDiffCallback : DiffUtil.ItemCallback<DeviceInfo>() {
        override fun areItemsTheSame(oldItem: DeviceInfo, newItem: DeviceInfo): Boolean {
            return oldItem.deviceId == newItem.deviceId
        }

        override fun areContentsTheSame(oldItem: DeviceInfo, newItem: DeviceInfo): Boolean {
            return oldItem == newItem
        }
    }
}
