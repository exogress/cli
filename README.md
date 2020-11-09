Exogress 
========

![Checks](https://github.com/exogress/cli/workflows/Checks/badge.svg)

Exogress is a swiss army knife for web apps delivery. We help teams deliver their web applications 
with reduced complexity and operational costs. 

This repository contains the code of command-line application.

Installation
============

Each release generates a number of binary artifacts, including Linux, Windows, Mac OS and Docker. 
Binary releases are published under the [releases section](https://github.com/exogress/cli/releases)

## MacOS

Install through homebrew.

```
brew tap exogress/brew
brew udpate
brew install exogress
```

## Linux

Apt packages for Debian and Ubuntu are published to the repository.

```
curl -s https://apt.exogress.com/KEY.gpg | apt-key add -
echo "deb https://apt.exogress.com/ /" > /etc/apt/sources.list.d/exogress.list
apt update
apt install exogress
```

## Windows

Windows binaries are currently available only as a self-contained exe files in the [Releases section](https://github.com/exogress/cli/releases). 

## Docker

See the [blog post](https://blog.exogress.com/exogress-in-docker/)


More info
=========

Please, see more details on the following links:

- Web Page: https://www.exogress.com/
- Developer Documentation: https://developer.exogress.com/
- Blog: https://blog.exogress.com/
