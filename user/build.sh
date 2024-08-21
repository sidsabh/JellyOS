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

# Mount the partition
mkdir -p $MNT
sudo mount -t msdos $PARTITION $MNT
    
# Ensure cleanup on exit
trap "sudo umount $MNT; rm -rf $MNT; hdiutil detach $(echo $DEVICE | head -n 1 | awk '{print $1}')" EXIT

# Create the programs directory on the mounted partition
mkdir -p $MNT/programs

# Copy the binaries to the mounted partition
for d in ${PROGS[@]}; do
sudo cp $d/build/$d.bin $MNT/programs/$d.bin
done