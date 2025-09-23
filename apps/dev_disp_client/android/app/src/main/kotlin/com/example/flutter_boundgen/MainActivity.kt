package com.example.flutter_boundgen

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.display.DisplayManager
import android.hardware.usb.UsbAccessory
import android.hardware.usb.UsbManager
import android.os.Bundle
import android.os.ParcelFileDescriptor
import android.view.Display
import android.view.WindowManager
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.EventChannel
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
  private val METHOD_CHANNEL = "com.example.flutter_boundgen/usb"
  private var methodChannel: MethodChannel? = null
  private val ACCESSORY_EVENT = "com.example.flutter_boundgen/accessory"
  private var accessoryEvent: EventChannel? = null
  private var accessoryEventSink: EventChannel.EventSink? = null
  private var storedAccessoryData: Map<String, Any>? = null

  private val usbReceiver = object : BroadcastReceiver() {
    override fun onReceive(context: Context?, intent: Intent?) {
      val action = intent?.action;
      if (action == UsbManager.ACTION_USB_ACCESSORY_ATTACHED) {
        handleUsbAccessoryAttached(intent)
      } else if (action == UsbManager.ACTION_USB_ACCESSORY_DETACHED) {
        handleUsbAccessoryDetached(intent)
      }
    }
  }

  override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
    super.configureFlutterEngine(flutterEngine)

    methodChannel = MethodChannel(flutterEngine.dartExecutor.binaryMessenger, METHOD_CHANNEL)
    accessoryEvent = EventChannel(flutterEngine.dartExecutor.binaryMessenger, ACCESSORY_EVENT)

    methodChannel?.setMethodCallHandler { call, result ->
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
          val deviceList =
                  devices.map { device ->
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
          val accessoryList =
                  accessories.map { accessory ->
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
      } else if (call.method == "displayInfo") {
        result.success(getDisplayInfo())
      } else {
        result.notImplemented()
      }
    }

    accessoryEvent?.setStreamHandler(
            object : EventChannel.StreamHandler {
              override fun onListen(arguments: Any?, events: EventChannel.EventSink?) {
                println("ACCESSORY_EVENT_CHANNEL: Flutter started listening")
                accessoryEventSink = events
                if (storedAccessoryData != null) {
                  accessoryEventSink?.success(storedAccessoryData)
                  storedAccessoryData = null
                } else {
                  println("No stored accessory data")
                }
              }
              override fun onCancel(arguments: Any?) {
                println("ACCESSORY_EVENT_CHANNEL: Flutter stopped listening")
                accessoryEventSink = null
              }
            }
    )
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    println("MainActivity onCreate")
    handleIntent(intent)
  }

  override fun onResume() {
    super.onResume();
    val filter = IntentFilter().apply {
      addAction(UsbManager.ACTION_USB_ACCESSORY_ATTACHED)
      addAction(UsbManager.ACTION_USB_ACCESSORY_DETACHED)
    }
    registerReceiver(usbReceiver, filter)
  }

  override fun onPause() {
    super.onPause();
  unregisterReceiver(usbReceiver)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    handleIntent(intent)
  }
  fun getDisplayInfo(): Map<String, Int> {

    // Get display refresh rate
    val displayManager = getSystemService(Context.DISPLAY_SERVICE) as DisplayManager
    val display = displayManager.getDisplay(Display.DEFAULT_DISPLAY)

    val refreshRate = display.refreshRate

    val windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager
    val windowMetrics = windowManager.currentWindowMetrics

    // TODO: Later query insets and determine if we should fit differently
    val currentBounds =
            mapOf<String, Int>(
                    "width" to windowMetrics.bounds.width(),
                    "height" to windowMetrics.bounds.height(),
                    "refreshRate" to refreshRate.toInt()
            )

    return currentBounds
  }

  fun handleIntent(intent: Intent?) {
    if (intent == null) {
      return
    }
    println("Intent: " + intent.action)
    if (intent.action == UsbManager.ACTION_USB_ACCESSORY_ATTACHED) {
      handleUsbAccessoryAttached(intent)
    } else if (intent.action == UsbManager.ACTION_USB_ACCESSORY_DETACHED) {
      handleUsbAccessoryDetached(intent)
    }
  }

  fun handleUsbAccessoryAttached(intent: Intent) {
    val accessory = intent.getParcelableExtra<UsbAccessory>(UsbManager.EXTRA_ACCESSORY)
    if (accessory == null) {
      return
    }

    val usbManager = getSystemService(Context.USB_SERVICE) as UsbManager
    val pc_fd = try {
      usbManager.openAccessory(accessory)
    } catch (e: Exception) {
      return
    }
    val fd = pc_fd.detachFd()
    val accessoryData = mapOf("event" to "connect", "fd" to fd)
    if (accessoryEventSink != null) {
      accessoryEventSink!!.success(accessoryData)
      // TODO: For now close the fd
      pc_fd.close()
    } else {
      storedAccessoryData = accessoryData
    }
  }

  fun handleUsbAccessoryDetached(intent: Intent) {
    val accessoryData = mapOf("event" to "disconnect")
    if (accessoryEventSink == null) {
      storedAccessoryData = accessoryData
    } else {
      accessoryEventSink!!.success(accessoryData)
    }
  }
}
