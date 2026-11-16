#!/usr/bin/env python3
"""Release binary floor check: does the canon binary keep the floor its platform declares?

A release binary runs on whichever OS the runner that built it happened to
give it, unless something reads the binary. This reads it (standard library
only, nothing to install, no tool beyond python3) and fails when the floor is
not what the platform's row promises:

  Linux (musl, x86_64 and aarch64)  ELF: static, that is no interpreter
        (PT_INTERP) and no needed shared library (DT_NEEDED); jemalloc is
        inside; the aarch64 build's jemalloc was built with 64K pages.
  macOS (aarch64)                   Mach-O: `minos` is exactly 15.0. Below it
        the deployment target did not take; above it the floor rose unnoticed.
  Windows (x86_64, msvc)            PE: no Visual C++ runtime DLL among the
        imports (VCRUNTIME*, MSVCP*, CONCRT*). The UCRT is part of Windows and
        is not refused.

The release workflows run it on every build row, right after the build:

    python3 scripts/check-release-binary.py --target <triple> <binary>

It also runs on the binary of an npm package, on any OS, to see it pass or fail:

    npm pack @journeykit/canon-core-linux-x64@0.41.0 && tar xzf journeykit-canon-core-linux-x64-0.41.0.tgz
    python3 scripts/check-release-binary.py --target x86_64-unknown-linux-musl package/bin/canon

Every expectation is an option: `--target` picks the defaults of a row and the
options below replace one of them, so a check can be made to fail (or pass) to
see what it reads: `--minos 11.0`, `--linkage dynamic`, `--jemalloc no`,
`--page-size 4096`, `--forbid-dll 'api-ms-win-crt-*'`. Without `--target` the
format is read from the file and its defaults apply.

Exit 0: every check passed. 1: a check failed (each failure names what the
binary has). 2: the arguments or the file cannot be read as the expected format.
"""

from __future__ import annotations

import argparse
import fnmatch
import os
import platform
import re
import struct
import subprocess
import sys
from dataclasses import dataclass, field

EXIT_FAILED = 1
EXIT_USAGE = 2

# ---------------------------------------------------------------- the rows

ELF_MACHINES = {62: "x86_64", 183: "aarch64"}
MACHO_CPUS = {0x0100000C: "arm64", 0x01000007: "x86_64"}
PE_MACHINES = {0x8664: "x86_64", 0xAA64: "aarch64"}

VC_RUNTIME_DLLS = ("VCRUNTIME*", "MSVCP*", "CONCRT*")

# The floor of each release row. `page` is jemalloc's page size in bytes (None: not checked).
TARGETS: dict[str, dict] = {
    "x86_64-unknown-linux-musl": {"format": "elf", "machine": "x86_64", "linkage": "static", "jemalloc": "yes", "page": None},
    "aarch64-unknown-linux-musl": {"format": "elf", "machine": "aarch64", "linkage": "static", "jemalloc": "yes", "page": 65536},
    "aarch64-apple-darwin": {"format": "macho", "machine": "arm64", "minos": "15.0"},
    "x86_64-pc-windows-msvc": {"format": "pe", "machine": "x86_64", "forbid": VC_RUNTIME_DLLS},
}

# What a binary that is not a row's own gets when only its format is known.
FORMAT_DEFAULTS: dict[str, dict] = {
    "elf": {"linkage": "static", "jemalloc": "any", "page": None},
    "macho": {"minos": "15.0"},
    "pe": {"forbid": VC_RUNTIME_DLLS},
}


class NotThisFormat(Exception):
    """The file is not the format that was asked for, or is cut short."""


@dataclass
class Report:
    lines: list[str] = field(default_factory=list)
    failed: bool = False

    def ok(self, text: str) -> None:
        self.lines.append(f"  ok    {text}")

    def fail(self, text: str) -> None:
        self.failed = True
        self.lines.append(f"  FAIL  {text}")

    def info(self, text: str) -> None:
        self.lines.append(f"        {text}")


# ---------------------------------------------------------------- readers


def detect_format(data: bytes) -> str:
    if data[:4] == b"\x7fELF":
        return "elf"
    if data[:4] in (b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe", b"\xfe\xed\xfa\xcf", b"\xce\xfa\xed\xfe"):
        return "macho"
    if data[:2] == b"MZ":
        return "pe"
    return "unknown"


def _cstring(data: bytes, offset: int) -> str:
    if offset < 0 or offset >= len(data):
        raise NotThisFormat(f"a name at offset {offset} lies outside the file")
    end = data.find(b"\0", offset)
    return data[offset : end if end >= 0 else len(data)].decode("latin-1")


def _unpack(fmt: str, data: bytes, offset: int) -> tuple:
    size = struct.calcsize(fmt)
    if offset < 0 or offset + size > len(data):
        raise NotThisFormat(f"the file ends inside a header (offset {offset})")
    return struct.unpack_from(fmt, data, offset)


@dataclass
class Elf:
    machine: str
    interpreter: str | None
    needed: list[str]
    dynamic: bool


def read_elf(data: bytes) -> Elf:
    if data[:4] != b"\x7fELF":
        raise NotThisFormat("not an ELF file")
    if data[4] != 2 or data[5] != 1:
        raise NotThisFormat("not a 64-bit little-endian ELF (the only kind canon ships)")
    (machine,) = _unpack("<H", data, 18)
    (phoff,) = _unpack("<Q", data, 32)
    phentsize, phnum = _unpack("<HH", data, 54)
    loads: list[tuple[int, int, int]] = []  # (vaddr, filesz, offset)
    interpreter: str | None = None
    dynamic_at: tuple[int, int] | None = None
    for index in range(phnum):
        base = phoff + index * phentsize
        p_type, _flags, p_offset, p_vaddr, _paddr, p_filesz, _memsz, _align = _unpack("<IIQQQQQQ", data, base)
        if p_type == 1:  # PT_LOAD
            loads.append((p_vaddr, p_filesz, p_offset))
        elif p_type == 3:  # PT_INTERP
            interpreter = _cstring(data, p_offset)
        elif p_type == 2:  # PT_DYNAMIC
            dynamic_at = (p_offset, p_filesz)

    def file_offset(vaddr: int) -> int:
        for start, size, offset in loads:
            if start <= vaddr < start + size:
                return offset + vaddr - start
        raise NotThisFormat(f"virtual address {vaddr:#x} is in no loaded segment")

    needed: list[str] = []
    if dynamic_at is not None:
        offset, size = dynamic_at
        strtab: int | None = None
        needed_offsets: list[int] = []
        for entry in range(0, size, 16):
            tag, value = _unpack("<qQ", data, offset + entry)
            if tag == 0:  # DT_NULL
                break
            if tag == 1:  # DT_NEEDED
                needed_offsets.append(value)
            elif tag == 5:  # DT_STRTAB
                strtab = value
        if needed_offsets:
            if strtab is None:
                raise NotThisFormat("DT_NEEDED entries and no DT_STRTAB")
            base = file_offset(strtab)
            needed = [_cstring(data, base + value) for value in needed_offsets]
    return Elf(ELF_MACHINES.get(machine, f"machine {machine}"), interpreter, needed, dynamic_at is not None)


def _version(packed: int) -> str:
    major, minor, patch = packed >> 16, (packed >> 8) & 0xFF, packed & 0xFF
    return f"{major}.{minor}" + (f".{patch}" if patch else "")


def parse_version(text: str) -> str:
    """`15`, `15.0` and `15.0.0` are one version; returned in the spelling `_version` prints."""
    parts = text.strip().split(".")
    if not 1 <= len(parts) <= 3 or not all(part.isdigit() for part in parts):
        raise ValueError(f"{text!r} is not a version (15.0)")
    numbers = [int(part) for part in parts] + [0] * (3 - len(parts))
    return _version((numbers[0] << 16) | (numbers[1] << 8) | numbers[2])


@dataclass
class MachoSlice:
    machine: str
    minos: str | None


def _read_macho_slice(data: bytes, start: int) -> MachoSlice:
    (magic,) = _unpack("<I", data, start)
    if magic != 0xFEEDFACF:
        raise NotThisFormat("not a 64-bit little-endian Mach-O (the only kind canon ships)")
    cputype, _sub, _filetype, ncmds, _sizeofcmds, _flags, _reserved = _unpack("<iIIIIII", data, start + 4)
    offset = start + 32
    minos: str | None = None
    for _ in range(ncmds):
        cmd, cmdsize = _unpack("<II", data, offset)
        if cmd == 0x32:  # LC_BUILD_VERSION
            _platform, packed, _sdk, _ntools = _unpack("<IIII", data, offset + 8)
            minos = _version(packed)
        elif cmd == 0x24 and minos is None:  # LC_VERSION_MIN_MACOSX
            packed, _sdk = _unpack("<II", data, offset + 8)
            minos = _version(packed)
        if cmdsize < 8:
            raise NotThisFormat("a load command of size 0")
        offset += cmdsize
    return MachoSlice(MACHO_CPUS.get(cputype & 0xFFFFFFFF, f"cputype {cputype:#x}"), minos)


def read_macho(data: bytes) -> list[MachoSlice]:
    if data[:4] == b"\xca\xfe\xba\xbe":  # a universal binary: one thin Mach-O per architecture
        (count,) = _unpack(">I", data, 4)
        slices = []
        for index in range(count):
            _cpu, _sub, offset, _size, _align = _unpack(">iiIII", data, 8 + index * 20)
            slices.append(_read_macho_slice(data, offset))
        return slices
    return [_read_macho_slice(data, 0)]


@dataclass
class Pe:
    machine: str
    imports: list[str]


def read_pe(data: bytes) -> Pe:
    if data[:2] != b"MZ":
        raise NotThisFormat("not a PE file")
    (pe_at,) = _unpack("<I", data, 0x3C)
    if data[pe_at : pe_at + 4] != b"PE\0\0":
        raise NotThisFormat("no PE signature")
    machine, nsections, _stamp, _symtab, _nsyms, opt_size, _chars = _unpack("<HHIIIHH", data, pe_at + 4)
    optional = pe_at + 24
    (magic,) = _unpack("<H", data, optional)
    if magic == 0x20B:  # PE32+
        dirs_at = optional + 112
    elif magic == 0x10B:  # PE32
        dirs_at = optional + 96
    else:
        raise NotThisFormat(f"unknown optional header magic {magic:#x}")
    (ndirs,) = _unpack("<I", data, dirs_at - 4)
    sections = []
    for index in range(nsections):
        _name, vsize, vaddr, rawsize, rawptr = _unpack("<8sIIII", data, optional + opt_size + index * 40)
        sections.append((vaddr, max(vsize, rawsize), rawptr))

    def file_offset(rva: int) -> int:
        for vaddr, size, rawptr in sections:
            if vaddr <= rva < vaddr + size:
                return rawptr + rva - vaddr
        raise NotThisFormat(f"RVA {rva:#x} is in no section")

    imports: list[str] = []
    # Directory 1 is the import table (20-byte descriptors, the name RVA third from the end), 13 the delay-load
    # imports (32-byte descriptors, the name RVA second): a DLL loaded on first use is still a DLL the binary needs.
    for index, size, name_field in ((1, 20, 12), (13, 32, 4)):
        if index >= ndirs:
            continue
        rva, _length = _unpack("<II", data, dirs_at + index * 8)
        if rva == 0:
            continue
        offset = file_offset(rva)
        while True:
            descriptor = data[offset : offset + size]
            if len(descriptor) < size or not any(descriptor):
                break
            (name_rva,) = struct.unpack_from("<I", descriptor, name_field)
            if name_rva:
                imports.append(_cstring(data, file_offset(name_rva)))
            offset += size
    return Pe(PE_MACHINES.get(machine, f"machine {machine:#x}"), imports)


# ---------------------------------------------------------------- jemalloc

# jemalloc's own diagnostics: strings compiled into every build that links it (strip removes symbols, not these).
JEMALLOC_MARKER = b"<jemalloc>: "
# tikv-jemalloc-sys prefixes the names of jemalloc's entry points and of its environment variable with `_rjem_`.
JEMALLOC_CONF_ENV = "_RJEM_MALLOC_CONF"


def has_jemalloc(data: bytes) -> bool:
    return JEMALLOC_MARKER in data


def jemalloc_page_size(binary: str) -> int:
    """Run the binary once and read the page size jemalloc reports at exit (`stats_print`)."""
    env = dict(os.environ, **{JEMALLOC_CONF_ENV: "stats_print:true"})
    try:
        run = subprocess.run([binary, "--version"], env=env, capture_output=True, text=True, timeout=120, check=False)
    except (OSError, subprocess.SubprocessError) as error:
        raise NotThisFormat(f"cannot run the binary here ({error}); run the check where the binary runs") from error
    match = re.search(r"Page size:\s*(\d+)", run.stdout + run.stderr)
    if not match:
        raise NotThisFormat(f"jemalloc printed no page size (exit {run.returncode}); stderr: {run.stderr.strip()[:300]!r}")
    return int(match.group(1))


# ---------------------------------------------------------------- the checks


def check_elf(path: str, data: bytes, want: dict, report: Report) -> None:
    elf = read_elf(data)
    if want.get("machine") and elf.machine != want["machine"]:
        report.fail(f"machine is {elf.machine}, expected {want['machine']}")
    else:
        report.ok(f"machine {elf.machine}")
    linkage = want["linkage"]
    if linkage != "any":
        is_static = elf.interpreter is None and not elf.needed
        if linkage == "static":
            if is_static:
                report.ok("static: no interpreter (PT_INTERP) and no needed shared library (DT_NEEDED)")
            else:
                if elf.interpreter is not None:
                    report.fail(f"not static: it has an interpreter (PT_INTERP) {elf.interpreter}")
                for name in elf.needed:
                    report.fail(f"not static: it needs the shared library {name} (DT_NEEDED)")
        elif is_static:
            report.fail("expected a dynamic binary (--linkage dynamic) and it is static: no interpreter, no needed shared library")
        else:
            report.ok(f"dynamic, as expected: interpreter {elf.interpreter}, needs {', '.join(elf.needed) or 'nothing'}")
    jemalloc = want["jemalloc"]
    if jemalloc != "any":
        present = has_jemalloc(data)
        if present == (jemalloc == "yes"):
            report.ok("jemalloc is in the binary" if present else "jemalloc is not in the binary, as expected")
        elif present:
            report.fail("jemalloc is in the binary (--jemalloc no)")
        else:
            report.fail("jemalloc is not in the binary: on a musl build that is the allocator lost, and the build is the slow one")
    if want.get("page") is not None:
        try:
            page = jemalloc_page_size(path)
        except NotThisFormat as error:
            report.fail(f"jemalloc page size: {error}")
        else:
            if page == want["page"]:
                report.ok(f"jemalloc page size {page}")
            else:
                report.fail(f"jemalloc page size is {page}, expected {want['page']} (JEMALLOC_SYS_WITH_LG_PAGE did not take)")


def check_macho(data: bytes, want: dict, report: Report) -> None:
    slices = read_macho(data)
    expected = parse_version(want["minos"])
    for one in slices:
        if want.get("machine") and one.machine != want["machine"]:
            report.fail(f"architecture is {one.machine}, expected {want['machine']}")
        else:
            report.ok(f"architecture {one.machine}")
        if one.minos is None:
            report.fail("no minimum OS version in the load commands (LC_BUILD_VERSION, LC_VERSION_MIN_MACOSX)")
        elif one.minos == expected:
            report.ok(f"minos {one.minos}")
        elif parse_tuple(one.minos) < parse_tuple(expected):
            report.fail(f"minos is {one.minos}, expected {expected}: the deployment target did not take (MACOSX_DEPLOYMENT_TARGET)")
        else:
            report.fail(f"minos is {one.minos}, expected {expected}: the floor rose above the declared one")


def parse_tuple(version: str) -> tuple[int, ...]:
    parts = [int(part) for part in version.split(".")]
    return tuple(parts + [0] * (3 - len(parts)))


def check_pe(data: bytes, want: dict, report: Report) -> None:
    pe = read_pe(data)
    if want.get("machine") and pe.machine != want["machine"]:
        report.fail(f"machine is {pe.machine}, expected {want['machine']}")
    else:
        report.ok(f"machine {pe.machine}")
    report.info(f"imports {len(pe.imports)} DLLs: {', '.join(sorted(set(pe.imports), key=str.lower))}")
    refused = sorted({name for name in pe.imports if any(fnmatch.fnmatchcase(name.upper(), pattern.upper()) for pattern in want["forbid"])}, key=str.lower)
    if refused:
        for name in refused:
            why = ": a Windows without the Visual C++ redistributable cannot start it" if any(fnmatch.fnmatchcase(name.upper(), p) for p in VC_RUNTIME_DLLS) else ""
            report.fail(f"it imports {name} (refused: {', '.join(want['forbid'])}){why}")
    else:
        report.ok(f"no import matches {', '.join(want['forbid'])}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Check that a canon release binary keeps the floor its platform declares (ELF static, Mach-O minos, PE imports).",
        epilog="Exit 0: all checks passed; 1: a check failed; 2: usage, or the file is not the expected format.",
    )
    parser.add_argument("binary", help="the canon binary (`canon`, `canon.exe`) to read")
    parser.add_argument("--target", choices=sorted(TARGETS), help="the release row's Rust target: its defaults for every check below")
    parser.add_argument("--linkage", choices=("static", "dynamic", "any"), help="ELF: static (no PT_INTERP, no DT_NEEDED), dynamic, or unchecked")
    parser.add_argument("--jemalloc", choices=("yes", "no", "any"), help="ELF: jemalloc must be in the binary, must not, or unchecked")
    parser.add_argument("--page-size", metavar="BYTES|skip", help="ELF: jemalloc's page size, read by running the binary once (`skip`: do not run it)")
    parser.add_argument("--minos", metavar="X.Y", help="Mach-O: the exact minimum macOS version")
    parser.add_argument("--forbid-dll", action="append", metavar="GLOB", help="PE: refuse an import matching GLOB (repeat; replaces the default VCRUNTIME*, MSVCP*, CONCRT*)")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        with open(args.binary, "rb") as handle:
            data = handle.read()
    except OSError as error:
        print(f"check-release-binary: cannot read {args.binary}: {error}", file=sys.stderr)
        return EXIT_USAGE
    found = detect_format(data)
    row = TARGETS.get(args.target or "")
    fmt = row["format"] if row else found
    if fmt == "unknown":
        print(f"check-release-binary: {args.binary} is not an ELF, Mach-O or PE file", file=sys.stderr)
        return EXIT_USAGE
    want = dict(FORMAT_DEFAULTS[fmt])
    if row:
        want.update(row)
    if args.linkage:
        want["linkage"] = args.linkage
    if args.jemalloc:
        want["jemalloc"] = args.jemalloc
    if args.page_size:
        if args.page_size == "skip":
            want["page"] = None
        elif args.page_size.isdigit():
            want["page"] = int(args.page_size)
        else:
            parser.error("--page-size takes a number of bytes or `skip`")
    if args.minos:
        try:
            want["minos"] = parse_version(args.minos)
        except ValueError as error:
            parser.error(str(error))
    if args.forbid_dll:
        want["forbid"] = tuple(args.forbid_dll)
    # A row's own machine is only checked against the row's own binary.
    if not row:
        want.pop("machine", None)

    report = Report()
    label = args.target or fmt
    if found != fmt:
        print(f"check-release-binary: {args.binary}: expected {fmt.upper()} for {label}, the file is {found}", file=sys.stderr)
        return EXIT_USAGE
    try:
        if fmt == "elf":
            check_elf(args.binary, data, want, report)
        elif fmt == "macho":
            check_macho(data, want, report)
        else:
            check_pe(data, want, report)
    except NotThisFormat as error:
        print(f"check-release-binary: {args.binary}: unreadable as {fmt.upper()}: {error}", file=sys.stderr)
        return EXIT_USAGE
    print(f"check-release-binary: {args.binary} ({label}, host {platform.system()} {platform.machine()})")
    print("\n".join(report.lines))
    if report.failed:
        print(f"check-release-binary: FAILED: {args.binary} does not keep the floor of {label}", file=sys.stderr)
        return EXIT_FAILED
    print("check-release-binary: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
