import tkinter as tk
from tkinter import ttk, messagebox
from PIL import Image, ImageTk
import os
import re

class PluginManagerTab(ttk.Frame):
    def __init__(self, parent, theme_var, logger=None):
        super().__init__(parent)
        self.theme_var = theme_var
        self.logger = logger
        
        # Define the base folder path for server plugins
        self.BASE_FOLDER = r"C:\ArkServers\AsaServers"
        
        self.plugins = {}
        self.current_server = tk.StringVar(value='')
        self.background_photo = None
        
        self.setup_ui()
        self.update_theme()

    def setup_ui(self):
        # Main frame
        main_frame = ttk.Frame(self)
        main_frame.pack(fill="both", expand=True, padx=10, pady=10)

        # Server selection frame
        server_frame = ttk.Frame(main_frame)
        server_frame.pack(fill='x', pady=(0, 10))

        ttk.Label(server_frame, text="Select Server:").pack(side='left', padx=(0, 5))
        self.server_combobox = ttk.Combobox(server_frame, state="readonly", values=self.get_server_list())
        self.server_combobox.bind("<<ComboboxSelected>>", self.on_server_selected)
        self.server_combobox.pack(side='left', padx=(0, 10))

        # Plugin list frame
        listbox_frame = ttk.Frame(main_frame)
        listbox_frame.pack(expand=True, fill='both', pady=(0, 10))

        # Create treeview for plugins
        self.plugin_listbox = ttk.Treeview(listbox_frame, columns=('Plugin', 'Status'), show='headings')
        self.plugin_listbox.heading('Plugin', text='Plugin')
        self.plugin_listbox.heading('Status', text='Status')
        self.plugin_listbox.column('Plugin', width=400)
        self.plugin_listbox.column('Status', width=100, anchor='center')
        self.plugin_listbox.pack(side='left', expand=True, fill='both')

        # Scrollbar for treeview
        scrollbar = ttk.Scrollbar(listbox_frame, orient="vertical", command=self.plugin_listbox.yview)
        scrollbar.pack(side="right", fill="y")
        self.plugin_listbox.config(yscrollcommand=scrollbar.set)

        # Button frame
        button_frame = ttk.Frame(main_frame)
        button_frame.pack(fill='x')

        ttk.Button(button_frame, text="Toggle Selected on Current Server", 
                  command=self.toggle_selected_plugin_on_current_server).pack(side='left', padx=(0, 5))
        ttk.Button(button_frame, text="Toggle Selected on All Servers", 
                  command=self.toggle_selected_plugin_on_all_servers).pack(side='left', padx=(0, 5))
        ttk.Button(button_frame, text="Refresh", 
                  command=self.scan_plugins).pack(side='left', padx=(0, 5))

        # Initial scan
        self.scan_plugins()

    def get_server_list(self):
        try:
            if not os.path.exists(self.BASE_FOLDER):
                if self.logger:
                    self.logger.warning(f"ARK servers folder not found: {self.BASE_FOLDER}")
                return []
            return [folder for folder in os.listdir(self.BASE_FOLDER) 
                   if os.path.isdir(os.path.join(self.BASE_FOLDER, folder))]
        except Exception as e:
            if self.logger:
                self.logger.error(f"Failed to list servers: {e}")
            return []

    def on_server_selected(self, event):
        self.current_server.set(self.server_combobox.get())
        self.scan_plugins()

    def scan_plugins(self):
        self.plugins.clear()
        self.plugin_listbox.delete(*self.plugin_listbox.get_children())

        selected_server = self.current_server.get()
        if not selected_server:
            return

        plugin_folder = os.path.join(self.BASE_FOLDER, selected_server, "ShooterGame", "Binaries", "Win64", "ArkApi", "Plugins")
        if os.path.exists(plugin_folder):
            self.load_plugins(plugin_folder)
        else:
            if self.logger:
                self.logger.warning(f"Plugin folder not found: {plugin_folder}")

        for plugin, info in sorted(self.plugins.items()):
            status = "Active" if info["active"] else "Inactive"
            self.plugin_listbox.insert('', 'end', iid=plugin, values=(info["full_name"], status))
            self.update_plugin_colors(plugin)

    def load_plugins(self, plugin_folder):
        try:
            for plugin in os.listdir(plugin_folder):
                plugin_path = os.path.join(plugin_folder, plugin)
                if os.path.isdir(plugin_path):
                    dll_name = self.get_dll_name(plugin_path)
                    if dll_name:
                        self.plugins[plugin] = {
                            "active": plugin == dll_name,
                            "path": plugin_folder,
                            "full_name": plugin,
                            "dll_name": dll_name
                        }
        except Exception as e:
            if self.logger:
                self.logger.error(f"Failed to load plugins: {e}")

    def get_dll_name(self, plugin_path):
        try:
            return next((os.path.splitext(file)[0] for file in os.listdir(plugin_path) if file.endswith('.dll')), None)
        except Exception:
            return None

    def update_plugin_colors(self, plugin):
        tag = 'active' if self.plugins[plugin]["active"] else 'inactive'
        self.plugin_listbox.tag_configure('active', foreground='green')
        self.plugin_listbox.tag_configure('inactive', foreground='red')
        self.plugin_listbox.item(plugin, tags=(tag,))

    def toggle_plugin(self, plugin_info, server_name):
        plugin_path = os.path.join(plugin_info["path"], plugin_info["full_name"])
        new_name = plugin_info["dll_name"] if not plugin_info["active"] else f"{plugin_info['dll_name']}_OFF"
        try:
            os.rename(plugin_path, os.path.join(plugin_info["path"], new_name))
            if self.logger:
                self.logger.info(f"Toggled plugin '{plugin_info['full_name']}' on server '{server_name}'")
        except Exception as e:
            error_msg = f"Failed to rename plugin on server '{server_name}': {e}"
            if self.logger:
                self.logger.error(error_msg)
            messagebox.showerror("Error", error_msg)

    def toggle_selected_plugin_on_current_server(self):
        selection = self.plugin_listbox.selection()
        if not selection:
            messagebox.showinfo("Info", "Please select a plugin to toggle.")
            return

        for item in selection:
            plugin_name = self.plugin_listbox.item(item)['values'][0]
            plugin_info = self.plugins.get(plugin_name)
            if plugin_info:
                self.toggle_plugin(plugin_info, self.current_server.get())

        self.scan_plugins()

    def toggle_selected_plugin_on_all_servers(self):
        selection = self.plugin_listbox.selection()
        if not selection:
            messagebox.showinfo("Info", "Please select a plugin to toggle.")
            return

        for item in selection:
            plugin_name = self.plugin_listbox.item(item)['values'][0]
            plugin_info = self.plugins.get(plugin_name)
            if not plugin_info:
                continue

            for server in self.get_server_list():
                plugin_path = os.path.join(self.BASE_FOLDER, server, "ShooterGame", "Binaries", "Win64", "ArkApi", "Plugins")
                full_plugin_path = os.path.join(plugin_path, plugin_name)

                if os.path.exists(full_plugin_path):
                    current_state = plugin_name == plugin_info["dll_name"]
                    new_name = plugin_info["dll_name"] if not current_state else f"{plugin_info['dll_name']}_OFF"
                    try:
                        os.rename(full_plugin_path, os.path.join(plugin_path, new_name))
                        if self.logger:
                            self.logger.info(f"Toggled plugin '{plugin_name}' on server '{server}'")
                    except Exception as e:
                        error_msg = f"Failed to rename plugin on server '{server}': {e}"
                        if self.logger:
                            self.logger.error(error_msg)
                        messagebox.showerror("Error", error_msg)

        self.scan_plugins()

    def update_theme(self):
        """Update the theme for this tab"""
        # This method will be called when the theme changes
        # You can add theme-specific styling here if needed
        pass

    def log(self, message):
        """Log a message if logger is available"""
        if self.logger:
            self.logger.info(f"[Plugin Manager] {message}")
        else:
            print(f"[Plugin Manager] {message}")
