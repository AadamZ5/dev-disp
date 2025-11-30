# dev-disp

I have an Android device with a screen, why can't I use it as another display for my laptop?!? This repository aims to create a virtual screen-extension utility that can be easily cast to other devices.

## Goal

The goal of this utility is to use the EVDI sub-system developed for the DisplayLink driver to create a virtual display that represents the geometry of the device connected, then transmit that virtual display information to the device via some transport. Device Display will use a companion app to receive high-quality and low-latency data. While this is not targeted at an ultra-low-latency use-case such as gaming, it should
function no worse than something like steam-link.

- [x] Implement Domain via Core Library
- [x] Implement PoC EVDI Subsystem [#25](https://github.com/AadamZ5/dev-disp/issues/25) (thanks to [evdi](https://github.com/dzfranklin/evdi-rs) crate! May need forked or contributed to)
- [x] Implement PoC HEVC Encoding [#21](https://github.com/AadamZ5/dev-disp/issues/21) (thanks to [ffmpeg-next](https://github.com/zmwangx/rust-ffmpeg#readme) crate!)
- [ ] Implement PoC Web Test Page [#3](https://github.com/AadamZ5/dev-disp/issues/3)

## General Function

The `dev-disp-server` should run on your host machine, and allow connections from clients via the companion apps. In it's current form, this is hard-coded to accept any websocket connection to this server. In the future, a UI should be implemented with proper handshake and security aspects. ([#26](https://github.com/AadamZ5/dev-disp/issues/26))

## Usage Requirements

For encoding, this project currently uses `ffmpeg`.

High-efficiency encoders usually need access to hardware to aid in encoding. Without any, this will currently fallback to the software CPU encoder `libx265` or `libx264` which is pretty slow.

## Disclaimer

I don't know if this will be faster than VNC, but I have a big hunch that it will be. If tech like steam-link can transmit games between devices with acceptable latency and image quality (wirelessly!) then we should be able to do this :)
