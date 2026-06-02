"""Run client_win input mode with full crash logging."""
import sys, os, traceback
sys.excepthook = lambda tp, val, tb: (
    open(r"C:\Users\26063\AppData\Local\Temp\crash.log", "a", encoding="utf-8").write(
        "EXCEPTHOOK: " + "".join(traceback.format_exception(tp, val, tb))
    ),
)
import threading
def _thread_excepthook(args):
    open(r"C:\Users\26063\AppData\Local\Temp\crash.log", "a", encoding="utf-8").write(
        "THREAD-EXC: " + "".join(traceback.format_exception(args.exc_type, args.exc_value, args.exc_traceback))
    )
threading.excepthook = _thread_excepthook

# Now exec the main script
import runpy
sys.argv = ["client_win.py", "input"]
runpy.run_path(r"G:\codex-AI-tools\lan-link\client_win.py", run_name="__main__")