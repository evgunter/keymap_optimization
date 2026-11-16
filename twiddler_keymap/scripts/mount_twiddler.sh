#!/usr/bin/env bash
# (Re)mount the Twiddler's storage at /mnt/twiddler and optionally install a config onto it.
#
# The Twiddler re-enumerates (getting a new /dev/sdX) every time it is power cycled, which leaves any
# existing mount stale, so this always drops the old mount and mounts the device via its stable by-id path.
# The Twiddler must already be attached to WSL with usbipd.
#
# Usage: sudo scripts/mount_twiddler.sh [config.cfg]
set -euo pipefail

readonly device=/dev/disk/by-id/usb-TekGear_Mass_Storage-0:0-part1
readonly mount_point=/mnt/twiddler

if [[ $EUID -ne 0 ]]; then
    echo "must be run as root" >&2
    exit 1
fi
if [[ ! -e $device ]]; then
    echo "$device not found; is the Twiddler attached to WSL (usbipd attach --wsl --busid ...)?" >&2
    exit 1
fi

if mountpoint -q "$mount_point"; then
    umount -l "$mount_point"
fi
mkdir -p "$mount_point"
mount -t vfat -o "uid=${SUDO_UID:-0},gid=${SUDO_GID:-0}" "$device" "$mount_point"
echo "mounted $(readlink -f "$device") at $mount_point"

if [[ $# -ge 1 ]]; then
    cp "$1" "$mount_point/twiddler.cfg"
    sync
    cmp "$1" "$mount_point/twiddler.cfg"
    echo "installed $1 as $mount_point/twiddler.cfg; power cycle the Twiddler to load it"
fi
