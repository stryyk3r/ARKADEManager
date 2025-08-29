
class Logger:
    _instance = None

    # ANSI color codes
    PINK = '\033[95m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    BLUE = '\033[94m'
    RESET = '\033[0m'

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
            cls._instance.callback = None
            cls._instance.ui_thread_call = None
        return cls._instance

    def set_callback(self, callback, ui_thread_call=None):
        """Set the callback function for logging and a UI-thread marshaller."""
        self.callback = callback
        self.ui_thread_call = ui_thread_call

    def log(self, message, level="info", color=None):
        """Log a message with specified level and color; always marshal to Tk thread if provided."""
        if self.callback:
            colored_message = f"{color}{message}{self.RESET}" if color else message
            if self.ui_thread_call:
                self.ui_thread_call(lambda: self.callback(colored_message, level))
            else:
                self.callback(colored_message, level)

    def info(self, message, color=None):
        self.log(message, "info", color)

    def success(self, message, color=None):
        self.log(message, "success", color)

    def warning(self, message):
        self.log(message, "warning")

    def error(self, message):
        self.log(message, "error")
