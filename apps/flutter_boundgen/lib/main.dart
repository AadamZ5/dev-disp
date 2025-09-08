import 'package:flutter/material.dart';
import 'package:flutter_boundgen/src/rust/api/simple.dart';
import 'package:flutter_boundgen/src/rust/frb_generated.dart';
import 'package:flutter_boundgen/usb_service.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(const MyApp());
}

class MyApp extends StatefulWidget {
  const MyApp({super.key});

  @override
  State<MyApp> createState() => _MyAppState();
}

class _MyAppState extends State<MyApp> {
  final _usbService = UsbService();
  String _usbResult = 'Press the button to request USB permission.';
  String _devices = 'Press the button to list USB devices.';
  String _accessories = 'Press the button to list USB accessories.';

  Future<void> _requestPermission() async {
    try {
      // TODO: Replace with your device's VID and PID
      final fd = await _usbService.requestUsbPermission(
        vid: 0x1234,
        pid: 0x5678,
      );
      setState(() {
        _usbResult = 'Success! File descriptor: $fd';
      });
    } catch (e) {
      setState(() {
        _usbResult = 'Error: $e';
      });
    }
  }

  Future<void> _listDevices() async {
    try {
      final devices = await _usbService.listDevices();
      setState(() {
        _devices =
            'Available USB devices:\n${devices.map((d) => d.toString()).join('\n')}';
      });
    } catch (e) {
      setState(() {
        _devices = 'Error: $e';
      });
    }
  }

  Future<void> _listAccessories() async {
    try {
      final accessories = await _usbService.listAccessories();
      setState(() {
        _accessories =
            'Available USB accessories:\n${accessories.map((a) => a.toString()).join('\n')}';
      });
    } catch (e) {
      setState(() {
        _accessories = 'Error: $e';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        appBar: AppBar(
          title: const Text('Dev Disp Boundgen Example'),
          foregroundColor: Colors.black,
          backgroundColor: Colors.grey[300],
        ),
        body: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Text(
                'Action: Call Rust `greet("Tom")`\nResult: `${greet(name: "Tom")}`',
              ),
              const SizedBox(height: 20),
              ElevatedButton(
                onPressed: _requestPermission,
                child: const Text('Request USB Permission'),
              ),
              const SizedBox(height: 20),
              Text(_usbResult),
              const SizedBox(height: 40),
              ElevatedButton(
                onPressed: _listDevices,
                child: const Text('List USB devices'),
              ),
              const SizedBox(height: 20),
              Text(_devices),
              const SizedBox(height: 40),
              ElevatedButton(
                onPressed: _listAccessories,
                child: const Text('List USB accessories'),
              ),
              const SizedBox(height: 20),
              Text(_accessories),
            ],
          ),
        ),
      ),
    );
  }
}
