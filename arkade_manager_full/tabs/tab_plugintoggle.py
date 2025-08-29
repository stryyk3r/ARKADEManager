import os
import stat
import configparser
import tkinter as tk
from tkinter import *
from tkinter import ttk, font
from core.theme import ModernTheme, CheckboxWithSymbol

FILES = ["Game.ini", "GameUserSettings.ini"]
CONFIG_RELATIVE_PATH = os.path.join("ShooterGame", "Saved", "Config", "WindowsServer")

# Color scheme - Dark purple and blue theme
COLORS = {
    'bg_dark': '#1a1a2e',
    'bg_medium': '#16213e',
    'bg_light': '#0f3460',
    'accent_purple': '#7209b7',
    'accent_blue': '#4a90e2',
    'text_light': '#ffffff',
    'text_gray': '#b8b8b8',
    'button_hover': '#8e44ad',
    'success': '#27ae60',
    'error': '#e74c3c'
}

class PluginToggleTab(ttk.Frame):
    def __init__(self, parent, theme_var):
        super().__init__(parent)
        self.theme_var = theme_var
        
        self.base_dir = "C:/arkservers/asaservers"
        self.file_vars = {f: IntVar(value=1) for f in FILES}
        self.server_vars = {}
        
        # Set up fonts
        self.title_font = font.Font(family="Segoe UI", size=14, weight="bold")
        self.label_font = font.Font(family="Segoe UI", size=10, weight="bold")
        self.text_font = font.Font(family="Segoe UI", size=9)
        self.button_font = font.Font(family="Segoe UI", size=9, weight="bold")

        self.build_gui()
        self.load_server_folders()

    def build_gui(self):
        # Create a canvas with scrollbar for the main content
        canvas = Canvas(self, highlightthickness=0, borderwidth=0)
        scrollbar = ttk.Scrollbar(self, orient="vertical", command=canvas.yview)
        scrollable_frame = ttk.Frame(canvas)

        scrollable_frame.bind(
            "<Configure>",
            lambda e: canvas.configure(scrollregion=canvas.bbox("all"))
        )

        canvas.create_window((0, 0), window=scrollable_frame, anchor="nw")
        canvas.configure(yscrollcommand=scrollbar.set)

        # Main container
        main_frame = ttk.Frame(scrollable_frame)
        main_frame.pack(fill='both', expand=True, padx=20, pady=20)

        # Title
        title_label = ttk.Label(main_frame, text="ARK Server Config Toggler", font=self.title_font)
        title_label.pack(pady=(0, 20))

        # Create notebook for tabs
        notebook = ttk.Notebook(main_frame)
        notebook.pack(fill=BOTH, expand=True)

        # Pack the canvas and scrollbar
        canvas.pack(side="left", fill="both", expand=True)
        scrollbar.pack(side="right", fill="y")

        # Bind mouse wheel to canvas
        def _on_mousewheel(event):
            canvas.yview_scroll(int(-1*(event.delta/120)), "units")
        canvas.bind_all("<MouseWheel>", _on_mousewheel)

        # Tab 1: File Toggle
        toggle_frame = ttk.Frame(notebook)
        notebook.add(toggle_frame, text="🔒 File Toggle")
        self.build_toggle_tab(toggle_frame)

        # Tab 2: Config Editor
        editor_frame = ttk.Frame(notebook)
        notebook.add(editor_frame, text="⚙️ Config Editor")
        self.build_editor_tab(editor_frame)

    def build_toggle_tab(self, parent):
        # Main container frame
        main_container = ttk.Frame(parent)
        main_container.pack(fill=BOTH, expand=True, padx=20, pady=20)

        # Left panel for config files
        left_panel = ttk.LabelFrame(main_container, text="Config Files")
        left_panel.pack(side=LEFT, padx=(0, 10), fill=Y, ipadx=15, ipady=15)

        for fname in FILES:
            CheckboxWithSymbol(left_panel, text=fname, variable=self.file_vars[fname]).pack(anchor="w", pady=2)

        # Right panel for server folders
        right_panel = ttk.LabelFrame(main_container, text="Server Folders")
        right_panel.pack(side=LEFT, fill=BOTH, expand=True, ipadx=15, ipady=15)

        self.server_checkboxes_frame = ttk.Frame(right_panel)
        self.server_checkboxes_frame.pack(fill=BOTH, expand=True)

        # Button frame
        button_frame = ttk.Frame(parent)
        button_frame.pack(fill=X, pady=15, padx=20)

        tk.Button(button_frame, text="🔒 Toggle ON (Read-Only)", 
                  command=lambda: self.toggle_files(True),
                  relief="raised", borderwidth=3,
                  bg=self.get_button_color(), fg=self.get_button_text_color(),
                  activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                  ).pack(side=LEFT, padx=(0, 10))
        tk.Button(button_frame, text="🔓 Toggle OFF (Writable)", 
                  command=lambda: self.toggle_files(False),
                  relief="raised", borderwidth=3,
                  bg=self.get_button_color(), fg=self.get_button_text_color(),
                  activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                  ).pack(side=LEFT)

        # Log section
        ttk.Label(parent, text="Operation Log", font=self.label_font).pack(anchor="w", padx=20, pady=(10, 5))

        # Create a frame for the log with scrollbar
        log_frame = ttk.Frame(parent)
        log_frame.pack(fill=BOTH, padx=20, pady=(0, 20))

        self.log_output = Text(log_frame, height=8, font=self.text_font, wrap='word')
        log_scrollbar = ttk.Scrollbar(log_frame, orient="vertical", command=self.log_output.yview)
        self.log_output.configure(yscrollcommand=log_scrollbar.set)
        
        self.log_output.pack(side=LEFT, fill=BOTH, expand=True)
        log_scrollbar.pack(side=RIGHT, fill=Y)

        self.log("Welcome to ARK Server Config Toggler!")
        self.log(f"Base directory: {self.base_dir}")

    def build_editor_tab(self, parent):
        # Variables for editor
        self.current_file = StringVar(value="GameUserSettings.ini")
        self.current_section = StringVar(value="ArkadeEssentials")
        self.source_server = StringVar()
        self.config_entries = {}
        self.all_config_data = {}  # Store all sections data
        self.filtered_config_data = {}  # Store filtered data for search
        self.is_search_active = False
        
        # Controls frame
        controls_frame = ttk.LabelFrame(parent, text="Configuration")
        controls_frame.pack(fill=X, padx=20, pady=20, ipadx=15, ipady=10)

        # Server selection
        ttk.Label(controls_frame, text="Source Server:", font=self.label_font).pack(anchor="w")
        
        self.server_dropdown = ttk.Combobox(controls_frame, textvariable=self.source_server,
                                           state="readonly", font=self.text_font)
        self.server_dropdown.pack(fill=X, pady=(5, 10))
        self.server_dropdown.bind('<<ComboboxSelected>>', self.on_server_changed)

        # File and section selection
        file_section_frame = ttk.Frame(controls_frame)
        file_section_frame.pack(fill=X, pady=(0, 10))

        ttk.Label(file_section_frame, text="File:", font=self.label_font).pack(side=LEFT)
        
        for fname in FILES:
            ttk.Radiobutton(file_section_frame, text=fname, variable=self.current_file, value=fname, 
                           command=self.on_file_changed).pack(side=LEFT, padx=10)

        ttk.Label(file_section_frame, text="Section:", font=self.label_font).pack(side=LEFT, padx=(20, 5))
        
        self.section_dropdown = ttk.Combobox(file_section_frame, textvariable=self.current_section, 
                                            state="readonly", width=20)
        self.section_dropdown.pack(side=LEFT, padx=5)

        # Buttons
        button_frame = ttk.Frame(controls_frame)
        button_frame.pack(fill=X, pady=(10, 0))

        tk.Button(button_frame, text="📋 Load Section", command=self.load_config_section,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side=LEFT, padx=(0, 10))
        tk.Button(button_frame, text="📄 Load All Sections", command=self.load_all_sections,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side=LEFT, padx=(0, 10))
        tk.Button(button_frame, text="🔍 Check Files", command=self.check_server_files,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side=LEFT)

        # Search frame
        search_frame = ttk.Frame(controls_frame)
        search_frame.pack(fill=X, pady=(10, 0))

        ttk.Label(search_frame, text="Search:", font=self.label_font).pack(side=LEFT)
        
        self.search_var = StringVar()
        self.search_entry = ttk.Entry(search_frame, textvariable=self.search_var, width=30)
        self.search_entry.pack(side=LEFT, padx=(5, 10))
        self.search_entry.bind('<KeyRelease>', self.on_search_changed)
        
        tk.Button(search_frame, text="🔍 Search", command=self.perform_search,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side=LEFT, padx=(0, 10))
        tk.Button(search_frame, text="❌ Clear", command=self.clear_search,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side=LEFT)

        # Config values frame
        values_frame = ttk.LabelFrame(parent, text="Configuration Values")
        values_frame.pack(fill=BOTH, expand=True, padx=20, pady=(0, 10), ipadx=15, ipady=15)

        # Scrollable canvas
        canvas_frame = ttk.Frame(values_frame)
        canvas_frame.pack(fill=BOTH, expand=True)

        self.config_canvas = Canvas(canvas_frame, highlightthickness=0, borderwidth=0)
        scrollbar = ttk.Scrollbar(canvas_frame, orient="vertical", command=self.config_canvas.yview)
        self.config_canvas.configure(yscrollcommand=scrollbar.set)
        
        scrollbar.pack(side=RIGHT, fill=Y)
        self.config_canvas.pack(side=LEFT, fill=BOTH, expand=True)

        self.config_frame = ttk.Frame(self.config_canvas)
        self.canvas_window = self.config_canvas.create_window((0, 0), window=self.config_frame, anchor='nw')

        def on_canvas_configure(event):
            self.config_canvas.configure(scrollregion=self.config_canvas.bbox("all"))
            self.config_canvas.itemconfig(self.canvas_window, width=event.width)

        self.config_canvas.bind('<Configure>', on_canvas_configure)
        self.config_canvas.bind("<MouseWheel>", lambda e: self.config_canvas.yview_scroll(int(-1*(e.delta/120)), "units"))

        # Target servers frame
        target_frame = ttk.LabelFrame(parent, text="Target Servers")
        target_frame.pack(fill=X, padx=20, pady=(0, 10), ipadx=15, ipady=10)

        self.target_servers_frame = ttk.Frame(target_frame)
        self.target_servers_frame.pack(fill=X)

        # Save buttons frame
        save_frame = ttk.Frame(parent)
        save_frame.pack(pady=20)
        
        tk.Button(save_frame, text="👁️ Preview Changes", command=self.preview_changes,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side=LEFT, padx=(0, 10))
        tk.Button(save_frame, text="💾 Save Changes", command=self.save_config_changes,
                 relief="raised", borderwidth=3,
                 bg=self.get_button_color(), fg=self.get_button_text_color(),
                 activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                 ).pack(side=LEFT)

    def load_server_folders(self):
        for widget in self.server_checkboxes_frame.winfo_children():
            widget.destroy()
        self.server_vars.clear()

        if hasattr(self, 'target_servers_frame'):
            for widget in self.target_servers_frame.winfo_children():
                widget.destroy()

        if not os.path.exists(self.base_dir):
            self.log(f"❌ Base directory not found: {self.base_dir}")
            return

        try:
            server_list = []
            for entry in os.listdir(self.base_dir):
                if os.path.isdir(os.path.join(self.base_dir, entry)):
                    server_list.append(entry)
                    var = IntVar(value=1)
                    self.server_vars[entry] = var
                    
                    # Toggle tab checkbox
                    CheckboxWithSymbol(self.server_checkboxes_frame, text=entry, variable=var).pack(anchor="w", pady=1)
                    
                    # Config editor checkbox
                    if hasattr(self, 'target_servers_frame'):
                        CheckboxWithSymbol(self.target_servers_frame, text=entry, variable=var).pack(side=LEFT, padx=(0, 15))

            # Update dropdown
            if hasattr(self, 'server_dropdown'):
                self.server_dropdown['values'] = server_list
                if server_list:
                    self.source_server.set(server_list[0])
                    # Trigger initial section dropdown update
                    self.update_section_dropdown(server_list[0], self.current_file.get())

            self.log(f"✅ Found {len(server_list)} server folders.")
            
        except Exception as e:
            self.log(f"❌ Error loading server folders: {e}")

    def toggle_files(self, read_only):
        selected_files = [fname for fname, var in self.file_vars.items() if var.get() == 1]
        selected_servers = [srv for srv, var in self.server_vars.items() if var.get() == 1]

        if not selected_files or not selected_servers:
            self.log("⚠️ No files or servers selected.")
            return

        action = "READ-ONLY" if read_only else "WRITABLE"
        self.log(f"\n🔒 Starting {action} operation...")

        success_count = 0
        for server in selected_servers:
            server_path = os.path.join(self.base_dir, server, CONFIG_RELATIVE_PATH)
            for fname in selected_files:
                file_path = os.path.join(server_path, fname)
                if os.path.isfile(file_path):
                    try:
                        file_attrs = os.stat(file_path).st_mode
                        if read_only:
                            os.chmod(file_path, file_attrs & ~stat.S_IWRITE)
                        else:
                            os.chmod(file_path, file_attrs | stat.S_IWRITE)
                        self.log(f"✅ {server}/{fname}")
                        success_count += 1
                    except Exception as e:
                        self.log(f"❌ {server}/{fname}: {e}")

        self.log(f"📊 Operation complete: {success_count} files processed.")

    def load_config_section(self):
        source_server = self.source_server.get()
        if not source_server:
            self.log("❌ Please select a source server.")
            return

        file_name = self.current_file.get()
        section_name = self.current_section.get().strip()

        # Clear existing entries
        for widget in self.config_frame.winfo_children():
            widget.destroy()
        self.config_entries.clear()

        # Load from selected server
        server_path = os.path.join(self.base_dir, source_server, CONFIG_RELATIVE_PATH)
        file_path = os.path.join(server_path, file_name)
        
        if not os.path.isfile(file_path):
            self.log(f"❌ File not found: {source_server}/{file_name}")
            return

        try:
            config = configparser.ConfigParser()
            config.optionxform = str
            # Use utf-8-sig to handle BOM properly
            config.read(file_path, encoding='utf-8-sig')
            
            if section_name not in config:
                self.log(f"❌ Section '[{section_name}]' not found")
                self.log(f"Available sections: {', '.join(config.sections())}")
                return
                
            config_data = dict(config[section_name])
                
        except Exception as e:
            self.log(f"❌ Error reading {file_path}: {e}")
            return

        self.log(f"✅ Loaded {len(config_data)} values from {source_server}")

        # Store the data in the new format
        self.all_config_data = {section_name: config_data}
        self.filtered_config_data = {section_name: config_data}
        self.is_search_active = False
        
        # Display the data
        self.display_config_data()

    def save_config_changes(self):
        if not self.config_entries:
            self.log("❌ No configuration values loaded.")
            return

        selected_servers = [srv for srv, var in self.server_vars.items() if var.get() == 1]
        if not selected_servers:
            self.log("❌ No servers selected.")
            return

        file_name = self.current_file.get()

        self.log(f"\n💾 Saving to {len(selected_servers)} servers...")

        success_count = 0
        for server in selected_servers:
            server_path = os.path.join(self.base_dir, server, CONFIG_RELATIVE_PATH)
            file_path = os.path.join(server_path, file_name)

            try:
                config = configparser.ConfigParser()
                config.optionxform = str
                
                if os.path.isfile(file_path):
                    # Use utf-8-sig to handle BOM properly
                    config.read(file_path, encoding='utf-8-sig')

                changes_made = 0
                # Process all config entries with their section names
                for entry_key, var in self.config_entries.items():
                    if '.' in entry_key:
                        section_name, key = entry_key.split('.', 1)
                    else:
                        # Fallback for old format
                        section_name = self.current_section.get().strip()
                        key = entry_key
                    
                    if section_name not in config:
                        config.add_section(section_name)

                    new_value = var.get().strip()
                    old_value = config.get(section_name, key, fallback=None)
                    
                    if old_value != new_value:
                        config.set(section_name, key, new_value)
                        changes_made += 1

                # Save without BOM using regular utf-8
                with open(file_path, 'w', encoding='utf-8') as f:
                    config.write(f, space_around_delimiters=False)

                self.log(f"✅ {server}: {changes_made} changes")
                success_count += 1

            except Exception as e:
                self.log(f"❌ {server}: {e}")

        self.log(f"📊 Updated {success_count}/{len(selected_servers)} servers.")

    def load_all_sections(self):
        """Load all sections from the selected file"""
        source_server = self.source_server.get()
        if not source_server:
            self.log("❌ Please select a source server.")
            return

        file_name = self.current_file.get()

        # Load from selected server
        server_path = os.path.join(self.base_dir, source_server, CONFIG_RELATIVE_PATH)
        file_path = os.path.join(server_path, file_name)
        
        if not os.path.isfile(file_path):
            self.log(f"❌ File not found: {source_server}/{file_name}")
            return

        try:
            config = configparser.ConfigParser()
            config.optionxform = str
            # Use utf-8-sig to handle BOM properly
            config.read(file_path, encoding='utf-8-sig')
            
            # Clear existing data
            self.all_config_data.clear()
            self.filtered_config_data.clear()
            
            # Load all sections
            total_values = 0
            for section_name in config.sections():
                section_data = dict(config[section_name])
                self.all_config_data[section_name] = section_data
                total_values += len(section_data)
            
            # Initially show all data
            self.filtered_config_data = self.all_config_data.copy()
            
            self.log(f"✅ Loaded {len(config.sections())} sections with {total_values} total values from {source_server}")
            self.display_config_data()
                
        except Exception as e:
            self.log(f"❌ Error reading {file_path}: {e}")

    def display_config_data(self):
        """Display the current config data (all or filtered) with modified items pinned to top"""
        # Store current changes before clearing
        current_changes = {}
        for entry_key, var in self.config_entries.items():
            current_changes[entry_key] = var.get()
        
        # Clear existing entries
        for widget in self.config_frame.winfo_children():
            widget.destroy()
        self.config_entries.clear()

        if not self.filtered_config_data:
            return

        row = 0
        
        # First, collect ALL modified items from the original data (not just filtered)
        all_modified_items = []
        all_unmodified_items = []
        
        # Check ALL data for modifications (not just filtered)
        for section_name, section_data in self.all_config_data.items():
            for key, value in sorted(section_data.items()):
                entry_key = f"{section_name}.{key}"
                original_value = str(value)
                current_value = current_changes.get(entry_key, original_value)
                
                # Check if this item has been modified
                if current_value != original_value:
                    all_modified_items.append((section_name, key, original_value, current_value))
                else:
                    all_unmodified_items.append((section_name, key, original_value, current_value))
        
        # Now separate modified items that are in the current filter vs not in filter
        modified_items_in_filter = []
        modified_items_not_in_filter = []
        unmodified_items = []
        
        # Check which modified items are in the current filter
        for section_name, key, original_value, current_value in all_modified_items:
            # Check if this item is in the current filtered data
            if (section_name in self.filtered_config_data and 
                key in self.filtered_config_data[section_name]):
                modified_items_in_filter.append((section_name, key, original_value, current_value))
            else:
                modified_items_not_in_filter.append((section_name, key, original_value, current_value))
        
        # Get unmodified items that are in the current filter
        for section_name, section_data in self.filtered_config_data.items():
            for key, value in sorted(section_data.items()):
                entry_key = f"{section_name}.{key}"
                original_value = str(value)
                current_value = current_changes.get(entry_key, original_value)
                
                # Only add if not modified
                if current_value == original_value:
                    unmodified_items.append((section_name, key, original_value, current_value))
        
        # Display modified items first (pinned to top)
        all_modified_items_to_show = modified_items_in_filter + modified_items_not_in_filter
        
        if all_modified_items_to_show:
            # Add a separator label for modified items
            modified_label = ttk.Label(self.config_frame, text="📌 Modified Settings", 
                                     font=self.label_font, foreground="orange")
            modified_label.grid(row=row, column=0, columnspan=2, sticky='w', padx=(0, 10), pady=(10, 5))
            row += 1
            
            # Group modified items by section
            modified_by_section = {}
            for section_name, key, original_value, current_value in all_modified_items_to_show:
                if section_name not in modified_by_section:
                    modified_by_section[section_name] = []
                modified_by_section[section_name].append((key, original_value, current_value))
            
            # Display modified items
            for section_name in sorted(modified_by_section.keys()):
                # Add section header for modified items
                section_label = ttk.Label(self.config_frame, text=f"[{section_name}]", 
                                         font=self.label_font, foreground="orange")
                section_label.grid(row=row, column=0, columnspan=2, sticky='w', padx=(0, 10), pady=(5, 2))
                row += 1
                
                for key, original_value, current_value in modified_by_section[section_name]:
                    # Add a visual indicator that this is modified
                    ttk.Label(self.config_frame, text=f"✏️ {key}:", font=self.text_font,
                             width=30, foreground="orange").grid(row=row, column=0, sticky='e', padx=(0, 10), pady=2)

                    entry_key = f"{section_name}.{key}"
                    var = StringVar(value=current_value)
                    self.config_entries[entry_key] = var
                    
                    ttk.Entry(self.config_frame, textvariable=var, width=40).grid(row=row, column=1, sticky='ew', pady=2)
                    row += 1
        
        # Display unmodified items
        if unmodified_items:
            # Add a separator label for unmodified items
            unmodified_label = ttk.Label(self.config_frame, text="📄 Other Settings", 
                                       font=self.label_font, foreground="blue")
            unmodified_label.grid(row=row, column=0, columnspan=2, sticky='w', padx=(0, 10), pady=(10, 5))
            row += 1
            
            # Group unmodified items by section
            unmodified_by_section = {}
            for section_name, key, original_value, current_value in unmodified_items:
                if section_name not in unmodified_by_section:
                    unmodified_by_section[section_name] = []
                unmodified_by_section[section_name].append((key, original_value, current_value))
            
            # Display unmodified items
            for section_name in sorted(unmodified_by_section.keys()):
                # Add section header for unmodified items
                section_label = ttk.Label(self.config_frame, text=f"[{section_name}]", 
                                         font=self.label_font, foreground="blue")
                section_label.grid(row=row, column=0, columnspan=2, sticky='w', padx=(0, 10), pady=(5, 2))
                row += 1
                
                for key, original_value, current_value in unmodified_by_section[section_name]:
                    ttk.Label(self.config_frame, text=f"{key}:", font=self.text_font,
                             width=30).grid(row=row, column=0, sticky='e', padx=(0, 10), pady=2)

                    entry_key = f"{section_name}.{key}"
                    var = StringVar(value=current_value)
                    self.config_entries[entry_key] = var
                    
                    ttk.Entry(self.config_frame, textvariable=var, width=40).grid(row=row, column=1, sticky='ew', pady=2)
                    row += 1

        self.config_frame.grid_columnconfigure(1, weight=1)
        self.config_frame.update_idletasks()
        self.config_canvas.configure(scrollregion=self.config_canvas.bbox("all"))

    def on_search_changed(self, event=None):
        """Handle search input changes - perform search as user types"""
        if self.all_config_data:  # Only search if we have data loaded
            self.perform_search()

    def perform_search(self):
        """Perform search on loaded configuration data"""
        if not self.all_config_data:
            self.log("❌ No configuration data loaded. Please load sections first.")
            return

        search_term = self.search_var.get().strip().lower()
        
        if not search_term:
            # Show all data if search is empty
            self.filtered_config_data = self.all_config_data.copy()
            self.is_search_active = False
        else:
            # Filter data based on search term
            self.filtered_config_data = {}
            self.is_search_active = True
            
            for section_name, section_data in self.all_config_data.items():
                filtered_section = {}
                
                # Check if section name matches
                if search_term in section_name.lower():
                    filtered_section = section_data.copy()
                else:
                    # Check if any key or value matches
                    for key, value in section_data.items():
                        if (search_term in key.lower() or 
                            search_term in str(value).lower()):
                            filtered_section[key] = value
                
                if filtered_section:
                    self.filtered_config_data[section_name] = filtered_section

        # Update display
        self.display_config_data()
        
        # Log search results
        total_matches = sum(len(section) for section in self.filtered_config_data.values())
        if self.is_search_active:
            self.log(f"🔍 Search '{search_term}': Found {len(self.filtered_config_data)} sections with {total_matches} matches")
        else:
            self.log(f"📄 Showing all {total_matches} configuration values")

    def clear_search(self):
        """Clear search and show all data"""
        self.search_var.set("")
        if self.all_config_data:
            self.filtered_config_data = self.all_config_data.copy()
            self.is_search_active = False
            self.display_config_data()
            self.log("📄 Search cleared - showing all configuration values")

    def preview_changes(self):
        """Show a preview window of all changes that will be made"""
        if not self.config_entries:
            self.log("❌ No configuration values loaded.")
            return

        selected_servers = [srv for srv, var in self.server_vars.items() if var.get() == 1]
        if not selected_servers:
            self.log("❌ No servers selected.")
            return

        # Create preview window
        preview_window = Toplevel(self)
        preview_window.title("Preview Configuration Changes")
        preview_window.geometry("800x600")
        preview_window.transient(self)  # Make it modal to the main window
        preview_window.grab_set()  # Make it modal

        # Main frame
        main_frame = ttk.Frame(preview_window, padding="20")
        main_frame.pack(fill=BOTH, expand=True)

        # Title
        title_label = ttk.Label(main_frame, text="Configuration Changes Preview", 
                               font=self.title_font)
        title_label.pack(pady=(0, 20))

        # Summary frame
        summary_frame = ttk.LabelFrame(main_frame, text="Summary")
        summary_frame.pack(fill=X, pady=(0, 20))

        # Calculate changes
        changes_summary = self.calculate_changes_summary()
        
        ttk.Label(summary_frame, text=f"Target Servers: {', '.join(selected_servers)}", 
                 font=self.label_font).pack(anchor="w", padx=10, pady=5)
        ttk.Label(summary_frame, text=f"Total Changes: {changes_summary['total_changes']}", 
                 font=self.label_font).pack(anchor="w", padx=10, pady=5)
        ttk.Label(summary_frame, text=f"Modified Sections: {len(changes_summary['sections'])}", 
                 font=self.label_font).pack(anchor="w", padx=10, pady=5)
        
        # Show per-server summary
        for server, server_info in changes_summary['servers'].items():
            if server_info['file_exists']:
                ttk.Label(summary_frame, text=f"  {server}: {server_info['total_changes']} changes", 
                         font=self.text_font).pack(anchor="w", padx=20, pady=2)
            else:
                ttk.Label(summary_frame, text=f"  {server}: File not found", 
                         font=self.text_font, foreground="red").pack(anchor="w", padx=20, pady=2)

        # Changes details frame
        details_frame = ttk.LabelFrame(main_frame, text="Detailed Changes")
        details_frame.pack(fill=BOTH, expand=True, pady=(0, 20))

        # Create scrollable text widget for changes
        text_frame = ttk.Frame(details_frame)
        text_frame.pack(fill=BOTH, expand=True, padx=10, pady=10)

        preview_text = Text(text_frame, wrap='word', font=self.text_font)
        scrollbar = ttk.Scrollbar(text_frame, orient="vertical", command=preview_text.yview)
        preview_text.configure(yscrollcommand=scrollbar.set)

        preview_text.pack(side=LEFT, fill=BOTH, expand=True)
        scrollbar.pack(side=RIGHT, fill=Y)

        # Populate preview text
        self.populate_preview_text(preview_text, changes_summary)

        # Buttons frame
        button_frame = ttk.Frame(main_frame)
        button_frame.pack(fill=X, pady=(0, 10))

        tk.Button(button_frame, text="✅ Apply Changes", 
                  command=lambda: self.apply_changes_from_preview(preview_window),
                  relief="raised", borderwidth=3,
                  bg=self.get_button_color(), fg=self.get_button_text_color(),
                  activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                  ).pack(side=RIGHT, padx=(10, 0))
        tk.Button(button_frame, text="❌ Cancel", 
                  command=preview_window.destroy,
                  relief="raised", borderwidth=3,
                  bg=self.get_button_color(), fg=self.get_button_text_color(),
                  activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                  ).pack(side=RIGHT)

    def calculate_changes_summary(self):
        """Calculate a summary of all changes that will be made for each server"""
        changes_summary = {
            'total_changes': 0,
            'sections': {},
            'servers': {}
        }

        # Get current values from the source server
        source_server = self.source_server.get()
        file_name = self.current_file.get()
        
        if not source_server:
            return changes_summary

        # Get selected servers
        selected_servers = [srv for srv, var in self.server_vars.items() if var.get() == 1]
        
        # Process each server
        for server in selected_servers:
            server_path = os.path.join(self.base_dir, server, CONFIG_RELATIVE_PATH)
            file_path = os.path.join(server_path, file_name)
            
            changes_summary['servers'][server] = {
                'file_exists': os.path.isfile(file_path),
                'changes': {},
                'total_changes': 0
            }
            
            if not os.path.isfile(file_path):
                continue
                
            try:
                config = configparser.ConfigParser()
                config.optionxform = str
                config.read(file_path, encoding='utf-8-sig')

                # Process all config entries
                for entry_key, var in self.config_entries.items():
                    if '.' in entry_key:
                        section_name, key = entry_key.split('.', 1)
                    else:
                        section_name = self.current_section.get().strip()
                        key = entry_key

                    new_value = var.get().strip()
                    old_value = config.get(section_name, key, fallback=None)

                    if old_value != new_value:
                        changes_summary['total_changes'] += 1
                        changes_summary['servers'][server]['total_changes'] += 1
                        
                        if section_name not in changes_summary['sections']:
                            changes_summary['sections'][section_name] = []
                        
                        if section_name not in changes_summary['servers'][server]['changes']:
                            changes_summary['servers'][server]['changes'][section_name] = []
                        
                        change_info = {
                            'key': key,
                            'old_value': old_value,
                            'new_value': new_value,
                            'exists': old_value is not None
                        }
                        
                        changes_summary['sections'][section_name].append(change_info)
                        changes_summary['servers'][server]['changes'][section_name].append(change_info)

            except Exception as e:
                self.log(f"❌ Error calculating changes for {server}: {e}")

        return changes_summary

    def populate_preview_text(self, text_widget, changes_summary):
        """Populate the preview text widget with detailed changes per server"""
        text_widget.delete(1.0, END)
        
        if changes_summary['total_changes'] == 0:
            text_widget.insert(END, "No changes detected.\n")
            return

        # Header
        text_widget.insert(END, "Configuration Changes Preview\n", "title")
        text_widget.insert(END, "=" * 50 + "\n\n")

        # Summary
        text_widget.insert(END, f"Total Changes: {changes_summary['total_changes']}\n", "summary")
        text_widget.insert(END, f"Modified Sections: {len(changes_summary['sections'])}\n\n", "summary")

        # Detailed changes per server
        for server, server_info in changes_summary['servers'].items():
            text_widget.insert(END, f"Server: {server}\n", "server")
            text_widget.insert(END, "=" * 40 + "\n")
            
            if not server_info['file_exists']:
                text_widget.insert(END, "❌ Configuration file not found\n\n", "error")
                continue
                
            if server_info['total_changes'] == 0:
                text_widget.insert(END, "✅ No changes needed\n\n", "no_changes")
                continue
            
            # Show changes by section for this server
            for section_name, changes in server_info['changes'].items():
                text_widget.insert(END, f"[{section_name}]\n", "section")
                text_widget.insert(END, "-" * 30 + "\n")
                
                for change in changes:
                    text_widget.insert(END, f"{change['key']}:\n", "key")
                    if change['exists']:
                        text_widget.insert(END, f"  Old: {change['old_value']}\n", "old")
                        text_widget.insert(END, f"  New: {change['new_value']}\n", "new")
                    else:
                        text_widget.insert(END, f"  New: {change['new_value']} (setting will be created)\n", "new_setting")
                    text_widget.insert(END, "\n")
            
            text_widget.insert(END, "\n")

        # Configure text tags for styling
        text_widget.tag_configure("title", font=("Segoe UI", 12, "bold"))
        text_widget.tag_configure("summary", font=("Segoe UI", 10, "bold"))
        text_widget.tag_configure("server", font=("Segoe UI", 11, "bold"), foreground="purple")
        text_widget.tag_configure("section", font=("Segoe UI", 10, "bold"), foreground="blue")
        text_widget.tag_configure("key", font=("Segoe UI", 9, "bold"))
        text_widget.tag_configure("old", font=("Segoe UI", 9), foreground="red")
        text_widget.tag_configure("new", font=("Segoe UI", 9), foreground="green")
        text_widget.tag_configure("new_setting", font=("Segoe UI", 9), foreground="orange")
        text_widget.tag_configure("error", font=("Segoe UI", 9), foreground="red")
        text_widget.tag_configure("no_changes", font=("Segoe UI", 9), foreground="gray")

    def apply_changes_from_preview(self, preview_window):
        """Apply the changes after user confirms in preview window"""
        preview_window.destroy()
        self.save_config_changes()

    def check_server_files(self):
        self.log("\n🔍 Checking server files...")
        
        for server in sorted(self.server_vars.keys()):
            self.log(f"\n📁 {server}:")
            server_path = os.path.join(self.base_dir, server, CONFIG_RELATIVE_PATH)
            
            if not os.path.exists(server_path):
                self.log("  ❌ Config directory not found")
                continue
                
            for file_name in FILES:
                file_path = os.path.join(server_path, file_name)
                if os.path.isfile(file_path):
                    try:
                        file_size = os.path.getsize(file_path)
                        self.log(f"  ✅ {file_name} ({file_size} bytes)")
                        
                        # Check for BOM
                        with open(file_path, 'rb') as f:
                            first_bytes = f.read(3)
                            has_bom = first_bytes == b'\xef\xbb\xbf'
                            if has_bom:
                                self.log(f"     📝 File has UTF-8 BOM")
                        
                        config = configparser.ConfigParser()
                        config.optionxform = str
                        # Use utf-8-sig to handle BOM properly
                        config.read(file_path, encoding='utf-8-sig')
                        
                        sections = config.sections()
                        if sections:
                            self.log(f"     Sections: {', '.join(sections)}")
                            if 'ArkadeEssentials' in sections:
                                count = len(config['ArkadeEssentials'])
                                self.log(f"     🎯 ArkadeEssentials: {count} values")
                            if 'ServerSettings' in sections:
                                count = len(config['ServerSettings'])
                                self.log(f"     🔧 ServerSettings: {count} values")
                        else:
                            self.log(f"     ⚠️ No sections (empty/invalid file)")
                            
                    except Exception as e:
                        self.log(f"  ❌ {file_name}: {str(e)}")
                else:
                    self.log(f"  ❌ {file_name}: Not found")

    def on_server_changed(self, event=None):
        """Update the section dropdown when server selection changes"""
        source_server = self.source_server.get()
        if not source_server:
            return
            
        file_name = self.current_file.get()
        self.update_section_dropdown(source_server, file_name)

    def on_file_changed(self):
        """Update the section dropdown when file selection changes"""
        source_server = self.source_server.get()
        if not source_server:
            return
            
        file_name = self.current_file.get()
        self.update_section_dropdown(source_server, file_name)

    def update_section_dropdown(self, server, file_name):
        """Update the section dropdown with available sections from the selected file"""
        try:
            server_path = os.path.join(self.base_dir, server, CONFIG_RELATIVE_PATH)
            file_path = os.path.join(server_path, file_name)
            
            if not os.path.isfile(file_path):
                self.section_dropdown['values'] = []
                self.current_section.set("")
                return

            config = configparser.ConfigParser()
            config.optionxform = str
            config.read(file_path, encoding='utf-8-sig')
            
            sections = config.sections()
            self.section_dropdown['values'] = sections
            
            # Set default selection if available
            if sections:
                if 'ArkadeEssentials' in sections:
                    self.current_section.set('ArkadeEssentials')
                elif 'ServerSettings' in sections:
                    self.current_section.set('ServerSettings')
                else:
                    self.current_section.set(sections[0])
            else:
                self.current_section.set("")
                
        except Exception as e:
            self.log(f"❌ Error updating section dropdown: {e}")
            self.section_dropdown['values'] = []
            self.current_section.set("")

    def log(self, message):
        self.log_output.insert("end", message + "\n")
        self.log_output.see("end")
        self.log_output.update()

    def update_theme(self):
        """Update theme for the plugin toggle tab"""
        colors = ModernTheme.get_colors(self.theme_var.get())
        self.log_output.configure(
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