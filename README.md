# dev-disp ([#18](https://github.com/AadamZ5/dev-disp/issues/18))

I have an Android device with a screen, why can't I use it as another display for my laptop?!? This repository aims to create a virtual screen-extension utility that can be easily cast to other devices.

![Preview animation](assets/dev-disp-preview.gif)

_(Web PoC pictured above with software encoding)_

## Motivation

I have some devices with screens on my desk, like a Steam Deck, or a Galaxy ZFold 4. Why can't I make these beautiful devices extend my main device's screen? I would love a solution that requires minimal setup, and can be controlled from my main device. Plug in a cable or join the network, and initiate "display extending" from my main laptop.

Some devices support this at a hardware level. Devices like the [Lenovo Tab Extreme](https://www.lenovo.com/us/en/p/tablets/android-tablets/lenovo-tab-series/lenovo-tab-extreme/len103l0015#tech_specs) support DisplayPort alt mode input, meaning the integrated display can be used as a display sink by an external device. That's pretty cool! Unfortunately, many devices do not have this sort of IO tech implemented.

The goal of this project is to reduce the friction to get another device to act as a display for your main computer. If we cannot have a plug-and-play solution largely available, we should be able to get to the point of install, click, play. On the main computer, this display should act natively, integrating flawlessly with your OS's desktop layout. In other words, a connected device acting as an extended display should really show up in your display settings on your main computer.

This project is being designed with support for Windows in mind, but is primarily being designed for Linux initially. Also, this project is not aimed at high-performance gaming use, but low latency and visual fidelity is a priority.

While this aims to perform a large portion of what RDP or VNC programs might do, this project has a few priorities that go beyond what existing solutions might offer. We will prioritize:

- **Extensibility** - A network stack doesn't have to be the only way data is transferred. Designed with transport-agnosticism in mind, we can move the data in the most efficient way for the application.
- **Ease-of-use** - The main goal is to get up and running quick. No starting up 3 programs. Just one UI on your main device to click and start casting a virtual screen extension. (After initial setup)
- **Low latency, high fidelity** - Using the latest and greatest openly available codecs and any GPU power avavilable, screen data should be high fidelity and low latency to match the likes of Sunshine/Moonlight, or Steam Link remote play. In potential unique data transports, encoding may not even be needed :)

### Proof of Concept Goals:

- [x] Implement Domain via Core Library
- [x] Implement PoC EVDI Subsystem [#25](https://github.com/AadamZ5/dev-disp/issues/25) (thanks to [evdi](https://github.com/dzfranklin/evdi-rs) crate! May need forked or contributed to)
- [x] Implement PoC HEVC Encoding [#21](https://github.com/AadamZ5/dev-disp/issues/21) (thanks to [ffmpeg-next](https://github.com/zmwangx/rust-ffmpeg#readme) crate!)
- [x] Implement PoC Web Test Page [#3](https://github.com/AadamZ5/dev-disp/issues/3)

### Future Goals:

- Implement proper screen size configuration with EDID protocol [#8](https://github.com/AadamZ5/dev-disp/issues/8)
- Implement Windows IDD screen provider
- Implement WebUSB in web testpage (See concerns in [#33](https://github.com/AadamZ5/dev-disp/issues/33))
- Implement Android companion app

# Usage

⚠️ Hey! This is still in development! It may not be usable for you yet.

Usage notes are not available yet, since this project is still so early in development.

## Runtime Requirements

Only Linux is currently supported. To run on Linux, you will need the [`evdi`](https://github.com/DisplayLink/evdi) module installed. This should be available from your package manager. For efficient encoding, you will want a vulkan-supporting GPU available, or an NVidia, AMD, or Intel GPU compatible with some of `ffmpeg`'s encoders. If you don't have those available and working, the application will fallback to using ffmpeg's `vp9` or `vp8` codecs, which are CPU-based (software encoders) and can be slower.

# Building and Testing

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

# Disclaimer

I don't know if this will be faster than VNC, but I have a big hunch that it will be. If tech like steam-link can transmit games between devices with acceptable latency and image quality (wirelessly!) then we should be able to do this :)
