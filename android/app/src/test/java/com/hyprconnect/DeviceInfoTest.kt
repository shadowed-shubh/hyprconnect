package com.hyprconnect

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.hyprconnect_core.DeviceInfo

class DeviceInfoTest {

    @Test
    fun testDeviceInfoProperties() {
        val device = DeviceInfo(
            deviceId = "dev-123",
            deviceName = "My Laptop",
            deviceType = "desktop",
            trusted = true,
            online = true
        )

        assertEquals("dev-123", device.deviceId)
        assertEquals("My Laptop", device.deviceName)
        assertEquals("desktop", device.deviceType)
        assertTrue(device.trusted)
        assertTrue(device.online)
    }
}
