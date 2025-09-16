import tkinter as tk
from tkinter import ttk, END
from tkinter.scrolledtext import ScrolledText
from core.theme import ModernTheme
from core.logger import Logger


class LogFrame(ttk.Frame):
    def __init__(self, parent, theme_var):
        super().__init__(parent)
        self.theme_var = theme_var

        # Create log text widget (read-only)
        self.log_text = ScrolledText(self, height=10, state='disabled')
        self.log_text.pack(fill='both', expand=True, padx=5, pady=5)

        # Configure tags for different message types
        self.log_text.tag_configure("info", foreground="black")
        self.log_text.tag_configure("success", foreground="#90EE90")  # Light green
        self.log_text.tag_configure("monthly", foreground="#00BFFF")  # Deep Sky Blue
        self.log_text.tag_configure("warning", foreground="orange")
        self.log_text.tag_configure("error", foreground="red")

    def add_message(self, message, level="info"):
        """Add a message to the log"""
        # Temporarily enable the widget for writing
        self.log_text.config(state='normal')
        self.log_text.insert(END, message + "\n")
        self.log_text.tag_add(level, "end-2c linestart", "end-1c")
        self.log_text.see(END)  # Auto-scroll to the bottom
        # Disable the widget again to make it read-only
        self.log_text.config(state='disabled')

    def ui_call(self, fn):
        """Marshal a callable to the Tk main thread."""
        self.after(0, fn)

    def update_theme(self):
        """Update the theme colors"""
        colors = ModernTheme.get_colors(self.theme_var.get())

        # Update log text widget colors
        self.log_text.configure(
            bg=colors['entry_bg'],
            fg=colors['entry_fg']
        )

        # Update tag colors based on theme
        self.log_text.tag_configure("info", foreground=colors['text_primary'])
        self.log_text.tag_configure("success", foreground=colors['success'])
        self.log_text.tag_configure("monthly", foreground=colors['accent'])
        self.log_text.tag_configure("warning", foreground=colors['warning'])
        self.log_text.tag_configure("error", foreground=colors['error'])


class LogsTab(LogFrame):
    pass
