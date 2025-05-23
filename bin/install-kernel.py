#!/usr/bin/env python3

import os
import sys
import glob
import hashlib
import subprocess
import shutil
import time

ROOT = os.path.abspath(os.path.dirname(__file__))
CACHE = "/tmp/.cs3210-sdcard-dir.txt"

def load_target_dir():
    global CACHE
    
    dst = None
    if os.path.exists(CACHE):
        dst = open(CACHE).read().strip()
    if len(sys.argv) == 3:
        dst = sys.argv[2]

    vols = "/Volumes/"
    if dst is None:
        os.system(f"ls {vols}")
        print("[!] Please provide a installation directory")
        dst = vols + input("(input) > ").strip()

    if not os.path.isdir(dst):
        print("[!] Please insert your sdcard (mounting point: %s)" % dst)
        print("    waiting", end="", flush=True)

        while not os.path.isdir(dst):
            print(".", end="", flush=True)
            time.sleep(1)
        print()

    with open(CACHE, "w") as fd:
        fd.write(dst)

    return dst

def load_firmware():
    rtn = []
    for pn in glob.glob("%s/../ext/firmware/*" % ROOT):
        rtn.append((os.path.abspath(pn), os.path.basename(pn)))
    return rtn
def load_user_programs():
    rtn = []
    for pn in glob.glob("%s/../user/cache/*" % ROOT):
        rtn.append((os.path.abspath(pn), "programs/"+os.path.basename(pn)))
    return rtn

def load_kernel():
    assert(len(sys.argv) >= 2)

    # check if it's just a directory
    pn = os.path.abspath(sys.argv[1])
    if os.path.isdir(pn):
        pn = os.path.join(pn, "kernel8.img")

    if not os.path.exists(pn):
        print("[!] %s doesn't exist" % pn)
        exit(1)

    (name, ext) = os.path.splitext(pn)
    assert(ext in [".bin", ".elf", ".img"])

    for ext in [".bin", ".img"]:
        pn = name + ext
        if os.path.exists(pn):
            return pn

    raise Exception("Please dump the code from elf first!")

def build_config(kernel):
    (name, ext) = os.path.splitext(kernel)
    assert(ext in [".bin", ".elf", ".img"])

    config = "%s/config.txt" % os.path.dirname(kernel)

    # already have config.txt, like ext/rpi*
    if ext == ".img" and os.path.exists(config):
        return config

    kernel = name + ".elf"
    subprocess.check_call(["%s/gen-rpi3-config.py" % ROOT,
                           kernel],
                          universal_newlines=True)
    return config

def md5sum(pn):
    if not os.path.exists(pn):
        return ""
    md5 = hashlib.new("md5")
    md5.update(open(pn, "rb").read())
    return md5.hexdigest()

def copy_to(src, name, dst):
    assert(os.path.isfile(src))
    assert(os.path.isdir(dst))

    dst = os.path.join(dst, name)
    os.makedirs(os.path.dirname(dst), exist_ok=True)

    hash_dst = md5sum(dst)
    hash_src = md5sum(src)

    if hash_dst != hash_src:
        bak = "%s~" % dst
        if hash_dst != "":
            if os.path.exists(bak):
                os.unlink(bak)
            shutil.move(dst, bak)
        print("[!] %s is updated" % dst)
        shutil.copy2(src, dst)
    else:
        print("[!] %s is up-to-date" % dst)

def clear_sd_card(directory):
    for filename in os.listdir(directory):
        file_path = os.path.join(directory, filename)
        try:
            if os.path.isfile(file_path) or os.path.islink(file_path):
                os.unlink(file_path)
            elif os.path.isdir(file_path):
                shutil.rmtree(file_path)
        except Exception as e:
            print(f'Failed to delete {file_path}. Reason: {e}')

# Use the function in the main logic
if __name__ == '__main__':
    # if len(sys.argv) == 1 or "-h" in sys.argv or "--help" in sys.argv:
    #     print(f"[!] Usage: {sys.argv[0]} [kernel.{bin|elf}] [sdcard directory]?")
    #     print(" NOTE: if the sdcard directory is not provided,")
    #     print("       we will select the directory previously used")
    #     exit(1)
    
    # assert(len(sys.argv) <= 3)

    kernel = load_kernel()
    sdcard = load_target_dir()
    clear_sd_card(sdcard)  # Clearing the SD card before proceeding
    config = build_config(kernel)

    for f in load_firmware() + [(kernel, "kernel8.img"), (config, "config.txt")] + load_user_programs():
        copy_to(*f, sdcard)

    print(f"[!] unmounting {sdcard}")
    os.system("diskutil unmount '%s'" % sdcard)