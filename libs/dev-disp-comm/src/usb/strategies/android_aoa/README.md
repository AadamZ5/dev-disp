# Android USB to nusb/rusb plan

This has mostly worked so far.

1. The user connects the Android device to the Linux computer
2. On the linux host, the user selects the device (WIP currently hardcoded in dev-disp-server) and the Linux host puts the device in accessory mode
3. Android opens our registered flutter app for the connected device specified in AOA mode. The user may accept the request to open a USB Accessory device, then we pass off the USB device to Flutter.
4. Flutter can pass the USB device to the Rust bindings via a file descriptor (parcelManager / UsbManager.openAccessory)
5. Rust can bang around with the file descriptor
6. Rust MUST close the file descriptor!

Notes for steps 2 - 4:

- https://nvlad1.medium.com/implementing-android-open-accessory-protocol-66cfc59ed240
- https://developer.android.com/reference/android/hardware/usb/UsbAccessory

### Caveats

- Android does _not_ allow Isochronous transfers in AOA mode. When the Android device is acting as an accessory, only bulk transfer. This _might_ be okay if our encoding is efficient enough, but still allows for the possibility of traffic on the USB connection.

### Alternative Approaches

- Use the network to transfer the data stream, which could maybe still be fast enough.
- Implement special Linux mode to enable USB "gadget" drivers for that device, and have the laptop act as a USB accessory. This makes it more of a headache for a Windows port.
