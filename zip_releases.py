"""
Script for collecting releases and zipping the necessary files.
Tested in python 3.9.
"""
import sys
import os
from zipfile import ZipFile

BASE_BINARIES_PATH = "target_releases"
BASE_RELEASE_PATH = "releases"

BINARIES = [
    ("x86_64-apple-darwin", "yamis"),
    ("x86_64-pc-windows-gnu", "yamis.exe"),
    ("x86_64-unknown-linux-gnu", "yamis"),
]
"""Architectures to package. Should match the folder name"""


EXTRA = ["README.md", "LICENSE", "CHANGELOG.MD"]
"""Extra files to package"""


def package_binary(version, architecture, binary):
    binary_path = os.path.join(BASE_BINARIES_PATH, f"v{version}", architecture, "release", binary)
    zip_dir = os.path.join(BASE_RELEASE_PATH, f"v{version}")
    zip_name = f"{architecture}-v{version}.zip"

    os.makedirs(zip_dir, exist_ok=True)

    with ZipFile(os.path.join(zip_dir, zip_name), "w") as zip_obj:
        zip_obj.write(binary_path, binary)
        for extra in EXTRA:
            zip_obj.write(extra)


def main():
    version_to_package = sys.argv[1]
    for arch, binary in BINARIES:
        package_binary(version_to_package, arch, binary)


if __name__ == "__main__":
    main()
