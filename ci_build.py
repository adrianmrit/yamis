"""
Used in GitHub CI to build the binary, zip it and generate a SHA256 sum.
"""
import hashlib
import os

EXTRA_FILES_TO_ZIP = ["README.md", "LICENSE", "CHANGELOG.md"]
PKG_NAME = "yamis"


def get_sha256_sum(filename):
    """Returns the SHA256 sum of the file."""
    sha256_hash = hashlib.sha256()
    with open(filename, "rb") as f:
        for byte_block in iter(lambda: f.read(4096), b""):
            sha256_hash.update(byte_block)
    return sha256_hash.hexdigest()


def create_hash_file(filepath, final_dir):
    """Creates a file with the SHA256 sum of the file. This mimics the behavior of sha256sum."""
    sha256_hash = get_sha256_sum(filepath)
    filename = os.path.basename(filepath)
    hash_filepath = os.path.join(final_dir, filename + ".sha256")
    with open(hash_filepath, "w") as f:
        f.write(sha256_hash)
        f.write("  ")
        f.write(filename)


def zip_files(binary, version, target, final_dir):
    """Zips the binary and extra files into a zip file."""
    import zipfile

    zip_name = f"{PKG_NAME}-{version}-{target}.zip"
    zip_path = os.path.join(final_dir, zip_name)
    with zipfile.ZipFile(zip_path, "w") as zip_file:
        zip_file.write(binary, arcname=os.path.basename(binary))
        for file in EXTRA_FILES_TO_ZIP:
            zip_file.write(file)
    create_hash_file(zip_path, final_dir)


def tar_files(binary, version, target, final_dir):
    """Zips the binary with tar.gz format and extra files into a zip file."""
    import tarfile

    tar_name = f"{PKG_NAME}-{version}-{target}.tar.gz"
    tar_path = os.path.join(final_dir, tar_name)
    with tarfile.open(tar_path, "w:gz") as tar_file:
        tar_file.add(binary, arcname=os.path.basename(binary))
        for file in EXTRA_FILES_TO_ZIP:
            tar_file.add(file)
    create_hash_file(tar_path, final_dir)


def __main__(target_dir, target, version, final_dir):
    import subprocess

    subprocess.run(
        ["cargo", "build", "--release", "--target", target, "--target-dir", target_dir],
        check=True
    )

    if not os.path.exists(final_dir):
        os.mkdir(final_dir)

    bin_name = PKG_NAME
    if "windows" in target:
        bin_name += ".exe"
    binary_path = os.path.join(target_dir, target, "release", bin_name)

    if "linux" in target:
        tar_files(binary_path, version, target, final_dir)
    else:
        zip_files(binary_path, version, target, final_dir)


if __name__ == "__main__":
    import sys

    __main__(*sys.argv[1:])
