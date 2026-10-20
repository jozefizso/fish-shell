"""Build the deterministic local wheel consumed by the real N8 venv/pip test."""
import base64
import csv
import hashlib
import io
import pathlib
import sys
import zipfile


def main():
    destination = pathlib.Path(sys.argv[1])
    dist_info = "fish_n8_local-1.0.dist-info"
    files = {
        "fish_n8_local/__init__.py": b"def message(value):\n    return 'installed:' + value\n",
        f"{dist_info}/METADATA": (
            b"Metadata-Version: 2.1\nName: fish-n8-local\nVersion: 1.0\n\n"
        ),
        f"{dist_info}/WHEEL": (
            b"Wheel-Version: 1.0\nGenerator: fish-native-n8-fixture\n"
            b"Root-Is-Purelib: true\nTag: py3-none-any\n\n"
        ),
    }
    record = io.StringIO(newline="")
    writer = csv.writer(record, lineterminator="\n")
    for name, content in files.items():
        digest = base64.urlsafe_b64encode(hashlib.sha256(content).digest()).rstrip(b"=")
        writer.writerow((name, "sha256=" + digest.decode("ascii"), str(len(content))))
    writer.writerow((f"{dist_info}/RECORD", "", ""))
    files[f"{dist_info}/RECORD"] = record.getvalue().encode("utf-8")
    with zipfile.ZipFile(destination, "x", compression=zipfile.ZIP_STORED) as archive:
        for name, content in files.items():
            member = zipfile.ZipInfo(name, date_time=(2020, 1, 1, 0, 0, 0))
            member.external_attr = 0o644 << 16
            archive.writestr(member, content)


if __name__ == "__main__":
    main()
