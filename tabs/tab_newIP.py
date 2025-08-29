import tkinter as tk
from tkinter import filedialog, messagebox, ttk, font
import json
import os
from core.theme import ModernTheme

class IPUpdaterTab(ttk.Frame):
    def __init__(self, parent, theme_var):
        super().__init__(parent)
        self.theme_var = theme_var
        
        # Set up fonts
        self.title_font = font.Font(family="Segoe UI", size=14, weight="bold")
        self.label_font = font.Font(family="Segoe UI", size=10, weight="bold")
        self.text_font = font.Font(family="Segoe UI", size=9)
        self.button_font = font.Font(family="Segoe UI", size=9, weight="bold")
        
        self.build_gui()

    def build_gui(self):
        # Title
        title_label = ttk.Label(self, text="IP Config Updater", font=self.title_font)
        title_label.pack(pady=(20, 10))

        # Folder path selection frame
        folder_frame = ttk.LabelFrame(self, text="Root Folder Path")
        folder_frame.pack(pady=10, padx=20, fill=tk.X, ipadx=15, ipady=10)
        
        folder_input_frame = ttk.Frame(folder_frame)
        folder_input_frame.pack(fill=tk.X, padx=10, pady=5)
        
        ttk.Label(folder_input_frame, text="Server Root Directory:", font=self.label_font).pack(side=tk.LEFT)
        self.folder_path = ttk.Entry(folder_input_frame, width=50, font=self.text_font)
        self.folder_path.pack(side=tk.LEFT, padx=(10, 5), fill=tk.X, expand=True)
        tk.Button(folder_input_frame, text="Browse", command=self.browse_folder,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side=tk.LEFT)

        # IP input fields frame
        ip_frame = ttk.LabelFrame(self, text="IP Address Configuration")
        ip_frame.pack(pady=10, padx=20, fill=tk.X, ipadx=15, ipady=10)
        
        ip_input_frame = ttk.Frame(ip_frame)
        ip_input_frame.pack(fill=tk.X, padx=10, pady=5)
        
        ttk.Label(ip_input_frame, text="Old IP:", font=self.label_font).grid(row=0, column=0, padx=(0, 5), sticky='w')
        self.old_ip = ttk.Entry(ip_input_frame, width=20, font=self.text_font)
        self.old_ip.grid(row=0, column=1, padx=5)
        
        ttk.Label(ip_input_frame, text="New IP:", font=self.label_font).grid(row=0, column=2, padx=(20, 5), sticky='w')
        self.new_ip = ttk.Entry(ip_input_frame, width=20, font=self.text_font)
        self.new_ip.grid(row=0, column=3, padx=5)
        
        ip_input_frame.grid_columnconfigure(1, weight=1)
        ip_input_frame.grid_columnconfigure(3, weight=1)

        # Update button frame - positioned between config and results
        button_frame = ttk.Frame(self)
        button_frame.pack(pady=15, padx=20, fill=tk.X)
        
        self.update_button = tk.Button(button_frame, text="🔄 Update IP Addresses",
                                        command=self.update_ip_addresses,
                                        relief="raised", borderwidth=3,
                                        bg=self.get_button_color(), fg=self.get_button_text_color(),
                                        activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color())
        self.update_button.pack(pady=10)

        # Results text area frame
        results_frame = ttk.LabelFrame(self, text="Update Results")
        results_frame.pack(pady=10, padx=20, fill=tk.BOTH, expand=True, ipadx=15, ipady=10)
        
        # Create scrollable text widget
        text_frame = ttk.Frame(results_frame)
        text_frame.pack(fill=tk.BOTH, expand=True, padx=10, pady=10)
        
        self.results_text = tk.Text(text_frame, height=12, font=self.text_font, wrap='word')
        scrollbar = ttk.Scrollbar(text_frame, orient="vertical", command=self.results_text.yview)
        self.results_text.configure(yscrollcommand=scrollbar.set)
        
        self.results_text.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        scrollbar.pack(side=tk.RIGHT, fill=tk.Y)

    def browse_folder(self):
        folder_selected = filedialog.askdirectory()
        self.folder_path.delete(0, tk.END)
        self.folder_path.insert(0, folder_selected)

    def update_ip_addresses(self):
        root_path = self.folder_path.get()
        old_ip = self.old_ip.get()
        new_ip = self.new_ip.get()

        if not all([root_path, old_ip, new_ip]):
            messagebox.showerror("Error", "Please fill in all fields")
            return

        self.results_text.delete(1.0, tk.END)
        self.results_text.insert(tk.END, "🔄 Starting IP update process...\n\n")

        try:
            files_modified = 0
            servers_processed = 0
            
            # Get immediate subdirectories in root_path
            for project_dir in next(os.walk(root_path))[1]:
                servers_processed += 1
                self.results_text.insert(tk.END, f"📁 Processing server: {project_dir}\n")
                
                # Construct the path to the Plugins directory
                plugins_path = os.path.join(
                    root_path,
                    project_dir,
                    "ShooterGame",
                    "Binaries",
                    "Win64",
                    "ArkApi",
                    "Plugins"
                )
                
                # Check if the plugins path exists
                if not os.path.exists(plugins_path):
                    self.results_text.insert(tk.END, f"  ⚠️ Skipping {project_dir}: Plugins path not found\n")
                    continue

                server_files_modified = 0
                # Walk through all subdirectories under the Plugins folder
                for root_dir, dirs, files in os.walk(plugins_path):
                    if "config.json" in files:
                        file_path = os.path.join(root_dir, "config.json")
                        try:
                            # Try UTF-8 first
                            try:
                                with open(file_path, 'r', encoding='utf-8') as f:
                                    data = json.load(f)
                            except UnicodeDecodeError:
                                # If UTF-8 fails, try with utf-8-sig (handles BOM)
                                with open(file_path, 'r', encoding='utf-8-sig') as f:
                                    data = json.load(f)
                            
                            # Convert data to string to find and replace IP
                            json_str = json.dumps(data)
                            if old_ip in json_str:
                                json_str = json_str.replace(old_ip, new_ip)
                                data = json.loads(json_str)
                                
                                # Write with UTF-8 encoding
                                with open(file_path, 'w', encoding='utf-8') as f:
                                    json.dump(data, f, indent=4)
                                
                                files_modified += 1
                                server_files_modified += 1
                                self.results_text.insert(tk.END, f"  ✅ Updated: {os.path.relpath(file_path, root_path)}\n")
                                
                        except Exception as e:
                            self.results_text.insert(tk.END, f"  ❌ Error processing {os.path.relpath(file_path, root_path)}: {str(e)}\n")
                
                if server_files_modified == 0:
                    self.results_text.insert(tk.END, f"  ℹ️ No config files found with IP '{old_ip}'\n")
                else:
                    self.results_text.insert(tk.END, f"  📊 Modified {server_files_modified} files in {project_dir}\n")
                
                self.results_text.insert(tk.END, "\n")

            self.results_text.insert(tk.END, f"🎉 Process completed!\n")
            self.results_text.insert(tk.END, f"📊 Summary:\n")
            self.results_text.insert(tk.END, f"  • Servers processed: {servers_processed}\n")
            self.results_text.insert(tk.END, f"  • Total files modified: {files_modified}\n")
            self.results_text.insert(tk.END, f"  • IP changed from '{old_ip}' to '{new_ip}'\n")
            
        except Exception as e:
            messagebox.showerror("Error", f"An error occurred: {str(e)}")
            self.results_text.insert(tk.END, f"❌ Error: {str(e)}\n")

    def update_theme(self):
        """Update theme for the IP updater tab"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        self.results_text.configure(
            bg=colors['entry_bg'],
            fg=colors['entry_fg'],
            insertbackground=colors['fg']
        )
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
            # Update the update button
            if hasattr(self, 'update_button'):
                self.update_button.config(
                    bg=self.get_button_color(),
                    fg=self.get_button_text_color(),
                    activebackground=self.get_button_hover_color(),
                    activeforeground=self.get_button_text_color()
                )
            
            # Find and update other buttons
            for widget in self.winfo_children():
                if hasattr(widget, 'winfo_children'):
                    for child in widget.winfo_children():
                        if hasattr(child, 'winfo_children'):
                            for grandchild in child.winfo_children():
                                if isinstance(grandchild, tk.Button):
                                    grandchild.config(
                                        bg=self.get_button_color(),
                                        fg=self.get_button_text_color(),
                                        activebackground=self.get_button_hover_color(),
                                        activeforeground=self.get_button_text_color()
                                    )
        except Exception as e:
            print(f"Error updating button colors: {e}")