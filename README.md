# dev-disp

I have an Android device with a screen, why can't I use it as another display for my laptop?!? `dev-disp` is a utility to use your Android device as a screen for your Linux laptop, **without** latent tech like VNC or RDP.

## Goal

The goal of this utility is to use the EVDI sub-system developed for the DisplayLink driver to create a virtual display that represents the geometry of the device connected, then transmit that virtual display information to the device via USB. Device Display will use a companion app
to receive high-quality and low-latency data. While this is not targeted at an ultra-low-latency use-case such as gaming, it should
function no worse than something like steam-link.

## General Function

The `dev-disp-server` should run on your host machine, and allow connections from clients via the companion apps. The initial implementaiton
for data transfer will be a USB 3 connection. There may be provisions for wireless data transfer in the future once I understand the intracacies more :) However, a USB connection should provide a good stable and high-bandwidth connection for the best image quality and latency on the device.

## Confession

I don't know if this will be faster than VNC, but I have a big hunch that it will be. If tech like steam-link can transmit games between devices with acceptable latency and image quality (wirelessly!) then we should be able to do this :)

---

### Flutter + Melos + Rust + Flutter Rust Bindgen

For development, I like to keep an `apps` and `libs` structure (thanks NX tools from JavaScript...)

```text
/
  apps/
    dev-disp-server/ (pure rust)
    dev-disp-android/ (plain flutter)
    flutter_boundgen/ (flutter-bindgen application)
  libs/
    dev-disp-core/ (pure rust)
    dev-disp-flutter-bindgen/ (pure rust)
  pubspec.yaml
  Cargo.toml
  ...
```

For plain flutter apps, you must `cd apps` first before using the app generation template `flutter create`. This will put the app in the right spot in the `apps/` folder. After the app is created, you must update the root `pubspec.yaml` to add the new flutter app as a member, then update the new flutter app's `pubspec.yaml` to add the `resolution: workspace` property.

For the cargo-installed `flutter_rust_bridge_codegen` tool, we can create the same sort of setup:

```shell
cd apps
flutter_rust_bridge_codegen create [app_name] --rust-crate-dir ../../libs/[rust-wrapper-lib]
```

After that, we need to:

- Update the top-level `Cargo.toml` to include the new rust lib as a member of the workspace
- Update the top-level `pubspec.yaml` to include the new flutter app so the Melos tool can see it

And everyone should be happy after that

### Android USB to nusb/rusb plan

I don't know if this will work, but this is the plan so far

1. The user connects the Android device to the Linux computer
2. On the linux host, the user selects the device (WIP currently via terminal dev-disp-server) and the Linux host puts the device in accessory mode
3. Android platform code needs created (Kotlin) for the flutter app to call (when?) that will request user permission to open a USB Accessory device, then pass off the USB device to Flutter.
4. Flutter can pass the USB device to the Rust bindings via a file descriptor
5. Rust can bang around with the file descriptor
6. Rust MUST close the file descriptor!

The backup plan is using network to transfer the data stream, which could maybe still be fast enough.
