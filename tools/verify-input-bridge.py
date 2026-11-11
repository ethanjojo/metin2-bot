"""Exercise the native input ABI without creating devices or sending input."""
import argparse
import ctypes
from pathlib import Path


def verify(path: Path, installed: bool) -> None:
    library = ctypes.CDLL(str(path.resolve()))
    library.mi_abi_version.restype = ctypes.c_uint32
    assert library.mi_abi_version() == 1, "ABI mismatch"
    buffer = ctypes.create_string_buffer(65536)
    pointer = ctypes.c_void_p
    size = ctypes.c_uint32
    handle = ctypes.c_size_t
    library.mi_manage.argtypes = [size, pointer, size]
    library.mi_open.argtypes = [ctypes.POINTER(handle), pointer, size]
    library.mi_keyboard.argtypes = [handle, ctypes.c_uint8, pointer, size, pointer, size]
    library.mi_mouse.argtypes = [handle, ctypes.c_int32, ctypes.c_int32, size, pointer, size]
    library.mi_close.argtypes = [handle, pointer, size]
    assert library.mi_manage(0xFFFFFFFF, buffer, len(buffer)) == 1
    assert b"Unknown" in buffer.value
    assert library.mi_manage(0, None, 0) == 1
    assert library.mi_open(None, buffer, len(buffer)) == 1
    assert b"handle" in buffer.value
    assert library.mi_keyboard(0, 0, None, 7, buffer, len(buffer)) == 1
    assert library.mi_mouse(0, 128, 0, 0, buffer, len(buffer)) == 1
    assert library.mi_mouse(0, 0, 0, 0, buffer, len(buffer)) == 1
    assert library.mi_close(0, buffer, len(buffer)) == 1
    # Short buffers must be bounded and NUL terminated even on an error.
    tiny = ctypes.create_string_buffer(2)
    assert library.mi_close(0, tiny, len(tiny)) == 1
    assert tiny.raw[-1] == 0
    result = library.mi_manage(0, buffer, len(buffer))
    if installed:
        assert result == 0, buffer.value.decode("utf-8", errors="replace")
    else:
        assert result in (0, 1)
    print("PASS: native ABI, bounded errors, invalid handles/reports, read-only driver status")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dll", type=Path)
    parser.add_argument("--installed", action="store_true")
    args = parser.parse_args()
    verify(args.dll, args.installed)
