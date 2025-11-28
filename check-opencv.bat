@echo off
REM OpenCV environment setup
set OPENCV_DIR=C:\tools\opencv\build
set OPENCV_INCLUDE_PATHS=C:\tools\opencv\build\include
set OPENCV_LINK_PATHS=C:\tools\opencv\build\x64\vc16\lib
set OPENCV_LINK_LIBS=opencv_world4110
REM LLVM/Clang for opencv-rs bindings generation
set LIBCLANG_PATH=C:\Program Files\LLVM\bin
REM Update PATH
set PATH=%PATH%;C:\tools\opencv\build\x64\vc16\bin;C:\Program Files\LLVM\bin
cd /d "c:\From D\03_Web_Development\realtor-photo-manager\src-tauri"
cargo check
