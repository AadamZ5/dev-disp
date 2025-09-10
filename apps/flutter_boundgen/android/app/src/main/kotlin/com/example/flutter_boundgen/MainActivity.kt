package com.example.flutter_boundgen

import android.content.Context
import android.hardware.display.DisplayManager
import android.os.Build
import android.view.Display
import android.view.WindowManager
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
  private val CHANNEL = "com.example.flutter_boundgen/usb"

  override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
    super.configureFlutterEngine(flutterEngine)
    MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL).setMethodCallHandler {
            call,
            result ->
      if (call.method == "requestUsbPermission") {
        val vid = call.argument<Int>("vid")
        val pid = call.argument<Int>("pid")
        if (vid == null || pid == null) {
          result.error("INVALID_ARGUMENTS", "VID and PID must be provided.", null)
          return@setMethodCallHandler
        }

        val usbHelper = UsbHelper(this@MainActivity)
        val device = usbHelper.findDevice(vid, pid)

        if (device == null) {
          result.error("DEVICE_NOT_FOUND", "USB device not found.", null)
          return@setMethodCallHandler
        }

        usbHelper.requestPermission(device) { fd, message ->
          if (fd != null) {
            result.success(fd)
          } else {
            result.error("PERMISSION_ERROR", message, null)
          }
        }
      } else if (call.method == "listDevices") {
        val usbHelper = UsbHelper(this@MainActivity)
        val devices = usbHelper.listDevices()
        if (devices.isNotEmpty()) {
          val deviceList = devices.map { device ->
            mapOf(
              "vendorId" to device.vendorId,
              "productId" to device.productId,
              "deviceName" to device.deviceName,
              "manufacturerName" to device.manufacturerName,
              "productName" to device.productName,
              "serialNumber" to device.serialNumber
            )
          }
          result.success(deviceList)
        } else {
          result.error("NO_DEVICES", "No USB devices found.", null)
        }
      } else if (call.method == "listAccessories") {
        val usbHelper = UsbHelper(this@MainActivity)
        val accessories = usbHelper.listAccessories()
        if (accessories.isNotEmpty()) {
          val accessoryList = accessories.map { accessory ->
            mapOf(
              "manufacturer" to accessory.manufacturer,
              "model" to accessory.model,
              "version" to accessory.version
            )
          }
          result.success(accessoryList)
        } else {
          result.error("NO_ACCESSORIES", "No USB accessories found.", null)
        }
      } else if (call.method == "edid") {

      } else {
        result.notImplemented()
      }
    }
  }

  fun getDisplayInfo() {

    // Get display refresh rate
    val displayManager = getSystemService(Context.DISPLAY_SERVICE) as DisplayManager;
    val display = displayManager.getDisplay(Display.DEFAULT_DISPLAY);

    val refreshRate = display.refreshRate;

    val windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager;
    val windowMetrics = windowManager.currentWindowMetrics;

    // TODO: Later query insets and determine if we should fit differently
    val currentBounds = mapOf<String, Int>(
      "width" to windowMetrics.bounds.width(),
      "height" to windowMetrics.bounds.height(),
      "refreshRate" to refreshRate.toInt()
    )

  }
}
