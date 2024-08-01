#!/bin/sh

qemu-system-aarch64 \
    -nographic \
    -M raspi3b \
    -serial null -serial pty \
    -serial file:serial_output.log \
    -kernel \
    "$@"