# OpenCV DLLs

This folder contains OpenCV DLLs that are bundled with the application.

## Setup

Run the setup script to download and copy the required DLLs:

```powershell
# From project root, run as Administrator
.\scripts\setup-opencv.ps1
```

## Required DLLs

- `opencv_world4100.dll` (or similar version)

These DLLs are automatically copied during the setup process and bundled with the installer.

## Note

The DLLs are not committed to git (see `.gitignore`). Each developer needs to run the setup script.
