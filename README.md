# dev-disp ([#18](https://github.com/AadamZ5/dev-disp/issues/18))

I have an Android device with a screen, why can't I use it as another display for my laptop?!? This repository aims to create a virtual screen-extension utility that can be easily cast to other devices.

![Preview animation](assets/dev-disp-preview.gif)

_(Web PoC pictured above with software encoding)_

## Goal

The goal of this utility is to use the EVDI sub-system developed for the DisplayLink driver to create a virtual display that represents the geometry of the device connected, then transmit that virtual display information to the device via some transport. Device Display will use a companion app to receive high-quality and low-latency data. While this is not targeted at an ultra-low-latency use-case such as gaming, it should
function no worse than something like steam-link.

- [x] Implement Domain via Core Library
- [x] Implement PoC EVDI Subsystem [#25](https://github.com/AadamZ5/dev-disp/issues/25) (thanks to [evdi](https://github.com/dzfranklin/evdi-rs) crate! May need forked or contributed to)
- [x] Implement PoC HEVC Encoding [#21](https://github.com/AadamZ5/dev-disp/issues/21) (thanks to [ffmpeg-next](https://github.com/zmwangx/rust-ffmpeg#readme) crate!)
- [x] Implement PoC Web Test Page [#3](https://github.com/AadamZ5/dev-disp/issues/3)

## General Function

The `dev-disp-server` should run on your host machine, and allow connections from clients via the companion apps. In it's current form, this is hard-coded to accept any websocket connection to this server. In the future, a UI should be implemented with proper handshake and security aspects. ([#26](https://github.com/AadamZ5/dev-disp/issues/26))

## Usage Requirements

For encoding, this project currently uses `ffmpeg`.

High-efficiency encoders usually need access to hardware to aid in encoding. Without any, this will currently fallback to a software CPU encoder like `libx265` or `libx264` which is pretty slow.

## Disclaimer

I don't know if this will be faster than VNC, but I have a big hunch that it will be. If tech like steam-link can transmit games between devices with acceptable latency and image quality (wirelessly!) then we should be able to do this :)

## Running / PoC Web Test Page

Currently, the "server" only supports Linux. Other platforms _may_ be supported in the future.

This barebones implementation implements very loose auto codec negotiation with the server. It is fragile right now and needs a better design server-side.

To build and run the project, you will need the following installed:

- [NodeJS/npm](https://nodejs.org/) (or an equivalent compatible JS runtime and package manager)
- [Rust](https://rust-lang.org/learn/get-started/) programming language
- [EVDI](https://github.com/DisplayLink/evdi?tab=readme-ov-file) Linux module (may be bundled for debain systems)
- [FFmpeg](https://www.ffmpeg.org/about.html) development libraries
  - `libavutil`
  - `libavcodec`
  - `libswscale`
  - `libavformat`
- VP9, VP8, HEVC/H.265, or AVC/H.264 decoder support on your web browser
  - _Note: If you are trying to stream to a different device, browsers like Chrome and Firefox will not allow the use of `VideoDecoders` in insecure contexts (meaning a webpage that is not `https` or `localhost`). You will need to allow specific origins as to be treated as secure ([Chrome](https://stackoverflow.com/a/60983263/7904401), Firefox ???)_

After dependencies, you should:

1.  Install JS dependencies with `npm install`
2.  Serve the angular repo with `npx nx serve web-testpage -- --host 0.0.0.0`
3.  Run the display server with `npx nx run dev-disp-server`
4.  Connect to `http://localhost:4200` and wait for the EVDI module to initialize the display
