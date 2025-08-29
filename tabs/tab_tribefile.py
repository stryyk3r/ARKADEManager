import tkinter as tk
from tkinter import ttk, messagebox, simpledialog, Menu, filedialog
from PIL import Image, ImageTk
import os
import re
import json
from pathlib import Path
from core.theme import ModernTheme

class TribeFileTab(ttk.Frame):
    def __init__(self, parent, theme_var):
        super().__init__(parent)
        self.theme_var = theme_var
        self.saved_folders = {}  # Dict of server_name: folder_path
        self.found_files = []  # List of (server, path, filename) tuples
        
        self.load_saved_folders()
        self.setup_ui()

    def setup_ui(self):
        # Main container
        main_frame = ttk.Frame(self)
        main_frame.pack(fill='both', expand=True, padx=20, pady=20)

        # Header
        header_frame = ttk.Frame(main_frame)
        header_frame.pack(fill='x', pady=(0, 20))
        ttk.Label(header_frame, text="Tribe File Management", font=('Segoe UI', 14, 'bold')).pack(side='left')

        # Split into left and right panes
        panes_frame = ttk.Frame(main_frame)
        panes_frame.pack(fill='both', expand=True)

        # Left pane - Server folders management
        left_pane = ttk.LabelFrame(panes_frame, text="Server Folders")
        left_pane.pack(side='left', fill='both', expand=True, padx=(0, 10), pady=10)

        # Server folders list
        folder_frame = ttk.Frame(left_pane)
        folder_frame.pack(fill='both', expand=True, padx=10, pady=10)
        
        ttk.Label(folder_frame, text="Saved Folders:", font=('Segoe UI', 12)).pack(anchor='w', pady=(0, 10))
        
        # Create Treeview for folders
        self.folder_tree = ttk.Treeview(folder_frame, columns=('Server', 'Path'), show='headings', height=10)
        self.folder_tree.heading('Server', text='Server Name')
        self.folder_tree.heading('Path', text='SavedArks Path')
        self.folder_tree.column('Server', width=150)
        self.folder_tree.column('Path', width=300)
        
        # Scrollbars for folder tree
        folder_scroll_y = ttk.Scrollbar(folder_frame, orient="vertical", command=self.folder_tree.yview)
        folder_scroll_x = ttk.Scrollbar(folder_frame, orient="horizontal", command=self.folder_tree.xview)
        self.folder_tree.configure(yscrollcommand=folder_scroll_y.set, xscrollcommand=folder_scroll_x.set)
        
        self.folder_tree.pack(side='left', fill='both', expand=True)
        folder_scroll_y.pack(side='right', fill='y')
        folder_scroll_x.pack(side='bottom', fill='x')

        # Folder management buttons
        folder_button_frame = ttk.Frame(left_pane)
        folder_button_frame.pack(fill='x', pady=(10, 0), padx=10)
        
        tk.Button(folder_button_frame, 
                  text="Add Server Directory", 
                  command=self.add_server_directory,
                  relief="raised", borderwidth=3,
                  bg=self.get_button_color(), fg=self.get_button_text_color(),
                  activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                  ).pack(side='left', padx=(0, 5))
        
        tk.Button(folder_button_frame, 
                  text="Remove Selected", 
                  command=self.remove_folder,
                  relief="raised", borderwidth=3,
                  bg=self.get_button_color(), fg=self.get_button_text_color(),
                  activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                  ).pack(side='left', padx=5)

        # Right pane - Tribe deletion
        right_pane = ttk.LabelFrame(panes_frame, text="Tribe File Management")
        right_pane.pack(side='right', fill='both', expand=True, padx=(10, 0), pady=10)

        # Tribe ID input
        input_frame = ttk.Frame(right_pane)
        input_frame.pack(fill='x', pady=(0, 10), padx=10)
        
        ttk.Label(input_frame, text="Enter Tribe IDs (comma-separated):", font=('Segoe UI', 12)).pack(anchor='w', pady=(0, 5))
        self.tribe_ids_entry = ttk.Entry(input_frame)
        self.tribe_ids_entry.pack(fill='x')

        # Search button
        tk.Button(input_frame, 
                  text="Search for Tribe Files", 
                  command=self.search_tribe_files,
                  relief="raised", borderwidth=3,
                  bg=self.get_button_color(), fg=self.get_button_text_color(),
                  activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                  ).pack(fill='x', pady=(10, 0))

        # Results frame
        results_frame = ttk.Frame(right_pane)
        results_frame.pack(fill='both', expand=True, pady=(10, 0), padx=10)
        
        ttk.Label(results_frame, text="Found Files:", font=('Segoe UI', 12)).pack(anchor='w', pady=(0, 5))
        
        # Create Treeview for found files
        self.files_tree = ttk.Treeview(results_frame, 
                                     columns=('Server', 'Map', 'File'), 
                                     show='headings', 
                                     height=10,
                                     selectmode='extended')
        
        self.files_tree.heading('Server', text='Server')
        self.files_tree.heading('Map', text='Map')
        self.files_tree.heading('File', text='File')
        
        self.files_tree.column('Server', width=100)
        self.files_tree.column('Map', width=100)
        self.files_tree.column('File', width=200)
        
        # Scrollbars for files tree
        files_scroll_y = ttk.Scrollbar(results_frame, orient="vertical", command=self.files_tree.yview)
        files_scroll_x = ttk.Scrollbar(results_frame, orient="horizontal", command=self.files_tree.xview)
        self.files_tree.configure(yscrollcommand=files_scroll_y.set, xscrollcommand=files_scroll_x.set)
        
        self.files_tree.pack(side='left', fill='both', expand=True)
        files_scroll_y.pack(side='right', fill='y')
        files_scroll_x.pack(side='bottom', fill='x')

        # Selection buttons
        selection_frame = ttk.Frame(right_pane)
        selection_frame.pack(fill='x', pady=(10, 0), padx=10)
        
        tk.Button(selection_frame, 
                  text="Select All", 
                  command=lambda: self.select_all_files(True),
                  relief="raised", borderwidth=3,
                  bg=self.get_button_color(), fg=self.get_button_text_color(),
                  activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                  ).pack(side='left', padx=(0, 5))
        
        tk.Button(selection_frame, 
                  text="Deselect All", 
                  command=lambda: self.select_all_files(False),
                  relief="raised", borderwidth=3,
                  bg=self.get_button_color(), fg=self.get_button_text_color(),
                  activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color()
                  ).pack(side='left', padx=5)

                # Delete button
        delete_button = tk.Button(right_pane, 
                                  text="Delete Selected Files", 
                                  command=self.delete_selected_files,
                                  relief="raised", borderwidth=3,
                                  bg=self.get_button_color(), fg=self.get_button_text_color(),
                                  activebackground=self.get_button_hover_color(), activeforeground=self.get_button_text_color())
        delete_button.pack(fill='x', pady=(10, 0), padx=10)

        # Status label
        self.status_label = ttk.Label(right_pane, text="", font=('Segoe UI', 10))
        self.status_label.pack(fill='x', pady=(10, 0), padx=10)

        # Populate folder list
        self.refresh_folder_list()

    def select_all_files(self, select=True):
        if select:
            self.files_tree.selection_set(self.files_tree.get_children())
        else:
            self.files_tree.selection_remove(self.files_tree.get_children())

    def search_tribe_files(self):
        tribe_ids = self.tribe_ids_entry.get().strip()
        if not tribe_ids:
            messagebox.showwarning("Warning", "Please enter at least one tribe ID.")
            return

        # Parse tribe IDs
        try:
            tribe_ids = [tid.strip() for tid in tribe_ids.split(',')]
            tribe_ids = [tid for tid in tribe_ids if tid.isdigit()]
            if not tribe_ids:
                messagebox.showwarning("Warning", "No valid tribe IDs found. Please enter numeric IDs separated by commas.")
                return
        except Exception as e:
            messagebox.showerror("Error", f"Failed to parse tribe IDs: {e}")
            return

        if not self.saved_folders:
            messagebox.showwarning("Warning", "No SavedArks folders configured. Please add folders first.")
            return

        # Clear previous results
        self.files_tree.delete(*self.files_tree.get_children())
        self.found_files = []
        self.status_label.config(text="Searching for files...")
        self.update()

        found_count = 0
        print(f"\nSearching for tribe IDs: {tribe_ids}")

        for server, base_path in self.saved_folders.items():
            print(f"\nSearching in server {server} at path: {base_path}")
            try:
                if not os.path.exists(base_path):
                    print(f"Path does not exist: {base_path}")
                    continue

                # First, check if we're actually in a SavedArks folder, if not, look for it
                if not base_path.endswith('SavedArks'):
                    saved_arks = os.path.join(base_path, 'ShooterGame', 'Saved', 'SavedArks')
                    if os.path.exists(saved_arks):
                        base_path = saved_arks
                        print(f"Found SavedArks folder: {base_path}")

                print(f"Walking directory: {base_path}")
                for root, _, files in os.walk(base_path):
                    map_folder = os.path.basename(root)
                    print(f"Checking folder: {map_folder}")
                    print(f"Files in folder: {files}")
                    
                    for file in files:
                        file_lower = file.lower()
                        if not (file_lower.endswith('.arktribe') or file_lower.endswith('.tribebak')):
                            continue
                            
                        for tribe_id in tribe_ids:
                            # Match exact tribe ID in filename
                            if file_lower == f"{tribe_id}.arktribe" or file_lower == f"{tribe_id}.tribebak":
                                full_path = os.path.join(root, file)
                                print(f"Found matching file: {full_path}")
                                found_count += 1
                                self.found_files.append({
                                    'server': server,
                                    'map': map_folder,
                                    'filename': file,
                                    'full_path': full_path
                                })
                                self.files_tree.insert('', 'end', values=(server, map_folder, file))
                                self.update()

            except Exception as e:
                print(f"Error searching in {server}: {e}")
                continue

        if found_count > 0:
            self.status_label.config(text=f"Found {found_count} tribe files. Select files to delete.")
            self.select_all_files(True)
        else:
            self.status_label.config(text="No tribe files found.")
            print("No files found matching the search criteria.")

    def delete_selected_files(self):
        selected = self.files_tree.selection()
        if not selected:
            messagebox.showwarning("Warning", "Please select files to delete.")
            return

        # Confirm deletion
        count = len(selected)
        if not messagebox.askyesno("Confirm Deletion", 
                                 f"Are you sure you want to delete {count} selected file(s)?\n\n"
                                 "This action cannot be undone!"):
            return

        deleted_files = []
        errors = []

        for item in selected:
            server, map_folder, filename = self.files_tree.item(item)['values']
            
            # Find the matching file info
            file_info = next((f for f in self.found_files 
                            if f['server'] == server 
                            and f['map'] == map_folder 
                            and f['filename'] == filename), None)
            
            if file_info:
                try:
                    file_path = file_info['full_path']
                    print(f"Attempting to delete: {file_path}")
                    
                    if os.path.exists(file_path):
                        os.remove(file_path)
                        deleted_files.append(f"{server} ({map_folder}): {filename}")
                        self.files_tree.delete(item)
                        print(f"Successfully deleted: {file_path}")
                    else:
                        errors.append(f"File not found: {file_path}")
                except Exception as e:
                    print(f"Error deleting {file_path}: {e}")
                    errors.append(f"Failed to delete {filename} from {server}: {e}")
            else:
                errors.append(f"Could not find file info for {filename} in {server}")

        # Update status and show results
        if deleted_files:
            self.status_label.config(text=f"Successfully deleted {len(deleted_files)} files.")
        
        # Show detailed results
        result_message = "Operation completed\n\n"
        if deleted_files:
            result_message += f"Deleted {len(deleted_files)} files:\n" + "\n".join(deleted_files) + "\n\n"
        if errors:
            result_message += "Errors:\n" + "\n".join(errors)
        
        messagebox.showinfo("Results", result_message)

    def load_saved_folders(self):
        try:
            cache_dir = Path.home() / '.ark_server_manager'
            cache_file = cache_dir / 'saved_folders.json'
            if cache_file.exists():
                with cache_file.open('r') as f:
                    self.saved_folders = json.load(f)
        except Exception as e:
            print(f"Error loading saved folders: {e}")
            self.saved_folders = {}

    def save_saved_folders(self):
        try:
            cache_dir = Path.home() / '.ark_server_manager'
            cache_dir.mkdir(exist_ok=True)
            cache_file = cache_dir / 'saved_folders.json'
            with cache_file.open('w') as f:
                json.dump(self.saved_folders, f)
        except Exception as e:
            print(f"Error saving folders: {e}")

    def refresh_folder_list(self):
        self.folder_tree.delete(*self.folder_tree.get_children())
        for server, path in self.saved_folders.items():
            self.folder_tree.insert('', 'end', values=(server, path))

    def add_server_directory(self):
        """Add a parent directory containing multiple server folders"""
        # Open folder dialog starting at the SystemDrive
        initial_dir = os.path.join(os.getenv('SystemDrive', 'C:'), '')
        parent_dir = filedialog.askdirectory(
            title="Select Parent Directory Containing Server Folders",
            initialdir=initial_dir
        )
        
        if not parent_dir:
            return

        # Convert to Windows path format
        parent_dir = os.path.normpath(parent_dir)
        
        # Look for server directories
        added_count = 0
        
        try:
            server_dirs = os.listdir(parent_dir)
            for server_dir in server_dirs:
                full_server_path = os.path.join(parent_dir, server_dir)
                
                if not os.path.isdir(full_server_path):
                    continue
                
                # Look for SavedArks folder
                saved_arks_path = None
                
                # Try the direct path first
                direct_saved_arks = os.path.join(full_server_path, 'SavedArks')
                if os.path.exists(direct_saved_arks) and os.path.isdir(direct_saved_arks):
                    saved_arks_path = direct_saved_arks
                
                # If not found, try the standard ARK path structure
                if saved_arks_path is None:
                    standard_saved_arks = os.path.join(full_server_path, 'ShooterGame', 'Saved', 'SavedArks')
                    if os.path.exists(standard_saved_arks) and os.path.isdir(standard_saved_arks):
                        saved_arks_path = standard_saved_arks
                
                # If SavedArks folder found, add it to the list
                if saved_arks_path:
                    server_name = server_dir
                    
                    # Check if server already exists
                    if server_name in self.saved_folders:
                        # Generate a unique name
                        count = 1
                        while f"{server_name} ({count})" in self.saved_folders:
                            count += 1
                        server_name = f"{server_name} ({count})"
                    
                    self.saved_folders[server_name] = saved_arks_path
                    added_count += 1
        
        except Exception as e:
            messagebox.showerror("Error", f"Failed to scan directories: {e}")
            return
        
        if added_count > 0:
            self.save_saved_folders()
            self.refresh_folder_list()
            messagebox.showinfo("Success", f"Added {added_count} server SavedArks folders.")
        else:
            messagebox.showinfo("Info", "No SavedArks folders were found in the selected directory.")

    def add_folder(self):
        """Legacy method to add a single folder"""
        # Open folder dialog starting at the SystemDrive
        initial_dir = os.path.join(os.getenv('SystemDrive', 'C:'), '')
        folder_path = filedialog.askdirectory(
            title="Select SavedArks folder",
            initialdir=initial_dir
        )
        
        if not folder_path:
            return

        # Convert to Windows path format
        folder_path = os.path.normpath(folder_path)
        
        # Try to extract a default server name from the path
        path_parts = folder_path.split(os.sep)
        default_name = ""
        
        # Look for common folder names that might indicate server name
        server_indicators = ["server", "ark", "saved", "savedarks"]
        for i, part in enumerate(path_parts):
            part_lower = part.lower()
            if any(indicator in part_lower for indicator in server_indicators) and i > 0:
                default_name = path_parts[i-1]
                break
        
        if not default_name and len(path_parts) > 1:
            default_name = path_parts[-2]
        
        # Ask for server name with suggested default
        server_name = simpledialog.askstring("Add Folder", "Enter server name:", initialvalue=default_name)
        if not server_name:
            return
            
        if server_name in self.saved_folders:
            if not messagebox.askyesno("Server exists", 
                                     f"Server '{server_name}' already exists. Do you want to update its path?"):
                return

        self.saved_folders[server_name] = folder_path
        self.save_saved_folders()
        self.refresh_folder_list()

    def remove_folder(self):
        selected = self.folder_tree.selection()
        if not selected:
            messagebox.showwarning("Warning", "Please select a folder to remove.")
            return

        for item in selected:
            server = self.folder_tree.item(item)['values'][0]
            del self.saved_folders[server]

        self.save_saved_folders()
        self.refresh_folder_list() 

    def update_theme(self):
        """Update theme for the tribe file tab"""
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