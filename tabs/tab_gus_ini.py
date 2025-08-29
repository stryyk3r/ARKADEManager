import os
import tkinter as tk
from tkinter import ttk, messagebox
from core.theme import ModernTheme, CheckboxWithSymbol

ROOT_SERVERS = r"C:\arkservers\asaservers"
CFG_TAIL = os.path.join("ShooterGame", "Saved", "Config", "WindowsServer")
FILENAME = "GameUserSettings.ini"


class GUSIniTab(ttk.Frame):
    def __init__(self, parent, theme_var):
        super().__init__(parent)
        self.theme_var = theme_var

        ttk.Label(self, text="Open GameUserSettings.ini Files").pack(pady=(8, 4))

        # Toolbar
        bar = ttk.Frame(self)
        bar.pack(fill="x", padx=8, pady=(0, 6))
        tk.Button(bar, text="Select All", command=self.select_all,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side="left", padx=4)
        tk.Button(bar, text="Clear All", command=self.clear_all,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side="left", padx=4)
        tk.Button(bar, text="Open Selected", command=self.open_selected,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side="left", padx=12)
        tk.Button(bar, text="Open All", command=self.open_all,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side="left", padx=4)

        # Scrollable checkbox list
        outer = ttk.Frame(self)
        outer.pack(fill="both", expand=True, padx=8, pady=8)

        self.canvas = tk.Canvas(outer, highlightthickness=0)
        self.scroll = ttk.Scrollbar(outer, orient="vertical", command=self.canvas.yview)
        self.inner = ttk.Frame(self.canvas)

        self.inner.bind("<Configure>", lambda e: self.canvas.configure(scrollregion=self.canvas.bbox("all")))
        self.canvas.create_window((0, 0), window=self.inner, anchor="nw")
        self.canvas.configure(yscrollcommand=self.scroll.set)

        self.canvas.pack(side="left", fill="both", expand=True)
        self.scroll.pack(side="right", fill="y")

        # Data
        self.items = []   # [(server, file_path), ...]
        self.vars = {}    # server -> tk.BooleanVar

        self.discover()
        self.populate()

    def discover(self):
        items = []
        if os.path.isdir(ROOT_SERVERS):
            for entry in os.scandir(ROOT_SERVERS):
                if not entry.is_dir():
                    continue
                server = os.path.basename(entry.path)
                file_path = os.path.join(entry.path, CFG_TAIL, FILENAME)
                if os.path.isfile(file_path):
                    items.append((server, file_path))
        self.items = sorted(items, key=lambda t: t[0].lower())

    def populate(self):
        for w in self.inner.winfo_children():
            w.destroy()
        self.vars.clear()

        for server, _ in self.items:
            v = tk.BooleanVar(value=False)
            chk = CheckboxWithSymbol(self.inner, text=server, variable=v)
            chk.pack(anchor="w", fill="x", padx=6, pady=2)
            self.vars[server] = v

    def select_all(self):
        for v in self.vars.values():
            v.set(True)

    def clear_all(self):
        for v in self.vars.values():
            v.set(False)

    def _selected_paths(self):
        selected = []
        chosen = {s for s, v in self.vars.items() if v.get()}
        if not chosen:
            return selected
        for server, path in self.items:
            if server in chosen:
                selected.append(path)
        return selected

    def open_selected(self):
        paths = self._selected_paths()
        if not paths:
            messagebox.showwarning("No Selection", "Check at least one server.")
            return
        opened = 0
        for p in paths:
            try:
                os.startfile(p)
                opened += 1
            except Exception:
                pass
        if opened:
            messagebox.showinfo("Opened", f"Opened {opened} {FILENAME} file(s).")
        else:
            messagebox.showwarning("None Opened", f"No {FILENAME} files were opened.")

    def open_all(self):
        opened = 0
        for _, p in self.items:
            try:
                os.startfile(p)
                opened += 1
            except Exception:
                pass
        if opened:
            messagebox.showinfo("Opened", f"Opened {opened} {FILENAME} file(s).")
        else:
            messagebox.showwarning("None Found", f"No {FILENAME} files found under servers.")

    def update_theme(self):
        """Update theme for the GUS ini tab"""
        # Update button colors
        self.update_button_colors()

    def get_button_color(self):
        """Get button background color based on current theme"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        return colors['button_bg']
        
    def get_button_text_color(self):
        """Get button text color based on current theme"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        return colors['button_fg']
        
    def get_button_hover_color(self):
        """Get button hover color based on current theme"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        return colors['hover']
        
    def update_button_colors(self):
        """Update colors of all action buttons"""
        try:
            # Recursively find and update all tk.Button widgets
            def update_buttons_recursive(widget):
                if isinstance(widget, tk.Button):
                    widget.config(
                        bg=self.get_button_color(),
                        fg=self.get_button_text_color(),
                        activebackground=self.get_button_hover_color(),
                        activeforeground=self.get_button_text_color()
                    )
                elif hasattr(widget, 'winfo_children'):
                    for child in widget.winfo_children():
                        update_buttons_recursive(child)
            
            # Start from the root widget
            update_buttons_recursive(self)
        except Exception as e:
            print(f"Error updating button colors: {e}")
