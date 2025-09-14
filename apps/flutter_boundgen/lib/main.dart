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
  final usbService = UsbService();
  Stream<MessageToDart>? updateStream;

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
            children: [_buildConnectOrUpdateTree()],
          ),
        ),
      ),
    );
  }

  Widget _buildConnectOrUpdateTree() {
    if (updateStream == null) {
      return _buildConnectTree();
    } else {
      return _buildUpdateTree(updateStream!);
    }
  }

  Widget _buildUpdateTree(Stream<MessageToDart> getScreenStream) {
    return StreamBuilder(
      stream: getScreenStream,
      builder: (context, snapshot) {
        if (snapshot.hasError) {
          return Text('Screen Stream Error: ${snapshot.error}');
        }
        if (snapshot.connectionState == ConnectionState.waiting) {
          return const Text('Waiting for screen data...');
        }
        if (snapshot.hasData) {
          return Text(
            'Screen Info Screen request received ${snapshot.data} at ${DateTime.now()}',
          );
        }
        return const Text('No update data yet.');
      },
    );
  }

  Widget _buildConnectTree() {
    return StreamBuilder(
      stream: UsbService.accessory,
      builder: (context, snapshot) => _buildAccessoryTree(context, snapshot),
    );
  }

  Widget _buildAccessoryTree(
    BuildContext context,
    AsyncSnapshot<Map<String, dynamic>> snapshot,
  ) {
    if (snapshot.hasError) {
      return Text('Accessory Stream Error: ${snapshot.error}');
    }
    if (snapshot.connectionState == ConnectionState.waiting) {
      return const Text('Waiting for accessory events...');
    }
    if (snapshot.hasData) {
      final data = snapshot.data;
      if (data is Map<String, dynamic>) {
        if (data['event'] == 'connect') {
          return _buildAccessoryConnected(context, data);
        } else if (data['event'] == 'disconnect') {
          return _buildAccessoryDisconnected(context);
        } else {
          return Text('Unknown event type: ${data['event']}');
        }
      } else {
        return Text('Unexpected data type: ${data.runtimeType}');
      }
    }
    return const Text('No accessory events yet.');
  }

  Widget _buildAccessoryConnected(
    BuildContext ctx,
    Map<String, dynamic> accessoryData,
  ) {
    final event = accessoryData['event'];
    if (event != 'connect') {
      return const Text('No accessory connected.');
    }

    final fd = accessoryData['fd'];
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Text('Accessory connected'),
        SizedBox(height: 20),
        TextButton(
          onPressed: () => _initialize(fd),
          child: Text("Initialize $fd"),
        ),
      ],
    );
  }

  Widget _buildAccessoryDisconnected(BuildContext ctx) {
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [Text('Accessory disconnected')],
    );
  }

  void _initialize(int fd) async {
    updateStream = initializeStreaming(fd: fd);
    setState(() {});
    print('fd has been given to rust: $fd');
  }
}
