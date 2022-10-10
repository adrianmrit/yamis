"""Generates a coverage report for the project using kcov."""

import subprocess
import typing

COV_OUT_DIR = "target_cov/cov"
TEST_DIR = "target_cov"

subprocess.run(f"rm -R {TEST_DIR}", shell=True)

print("Generating test binaries, this might take a while...")

result = subprocess.run(
    ["cargo", "test", "--no-run", "--target-dir", f"{TEST_DIR}"],
    check=True,
    capture_output=True,
    text=True
)

result = typing.cast(str, result.stdout + result.stderr)  # The output is unexpectedly in stderr

# Finds executables in the lines
lines = (line.strip() for line in result.split("\n"))
binaries = [line.split(" ")[-1].strip(")(") for line in lines if line.startswith("Executable")]
del lines

print("Target executables:\n{}".format("\n - ".join(binaries)))
if len(binaries) == 0:
    print("No executables found, aborting...")
    exit(1)
print("Running coverage...")

dirs_to_merge = []
for i, binary in enumerate(binaries):
    out = COV_OUT_DIR + "_" + str(i)
    dirs_to_merge.append(out)
    subprocess.run(["kcov", "--verify", "--exclude-pattern=/.cargo,/usr/lib", out, binary], check=True)

subprocess.run(["kcov", "--merge", COV_OUT_DIR, *dirs_to_merge], check=True)
