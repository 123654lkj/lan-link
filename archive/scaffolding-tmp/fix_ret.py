path = r"G:\codex-AI-tools\lan-link\crates\input\src\linux.rs"
with open(path, "r", encoding="utf-8") as f:
    c = f.read()
c = c.replace(
    "pub fn write_input_event(fd: i32, ev_type: u16, code: u16, value: i32) -> i32 {",
    "pub fn write_input_event(fd: i32, ev_type: u16, code: u16, value: i32) -> isize {"
)
# Make sure the unsafe block ends with the write call (not the ret it would have been)
c = c.replace(
    "    unsafe { libc::write(fd, buf.as_ptr() as *const libc::c_void, 24) }\n}",
    "    unsafe { libc::write(fd, buf.as_ptr() as *const libc::c_void, 24) }\n}\n"
)
# Remove the inner `unsafe { libc::write(...) }` that ignored return — restore it
c = c.replace(
    """        unsafe { libc::write(fd, buf.as_ptr() as *const libc::c_void, 24); }
}""",
    """        unsafe { libc::write(fd, buf.as_ptr() as *const libc::c_void, 24); }
}"""
)
with open(path, "w", encoding="utf-8", newline="\n") as f:
    f.write(c)
print("Fixed return type")