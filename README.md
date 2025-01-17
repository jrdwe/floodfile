## Floodfile

Unsecure file transfer over arp!

![floodfile tui](assets/interface.jpg)

## Usage 

The application broadcasts files to the network for your friends to download. It gets super unreliable with large files and can be very loud over the network.

1. Select your network interface to share on
2. Enter the path of the file to share.
3. Let your friend download it!

By default the application will save files into the tmp directory. This can be changed via the menu-bar.

## Install

Download the current [release](https://github.com/jrdwe/floodfile/releases/latest) for your operating system

### Linux requirements

Requires the correct network permissions w/ `sudo setcap CAP_NET_RAW+ep floodfile` 

### Windows requirements

Requires either `winpcap` or `npcap with winpcap api` to be installed.

## Credit + Motivation

This project was heavily inspired/adapted by [arpchat](https://github.com/kognise/arpchat) by kognise and was built to help myself learn rust
