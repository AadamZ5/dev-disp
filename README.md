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
