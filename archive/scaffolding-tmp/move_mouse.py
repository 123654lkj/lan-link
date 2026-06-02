import ctypes, time
user32 = ctypes.WinDLL("user32", use_last_error=True)
class POINT(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]
for i in range(5):
    user32.SetCursorPos(100 + i*50, 100 + i*50)
    time.sleep(0.15)
print("done")