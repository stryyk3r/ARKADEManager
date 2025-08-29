# tabs/tab_plugins.py

import os
import shutil
import tkinter as tk
from tkinter import ttk, messagebox, filedialog
from core.theme import ModernTheme, CheckboxWithSymbol

# Root that contains individual server folders
ROOT_SERVERS = r"C:\arkservers\asaservers"
# Tail path where the Plugins folder lives inside each server
PLUGINS_TAIL = os.path.join("ShooterGame", "Binaries", "Win64", "ArkApi", "Plugins")


class PluginsTab(ttk.Frame):
    def __init__(self, parent, theme_var):
        super().__init__(parent)
        self.theme_var = theme_var

        # --- State (initialize BEFORE any discovery calls) ---
        self.left_checkboxes = []        # [(path, var), ...] for source subfolders
        self.right_checkboxes = []       # [(path, var), ...] for destinations
        self.destination_nicknames = {}  # dest_path -> label shown in UI
        self.destinations = []           # list of dest paths

        # --- UI ---
        ttk.Label(self, text="Install Plugins").pack(pady=8)

        # Toolbar at the top for easier access
        toolbar = ttk.Frame(self)
        toolbar.pack(fill="x", padx=8, pady=(0, 8))
        tk.Button(toolbar, text="Browse Source Folder", command=self.browse_source,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side="left", padx=4)
        tk.Button(toolbar, text="Install Selected Plugins", command=self.install_selected,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side="left", padx=4)

        paned = ttk.Panedwindow(self, orient="horizontal")
        paned.pack(fill="both", expand=True, padx=8, pady=8)

        left = ttk.Labelframe(paned, text="Source Folders")
        right = ttk.Labelframe(paned, text="Destination Folders")
        paned.add(left, weight=1)
        paned.add(right, weight=1)

        # Left (sources) with scroll
        self.left_canvas = tk.Canvas(left, highlightthickness=0)
        self.left_scroll = ttk.Scrollbar(left, orient="vertical", command=self.left_canvas.yview)
        self.left_frame = ttk.Frame(self.left_canvas)
        self.left_frame.bind("<Configure>", lambda e: self.left_canvas.configure(scrollregion=self.left_canvas.bbox("all")))
        self.left_canvas.create_window((0, 0), window=self.left_frame, anchor="nw")
        self.left_canvas.configure(yscrollcommand=self.left_scroll.set)
        self.left_canvas.pack(side="left", fill="both", expand=True)
        self.left_scroll.pack(side="right", fill="y")

        # Right (destinations) with scroll
        self.right_canvas = tk.Canvas(right, highlightthickness=0)
        self.right_scroll = ttk.Scrollbar(right, orient="vertical", command=self.right_canvas.yview)
        self.right_frame = ttk.Frame(self.right_canvas)
        self.right_frame.bind("<Configure>", lambda e: self.right_canvas.configure(scrollregion=self.right_canvas.bbox("all")))
        self.right_canvas.create_window((0, 0), window=self.right_frame, anchor="nw")
        self.right_canvas.configure(yscrollcommand=self.right_scroll.set)
        self.right_canvas.pack(side="left", fill="both", expand=True)
        self.right_scroll.pack(side="right", fill="y")

        # Discover destinations now that state & UI exist
        self._load_destinations()

    def update_theme(self):
        """Update theme for the plugins tab"""
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

    # ---------- Discovery & UI population ----------

    def _load_destinations(self):
        self.destinations = []
        self.destination_nicknames.clear()

        base = ROOT_SERVERS
        if os.path.isdir(base):
            for entry in os.scandir(base):
                if not entry.is_dir():
                    continue
                server_name = os.path.basename(entry.path)
                plugins_path = os.path.join(entry.path, PLUGINS_TAIL)

                # 🔻 label should be just the server name
                self.destination_nicknames[plugins_path] = server_name
                self.destinations.append(plugins_path)

        # sort by the short label (server name)
        self.destinations.sort(key=lambda p: self.destination_nicknames.get(p, p).lower())

        self._populate_dest(self.destinations)

    def _populate_dest(self, folders):
        for w in self.right_frame.winfo_children():
            w.destroy()
        self.right_checkboxes.clear()

        for folder in folders:
            var = tk.BooleanVar()
            label = self.destination_nicknames.get(folder, folder)  # <- now just "lna-ragnarok-1"
            chk = CheckboxWithSymbol(self.right_frame, text=label, variable=var)
            chk.pack(anchor="w", fill="x", padx=6, pady=2)
            self.right_checkboxes.append((folder, var))

    # ---------- Sources (left side) ----------

    def browse_source(self):
        chosen = filedialog.askdirectory(title="Select Source Folder")
        if not chosen:
            return

        # Clear old list
        for w in self.left_frame.winfo_children():
            w.destroy()
        self.left_checkboxes.clear()

        # Show each immediate subfolder under chosen (each considered a plugin folder to copy)
        try:
            for name in sorted(os.listdir(chosen)):
                item_path = os.path.join(chosen, name)
                if os.path.isdir(item_path):
                    var = tk.BooleanVar()
                    chk = CheckboxWithSymbol(self.left_frame, text=name, variable=var)
                    chk.pack(anchor="w", fill="x", padx=6, pady=2)
                    self.left_checkboxes.append((item_path, var))
        except Exception as e:
            messagebox.showerror("Error", str(e))

    # ---------- Copy logic ----------

    def install_selected(self):
        sources = [p for p, v in self.left_checkboxes if v.get()]
        dests   = [p for p, v in self.right_checkboxes if v.get()]
        if not sources:
            messagebox.showwarning("No Folder Selected", "Select at least one source folder.")
            return
        if not dests:
            messagebox.showwarning("No Destination Selected", "Select at least one destination.")
            return

        # Ensure the Plugins folders exist
        for d in dests:
            os.makedirs(d, exist_ok=True)

        overwritten_total = 0
        for dest in dests:
            for src in sources:
                # Place each selected source folder under the Plugins dir
                dest_base = os.path.join(dest, os.path.basename(src))
                overwritten_total += self._copy_folder(src, dest_base)

        messagebox.showinfo("Plugins Installed", f"Completed. Files overwritten: {overwritten_total}")

    def _copy_folder(self, src, dest):
        overwritten = 0
        for root, _, files in os.walk(src):
            rel = os.path.relpath(root, src)
            out_dir = os.path.join(dest, rel) if rel != "." else dest
            os.makedirs(out_dir, exist_ok=True)
            for f in files:
                s = os.path.join(root, f)
                d = os.path.join(out_dir, f)
                if os.path.exists(d):
                    overwritten += 1
                shutil.copy2(s, d)
        return overwritten
