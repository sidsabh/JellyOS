#!/bin/bash -e

IMG=fs.img
MNT=mnt
PROGS=(sleep fib echo)

# Create the image file
dd if=/dev/zero of=$IMG bs=1m count=128

# Attach the image
DEVICE=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount $IMG | tr -d '[:space:]')

# Now appending 's1' should not introduce unexpected spaces

# Partition the disk and format it as FAT32
diskutil partitionDisk $DEVICE 1 MBR FAT32 "PARTITION" R

PARTITION="${DEVICE}s1"

sudo diskutil unmount $PARTITION

# # Mount the partition
mkdir -p $MNT
mount -t msdos $PARTITION $MNT

# Handle errors and clean up properly
trap "umount $MNT; rm -rf $MNT; hdiutil detach $DEVICE" EXIT

for d in ${PROGS[@]}; do
    (cd $d; make build)
    cp $d/build/$d.bin $MNT/$d
done