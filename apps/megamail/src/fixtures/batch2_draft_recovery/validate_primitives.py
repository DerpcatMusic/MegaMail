#!/usr/bin/env python3
"""Synthetic fixture + Linux primitive checks, NOT execution/typechecking of Rust.

Uses a unique TemporaryDirectory only. No profile, keyring, network or provider
access. The parent must run the Rust tests through the production store seam.
"""
import email.parser
import email.policy
import errno
import os
from pathlib import Path
import re
import tempfile

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parents[1] / "draft_recovery.rs"
text = SOURCE.read_text()
for rust_name, python_name in [("NOFOLLOW", "O_NOFOLLOW"), ("DIRECTORY", "O_DIRECTORY"), ("NONBLOCK", "O_NONBLOCK")]:
    value = re.search(rf"const {rust_name}: i32 = 0o([0-7]+);", text)
    assert value and int(value[1], 8) == getattr(os, python_name), rust_name
message = email.parser.BytesParser(policy=email.policy.default).parsebytes((HERE / "private-draft.eml").read_bytes())
assert str(message["Bcc"]) == "Hidden <hidden@example.test>"
assert str(message["Reply-To"]) == "replies@example.test"
attachments = list(message.iter_attachments())
assert len(attachments) == 1
assert attachments[0].get_filename() == "bytes.bin"
assert attachments[0].get_payload(decode=True) == b"binary\0\xff\x80"

with tempfile.TemporaryDirectory(prefix="megamail-f7-primitives-") as temporary:
    root = Path(temporary) / "recovery"
    root.mkdir(mode=0o700)
    root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        anchored = Path(f"/proc/self/fd/{root_fd}")
        final = anchored / "7-abcdefghijklmnopqrstuvwx.draft"
        final.write_bytes(b"known-good")
        final.chmod(0o600)
        pending = anchored / "abcdefghijklmnopqrstuvwx.pending"
        fd = os.open(pending, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "wb") as file:
            file.write(b"new-good")
            file.flush()
            os.fsync(file.fileno())
        # Inject failure before replace. This validates only the intended syscall
        # ordering; it is not a full-disk or process-crash reproduction of Rust.
        try:
            raise OSError(errno.ENOSPC, "synthetic precommit disk-full")
        except OSError:
            pending.unlink()
        assert final.read_bytes() == b"known-good"
        pending.write_bytes(b"new-good")
        pending.chmod(0o600)
        with pending.open("rb") as file:
            os.fsync(file.fileno())
        os.replace(pending, final)
        os.fsync(root_fd)
        assert final.read_bytes() == b"new-good"
        link = anchored / "unsafe-link"
        link.symlink_to(final)
        try:
            fd = os.open(link, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        except OSError as error:
            assert error.errno == errno.ELOOP
        else:
            os.close(fd)
            raise AssertionError("O_NOFOLLOW followed a symlink")
        # A retained dir FD continues to name the original private inode.
        moved = Path(temporary) / "renamed"
        root.rename(moved)
        assert final.read_bytes() == b"new-good"
    finally:
        os.close(root_fd)
print("PASS: MIME fixture bytes/headers; Linux flag constants; private fd/atomic/fsync/no-follow primitive probes")
print("NOT RUN: Rust store/composer tests, typechecking, UI autosave/restore, native/provider send")
