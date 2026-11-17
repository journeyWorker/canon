#!/usr/bin/env python3
"""Tests of scripts/check-release-binary.py on binaries built here byte by byte (an ELF, a Mach-O, a PE).

Each format has a binary that keeps its row's floor (it passes) and one that does not
(it fails and names what it has: the interpreter and the shared library, the `minos`, the
VC runtime DLL), and the options that change an expectation turn a pass into a failure.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import os
import pathlib
import stat
import struct
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("check_release_binary", ROOT / "check-release-binary.py")
assert SPEC and SPEC.loader
CHECKER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECKER  # dataclasses look their module up here
SPEC.loader.exec_module(CHECKER)


# ---------------------------------------------------------------- byte builders


def build_elf(machine: int = 62, interpreter: str | None = None, needed: tuple[str, ...] = (), dynamic: bool | None = None, extra: bytes = b"") -> bytes:
    """A 64-bit little-endian ELF: one PT_LOAD over the whole file, and PT_INTERP / PT_DYNAMIC when asked for."""
    has_dynamic = bool(interpreter or needed) if dynamic is None else dynamic
    phnum = 1 + (1 if interpreter else 0) + (1 if has_dynamic else 0)
    ph_at, base = 64, 0x400000
    cursor = ph_at + phnum * 56
    interp_bytes = (interpreter.encode() + b"\0") if interpreter else b""
    interp_at = cursor
    cursor += len(interp_bytes)
    strtab = b"\0" + b"".join(name.encode() + b"\0" for name in needed)
    strtab_at = cursor
    cursor += len(strtab)
    cursor = (cursor + 7) & ~7
    dynamic_at = cursor
    entries = []
    offset = 1
    for name in needed:
        entries.append((1, offset))  # DT_NEEDED
        offset += len(name) + 1
    entries.append((5, base + strtab_at))  # DT_STRTAB
    entries.append((0, 0))  # DT_NULL
    dynamic_bytes = b"".join(struct.pack("<qQ", tag, value) for tag, value in entries)
    cursor += len(dynamic_bytes)
    body = bytearray(cursor)
    body[interp_at : interp_at + len(interp_bytes)] = interp_bytes
    body[strtab_at : strtab_at + len(strtab)] = strtab
    body[dynamic_at : dynamic_at + len(dynamic_bytes)] = dynamic_bytes
    body += extra
    header = bytearray(64)
    header[0:4] = b"\x7fELF"
    header[4], header[5], header[6] = 2, 1, 1
    struct.pack_into("<HHI", header, 16, 3, machine, 1)
    struct.pack_into("<Q", header, 32, ph_at)
    struct.pack_into("<HHH", header, 52, 64, 56, phnum)
    phdrs = [struct.pack("<IIQQQQQQ", 1, 5, 0, base, base, len(body), len(body), 0x1000)]
    if interpreter:
        phdrs.append(struct.pack("<IIQQQQQQ", 3, 4, interp_at, base + interp_at, base + interp_at, len(interp_bytes), len(interp_bytes), 1))
    if has_dynamic:
        phdrs.append(struct.pack("<IIQQQQQQ", 2, 6, dynamic_at, base + dynamic_at, base + dynamic_at, len(dynamic_bytes), len(dynamic_bytes), 8))
    body[ph_at : ph_at + len(phdrs) * 56] = b"".join(phdrs)
    body[0:64] = header
    return bytes(body)


def pack_version(version: str) -> int:
    parts = [int(part) for part in version.split(".")] + [0, 0]
    return (parts[0] << 16) | (parts[1] << 8) | parts[2]


def build_macho_slice(minos: str | None, cputype: int = 0x0100000C, legacy: bool = False) -> bytes:
    commands = b""
    if minos is not None and not legacy:
        commands = struct.pack("<IIIIII", 0x32, 24, 1, pack_version(minos), pack_version("15.5"), 0)
    elif minos is not None:
        commands = struct.pack("<IIII", 0x24, 16, pack_version(minos), pack_version("15.5"))
    header = struct.pack("<IiIIIIII", 0xFEEDFACF, cputype, 0, 2, 1 if commands else 0, len(commands), 0, 0)
    return header + commands


def build_macho(minos: str | None, **kwargs: object) -> bytes:
    return build_macho_slice(minos, **kwargs)  # type: ignore[arg-type]


def build_fat(slices: list[bytes]) -> bytes:
    head = struct.pack(">II", 0xCAFEBABE, len(slices))
    offset = 8 + 20 * len(slices)
    table, body = b"", b""
    for item in slices:
        offset = (offset + 15) & ~15
        table += struct.pack(">iiIII", struct.unpack_from("<i", item, 4)[0], 0, offset, len(item), 14)
        body += b"\0" * (offset - 8 - 20 * len(slices) - len(body)) + item
        offset += len(item)
    return head + table + body


def build_pe(imports: tuple[str, ...] = (), delay: tuple[str, ...] = (), machine: int = 0x8664) -> bytes:
    """A PE32+ with one section holding the import descriptors (and delay-load descriptors) and their DLL names."""
    section_rva, section_raw = 0x1000, 0x400
    names = b""
    name_rva = {}
    names_at = (len(imports) + 1) * 20 + (len(delay) + 1) * 32
    for dll in (*imports, *delay):
        name_rva[dll] = section_rva + names_at + len(names)
        names += dll.encode() + b"\0"
    import_table = b"".join(struct.pack("<IIIII", 0, 0, 0, name_rva[dll], 0) for dll in imports) + bytes(20)
    delay_table = b"".join(struct.pack("<IIIIIIII", 1, name_rva[dll], 0, 0, 0, 0, 0, 0) for dll in delay) + bytes(32)
    section_data = import_table + delay_table + names
    pe_at = 0x80
    optional = bytearray(240)
    struct.pack_into("<H", optional, 0, 0x20B)
    struct.pack_into("<I", optional, 108, 16)
    if imports:
        struct.pack_into("<II", optional, 112 + 1 * 8, section_rva, len(import_table))
    if delay:
        struct.pack_into("<II", optional, 112 + 13 * 8, section_rva + len(import_table), len(delay_table))
    coff = struct.pack("<HHIIIHH", machine, 1, 0, 0, 0, len(optional), 0x22)
    section = struct.pack("<8sIIIIIIHHI", b".idata", len(section_data), section_rva, len(section_data), section_raw, 0, 0, 0, 0, 0x40000040)
    image = bytearray(section_raw + len(section_data))
    image[0:2] = b"MZ"
    struct.pack_into("<I", image, 0x3C, pe_at)
    image[pe_at : pe_at + 4] = b"PE\0\0"
    image[pe_at + 4 : pe_at + 24] = coff
    image[pe_at + 24 : pe_at + 24 + len(optional)] = optional
    image[pe_at + 24 + len(optional) : pe_at + 24 + len(optional) + 40] = section
    image[section_raw:] = section_data
    return bytes(image)


# ---------------------------------------------------------------- the tests


class CheckerCase(unittest.TestCase):
    def setUp(self) -> None:
        self._directory = tempfile.TemporaryDirectory()
        self.addCleanup(self._directory.cleanup)
        self.dir = pathlib.Path(self._directory.name)

    def run_checker(self, data: bytes, *args: str) -> tuple[int, str, str]:
        path = self.dir / "canon"
        path.write_bytes(data)
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            try:
                code = CHECKER.main([*args, str(path)])
            except SystemExit as exit_:  # argparse
                code = int(exit_.code or 0)
        return code, out.getvalue(), err.getvalue()


class ElfTests(CheckerCase):
    MUSL = ("--target", "x86_64-unknown-linux-musl")
    JEMALLOC = b"..<jemalloc>: Invalid conf pair: .."

    def test_a_static_binary_with_jemalloc_passes(self) -> None:
        code, out, err = self.run_checker(build_elf(extra=self.JEMALLOC), *self.MUSL)
        self.assertEqual(code, 0, out + err)

    def test_a_static_pie_with_a_dynamic_section_and_no_needed_library_passes(self) -> None:
        code, out, err = self.run_checker(build_elf(dynamic=True, extra=self.JEMALLOC), *self.MUSL)
        self.assertEqual(code, 0, out + err)

    def test_an_interpreter_fails_and_is_named(self) -> None:
        code, out, _ = self.run_checker(build_elf(interpreter="/lib64/ld-linux-x86-64.so.2", extra=self.JEMALLOC), *self.MUSL)
        self.assertEqual(code, 1)
        self.assertIn("/lib64/ld-linux-x86-64.so.2", out)

    def test_a_needed_library_fails_and_is_named(self) -> None:
        code, out, _ = self.run_checker(build_elf(interpreter="/lib/ld.so", needed=("libgcc_s.so.1", "libc.so.6"), extra=self.JEMALLOC), *self.MUSL)
        self.assertEqual(code, 1)
        self.assertIn("libc.so.6", out)
        self.assertIn("libgcc_s.so.1", out)

    def test_a_dynamic_section_alone_with_a_needed_library_and_no_interpreter_fails(self) -> None:
        code, out, _ = self.run_checker(build_elf(needed=("libm.so.6",), dynamic=True, extra=self.JEMALLOC), *self.MUSL)
        self.assertEqual(code, 1)
        self.assertIn("libm.so.6", out)

    def test_a_binary_without_jemalloc_fails(self) -> None:
        code, out, _ = self.run_checker(build_elf(), *self.MUSL)
        self.assertEqual(code, 1)

    def test_the_wrong_machine_fails(self) -> None:
        code, out, _ = self.run_checker(build_elf(machine=183, extra=self.JEMALLOC), "--page-size", "skip", *self.MUSL)
        self.assertEqual(code, 1)

    def test_options_change_the_expectation(self) -> None:
        static = build_elf(extra=self.JEMALLOC)
        code, out, _ = self.run_checker(static, *self.MUSL, "--linkage", "dynamic")
        self.assertEqual(code, 1, out)
        code, out, _ = self.run_checker(static, *self.MUSL, "--jemalloc", "no")
        self.assertEqual(code, 1, out)
        code, out, _ = self.run_checker(build_elf(), *self.MUSL, "--jemalloc", "any")
        self.assertEqual(code, 0, out)
        dynamic = build_elf(interpreter="/lib/ld.so", needed=("libc.so.6",))
        code, out, _ = self.run_checker(dynamic, *self.MUSL, "--linkage", "any", "--jemalloc", "any")
        self.assertEqual(code, 0, out)

    def test_without_a_target_the_format_decides_the_defaults(self) -> None:
        code, out, _ = self.run_checker(build_elf(interpreter="/lib/ld.so", needed=("libc.so.6",)))
        self.assertEqual(code, 1)
        self.assertIn("libc.so.6", out)
        code, out, err = self.run_checker(build_elf())
        self.assertEqual(code, 0, out + err)

    def test_a_truncated_elf_is_unreadable_not_a_pass(self) -> None:
        code, _, err = self.run_checker(build_elf()[:40], *self.MUSL)
        self.assertEqual(code, 2)


class JemallocPageSizeTests(CheckerCase):
    def stub(self, body: str) -> str:
        path = self.dir / "stub"
        path.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)
        return str(path)

    def test_reads_the_page_size_the_binary_prints_at_exit(self) -> None:
        stub = self.stub('[ "$_RJEM_MALLOC_CONF" = stats_print:true ] || exit 9\necho "Arenas:" >&2\necho "Page size: 65536" >&2')
        self.assertEqual(CHECKER.jemalloc_page_size(stub), 65536)

    def test_a_binary_that_prints_no_page_size_is_an_error(self) -> None:
        with self.assertRaises(CHECKER.NotThisFormat):
            CHECKER.jemalloc_page_size(self.stub("echo canon 0.42.0"))

    def test_a_binary_that_cannot_run_is_an_error(self) -> None:
        with self.assertRaises(CHECKER.NotThisFormat):
            CHECKER.jemalloc_page_size(str(self.dir / "missing"))


class MachoTests(CheckerCase):
    ROW = ("--target", "aarch64-apple-darwin")

    def test_minos_15_0_passes(self) -> None:
        code, out, err = self.run_checker(build_macho("15.0"), *self.ROW)
        self.assertEqual(code, 0, out + err)

    def test_minos_below_the_floor_fails_and_says_it(self) -> None:
        code, out, _ = self.run_checker(build_macho("11.0"), *self.ROW)
        self.assertEqual(code, 1)
        self.assertIn("11.0", out)

    def test_minos_above_the_floor_fails_and_says_it(self) -> None:
        code, out, _ = self.run_checker(build_macho("15.1"), *self.ROW)
        self.assertEqual(code, 1)
        self.assertIn("15.1", out)
        code, out, _ = self.run_checker(build_macho("26.0"), *self.ROW)
        self.assertEqual(code, 1)

    def test_the_legacy_version_command_is_read(self) -> None:
        code, out, _ = self.run_checker(build_macho("11.0", legacy=True), *self.ROW)
        self.assertEqual(code, 1)
        self.assertIn("11.0", out)
        code, out, err = self.run_checker(build_macho("15.0", legacy=True), *self.ROW)
        self.assertEqual(code, 0, out + err)

    def test_no_minimum_version_fails(self) -> None:
        code, out, _ = self.run_checker(build_macho(None), *self.ROW)
        self.assertEqual(code, 1)

    def test_the_minos_option_changes_the_expectation(self) -> None:
        code, out, err = self.run_checker(build_macho("11.0"), "--minos", "11.0")
        self.assertEqual(code, 0, out + err)
        code, out, _ = self.run_checker(build_macho("15.0"), *self.ROW, "--minos", "14.0")
        self.assertEqual(code, 1, out)
        code, out, err = self.run_checker(build_macho("15.0"), *self.ROW, "--minos", "15")
        self.assertEqual(code, 0, out + err)

    def test_a_universal_binary_is_checked_slice_by_slice(self) -> None:
        both = build_fat([build_macho_slice("15.0"), build_macho_slice("15.0", cputype=0x01000007)])
        code, out, _ = self.run_checker(both, *self.ROW)
        self.assertEqual(code, 1)
        low = build_fat([build_macho_slice("15.0"), build_macho_slice("11.0")])
        code, out, _ = self.run_checker(low, *self.ROW)
        self.assertEqual(code, 1)
        self.assertIn("11.0", out)


class PeTests(CheckerCase):
    ROW = ("--target", "x86_64-pc-windows-msvc")
    UCRT = ("KERNEL32.dll", "ntdll.dll", "api-ms-win-crt-runtime-l1-1-0.dll", "api-ms-win-crt-stdio-l1-1-0.dll")

    def test_the_ucrt_and_system_dlls_pass(self) -> None:
        code, out, err = self.run_checker(build_pe(self.UCRT), *self.ROW)
        self.assertEqual(code, 0, out + err)

    def test_vcruntime_fails_and_is_named(self) -> None:
        code, out, _ = self.run_checker(build_pe((*self.UCRT, "VCRUNTIME140.dll")), *self.ROW)
        self.assertEqual(code, 1)
        self.assertIn("VCRUNTIME140.dll", out)

    def test_the_msvcp_and_concrt_families_fail(self) -> None:
        for dll in ("MSVCP140.dll", "msvcp140_1.dll", "CONCRT140.dll", "vcruntime140_1.dll"):
            code, out, _ = self.run_checker(build_pe((*self.UCRT, dll)), *self.ROW)
            self.assertEqual(code, 1, dll)
            self.assertIn(dll, out)

    def test_a_delay_loaded_vc_runtime_fails(self) -> None:
        code, out, _ = self.run_checker(build_pe(self.UCRT, delay=("MSVCP140.dll",)), *self.ROW)
        self.assertEqual(code, 1)
        self.assertIn("MSVCP140.dll", out)

    def test_the_forbid_option_replaces_the_list(self) -> None:
        code, out, _ = self.run_checker(build_pe(self.UCRT), *self.ROW, "--forbid-dll", "api-ms-win-crt-*")
        self.assertEqual(code, 1)
        self.assertIn("api-ms-win-crt-runtime-l1-1-0.dll", out)
        code, out, err = self.run_checker(build_pe((*self.UCRT, "VCRUNTIME140.dll")), *self.ROW, "--forbid-dll", "NOTHING*")
        self.assertEqual(code, 0, out + err)

    def test_the_wrong_machine_fails(self) -> None:
        code, out, _ = self.run_checker(build_pe(self.UCRT, machine=0xAA64), *self.ROW)
        self.assertEqual(code, 1)


class FormatTests(CheckerCase):
    def test_a_file_of_the_wrong_format_for_the_target_is_refused(self) -> None:
        code, _, err = self.run_checker(build_macho("15.0"), "--target", "x86_64-unknown-linux-musl")
        self.assertEqual(code, 2)

    def test_a_file_that_is_no_binary_is_refused(self) -> None:
        code, _, err = self.run_checker(b"#!/bin/sh\necho canon\n")
        self.assertEqual(code, 2)

    def test_a_missing_file_is_refused(self) -> None:
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.assertEqual(CHECKER.main([str(self.dir / "missing")]), 2)

    def test_the_targets_are_the_four_release_rows(self) -> None:
        self.assertEqual(
            sorted(CHECKER.TARGETS),
            ["aarch64-apple-darwin", "aarch64-unknown-linux-musl", "x86_64-pc-windows-msvc", "x86_64-unknown-linux-musl"],
        )
        self.assertEqual(CHECKER.TARGETS["aarch64-unknown-linux-musl"]["page"], 65536)
        self.assertIsNone(CHECKER.TARGETS["x86_64-unknown-linux-musl"]["page"])


class VersionTests(unittest.TestCase):
    def test_spellings(self) -> None:
        self.assertEqual(CHECKER.parse_version("15"), "15.0")
        self.assertEqual(CHECKER.parse_version("15.0.0"), "15.0")
        self.assertEqual(CHECKER.parse_version("15.2.1"), "15.2.1")
        with self.assertRaises(ValueError):
            CHECKER.parse_version("fifteen")


if __name__ == "__main__":
    os.environ.pop("_RJEM_MALLOC_CONF", None)
    unittest.main()
