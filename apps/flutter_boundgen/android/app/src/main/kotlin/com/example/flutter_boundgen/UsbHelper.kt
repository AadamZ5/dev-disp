package com.example.flutter_boundgen

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbAccessory
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbManager
import android.os.Build

// This was made by Google Gemini, so be skeptical.
class UsbHelper(private val context: Context) {
  private val usbManager = context.getSystemService(Context.USB_SERVICE) as UsbManager

  companion object {
    private const val ACTION_USB_PERMISSION = "com.example.my_app.USB_PERMISSION"
  }

  // Find a device by VID and PID
  fun findDevice(vid: Int, pid: Int): UsbDevice? {
    return usbManager.deviceList.values.find { it.vendorId == vid && it.productId == pid }
  }

  fun listDevices(): List<UsbDevice> {
    return usbManager.deviceList.values.toList()
  }

  fun listAccessories(): List<UsbAccessory> {
    val accessoryList = usbManager.accessoryList
    return accessoryList?.toList() ?: emptyList()
  }

  // Request permission and pass the file descriptor to a callback
  fun requestPermission(device: UsbDevice, onResult: (fd: Int?, message: String) -> Unit) {
    if (usbManager.hasPermission(device)) {
      openDevice(device, onResult)
    } else {
      val permissionIntent =
              PendingIntent.getBroadcast(
                      context,
                      0,
                      Intent(ACTION_USB_PERMISSION),
                      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE
                      else 0
              )
      val filter = IntentFilter(ACTION_USB_PERMISSION)
      val receiver =
              object : BroadcastReceiver() {
                override fun onReceive(context: Context, intent: Intent) {
                  context.unregisterReceiver(this)
                  if (intent.action == ACTION_USB_PERMISSION) {
                    if (intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)) {
                      openDevice(device, onResult)
                    } else {
                      onResult(null, "USB permission denied.")
                    }
                  }
                }
              }
      context.registerReceiver(receiver, filter)
      usbManager.requestPermission(device, permissionIntent)
    }
  }

  private fun openDevice(device: UsbDevice, onResult: (fd: Int?, message: String) -> Unit) {
    val connection = usbManager.openDevice(device)
    if (connection != null) {
      val fd = connection.fileDescriptor
      // IMPORTANT: Do NOT close the connection here.
      // The file descriptor is only valid as long as the connection is open.
      // The Rust side will "own" it. The connection must be kept alive.
      onResult(fd, "Success")
    } else {
      onResult(null, "Failed to open device.")
    }
  }
}
