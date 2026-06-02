# Update linux.rs inject_key and inject_mouse to use the new write_input_event
# and return the number of bytes successfully written to uinput.
path = r"G:\codex-AI-tools\lan-link\crates\input\src\linux.rs"
with open(path, "r", encoding="utf-8") as f:
    c = f.read()

# inject_key: return the bytes written
old = """    fn inject_key(&mut self, event: &KeyEvent) {
        if let Some(ref f) = self.uinput_fd {
            let fd = f.as_raw_fd();
            // EV_KEY event
            write_input_event(fd, 1, event.scancode, if event.down { 1 } else { 0 });
            // EV_SYN
            write_input_event(fd, 0, 0, 0);
        }
    }"""
new = """    fn inject_key(&mut self, event: &KeyEvent) -> isize {
        let mut total = 0;
        if let Some(ref f) = self.uinput_fd {
            let fd = f.as_raw_fd();
            total += write_input_event(fd, 1, event.scancode, if event.down { 1 } else { 0 });
            total += write_input_event(fd, 0, 0, 0);
        }
        total
    }"""
c = c.replace(old, new)

# inject_mouse: return bytes written
old = """    fn inject_mouse(&mut self, event: &MouseEvent) {
        if let Some(ref f) = self.uinput_fd {
            let fd = f.as_raw_fd();
            match event {"""
new = """    fn inject_mouse(&mut self, event: &MouseEvent) -> isize {
        let mut total = 0;
        if let Some(ref f) = self.uinput_fd {
            let fd = f.as_raw_fd();
            match event {"""
c = c.replace(old, new)

# Add the total += inside the match
c = c.replace(
    """                MouseEvent::Move { dx, dy, .. } => {
                    write_input_event(fd, 2, 0, *dx); // REL_X
                    write_input_event(fd, 2, 1, *dy); // REL_Y
                }
                MouseEvent::Button { button, down } => {
                    let code = match button {
                        MouseButton::Left => 0x110,
                        MouseButton::Right => 0x111,
                        MouseButton::Middle => 0x112,
                        _ => return,
                    };
                    write_input_event(fd, 1, code, if *down { 1 } else { 0 });
                }
                MouseEvent::Wheel { delta, .. } => {
                    write_input_event(fd, 2, 8, *delta as i32);
                }
            }
            write_input_event(fd, 0, 0, 0); // EV_SYN
        }
    }""",
    """                MouseEvent::Move { dx, dy, .. } => {
                    total += write_input_event(fd, 2, 0, *dx); // REL_X
                    total += write_input_event(fd, 2, 1, *dy); // REL_Y
                }
                MouseEvent::Button { button, down } => {
                    let code = match button {
                        MouseButton::Left => 0x110,
                        MouseButton::Right => 0x111,
                        MouseButton::Middle => 0x112,
                        _ => return total,
                    };
                    total += write_input_event(fd, 1, code, if *down { 1 } else { 0 });
                }
                MouseEvent::Wheel { delta, .. } => {
                    total += write_input_event(fd, 2, 8, *delta as i32);
                }
            }
            total += write_input_event(fd, 0, 0, 0); // EV_SYN
        }
        total
    }"""
)
with open(path, "w", encoding="utf-8", newline="\n") as f:
    f.write(c)
print("linux.rs: inject methods now return bytes written")

# Also update lib.rs trait definition
trait_path = r"G:\codex-AI-tools\lan-link\crates\input\src\lib.rs"
with open(trait_path, "r", encoding="utf-8") as f:
    t = f.read()
t = t.replace(
    "fn inject_key(&mut self, event: &KeyEvent);",
    "fn inject_key(&mut self, event: &KeyEvent) -> isize;"
)
t = t.replace(
    "fn inject_mouse(&mut self, event: &MouseEvent);",
    "fn inject_mouse(&mut self, event: &MouseEvent) -> isize;"
)
# Also need to update win.rs
win_path = r"G:\codex-AI-tools\lan-link\crates\input\src\win.rs"
with open(win_path, "r", encoding="utf-8") as f:
    w = f.read()
w = w.replace(
    "    fn inject_key(&mut self, event: &KeyEvent) {",
    "    fn inject_key(&mut self, event: &KeyEvent) -> isize {"
)
w = w.replace(
    "    fn inject_mouse(&mut self, event: &MouseEvent) {",
    "    fn inject_mouse(&mut self, event: &MouseEvent) -> isize {"
)
# Add `0` at the end of win.rs inject_key
w = w.replace(
    """            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }""",
    """            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
        0
    }"""
)
w = w.replace(
    """            let _ = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
    }
}""",
    """            let _ = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
        0
    }
}"""
)
with open(win_path, "w", encoding="utf-8", newline="\n") as f:
    f.write(w)
print("win.rs: updated trait methods")

with open(trait_path, "w", encoding="utf-8", newline="\n") as f:
    f.write(t)
print("lib.rs: trait updated")