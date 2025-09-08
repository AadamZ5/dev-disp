import 'package:flutter/services.dart';

class UsbService {
  static const MethodChannel _channel = MethodChannel(
    'com.example.flutter_boundgen/usb',
  );

  Future<int> requestUsbPermission({required int vid, required int pid}) async {
    try {
      final int? fd = await _channel.invokeMethod('requestUsbPermission', {
        'vid': vid,
        'pid': pid,
      });
      if (fd == null) {
        throw Exception('Failed to get file descriptor.');
      }
      return fd;
    } on PlatformException catch (e) {
      throw Exception('Failed to request USB permission: ${e.message}');
    }
  }

  Future<List<Map<String, dynamic>>> listDevices() async {
    try {
      final List<dynamic>? devices = await _channel.invokeMethod('listDevices');
      if (devices == null) {
        throw Exception('Failed to list USB devices.');
      }
      return devices.cast<Map<String, dynamic>>();
    } on PlatformException catch (e) {
      throw Exception('Failed to list USB devices: ${e.message}');
    }
  }

  Future<List<dynamic>> listAccessories() async {
    try {
      final List<dynamic>? accessories = await _channel.invokeMethod(
        'listAccessories',
      );
      if (accessories == null) {
        throw Exception('Failed to list USB accessories.');
      }
      return accessories;
    } on PlatformException catch (e) {
      throw Exception('Failed to list USB accessories: ${e.message}');
    }
  }
}
