
import tkinter as tk
from tkinter import ttk

class CheckboxWithSymbol(tk.Frame):
    """Custom checkbox widget that shows Unicode checkmark symbols"""
    
    def __init__(self, parent, text="", variable=None, **kwargs):
        super().__init__(parent, **kwargs)
        self.text = text
        self.variable = variable
        
        # Create the checkbox indicator (larger size)
        self.indicator = tk.Label(self, width=3, height=1, relief="raised", borderwidth=2, 
                                 font=("TkDefaultFont", 10, "bold"))
        self.indicator.pack(side="left", padx=(0, 8))
        
        # Create the text label
        self.label = tk.Label(self, text=text, anchor="w", justify="left")
        self.label.pack(side="left", fill="x", expand=True)
        
        # Bind events
        self.indicator.bind("<Button-1>", self._toggle)
        self.label.bind("<Button-1>", self._toggle)
        
        # Update initial state
        self._update_appearance()
        
        # Trace variable changes
        if self.variable:
            self.variable.trace_add("write", lambda *args: self._update_appearance())
    
    def _toggle(self, event=None):
        if self.variable:
            self.variable.set(not self.variable.get())
        self._update_appearance()
    
    def _update_appearance(self):
        if self.variable and self.variable.get():
            # Checked state - show checkmark
            self.indicator.config(text="✓", relief="sunken")
        else:
            # Unchecked state - show empty box
            self.indicator.config(text="", relief="raised")
    
    def configure_colors(self, bg, fg, indicator_bg, indicator_fg, selected_bg, selected_fg):
        """Configure colors for the checkbox"""
        self.configure(bg=bg)
        self.label.configure(bg=bg, fg=fg)
        self.indicator.configure(bg=indicator_bg, fg=indicator_fg)
        
        # Update appearance with new colors
        self._update_appearance()



class ModernTheme:
    """Modern theme management system"""
    COLORS = {
        'light': {
            'bg': '#ffffff',
            'fg': '#2c3e50',
            'button_bg': '#3498db',
            'entry_bg': '#f8f9fa',
            'entry_fg': '#2c3e50',
            'highlight': '#3498db',
            'button_fg': '#000000',
            'secondary_text': '#7f8c8d',
            'border': '#2980b9',
            'hover': '#2980b9',
            'header': '#ecf0f1',
            'success': '#27ae60',
            'warning': '#f39c12',
            'error': '#e74c3c',
            'accent': '#9b59b6',
            'text_primary': '#000000',
            'text_secondary': '#7f8c8d',
            'surface': '#ffffff',
            'surface_variant': '#f8f9fa',
            'dropdown_text': '#2c3e50'
        },
        'dark': {
            'bg': '#1a1a1a',
            'fg': '#ecf0f1',
            'button_bg': '#302f2f',
            'entry_bg': '#2c3e50',
            'entry_fg': '#ecf0f1',
            'highlight': '#3498db',
            'button_fg': '#ffffff',
            'secondary_text': '#bdc3c7',
            'border': '#34495e',
            'hover': '#505050',
            'header': '#2c3e50',
            'success': '#27ae60',
            'warning': '#f39c12',
            'error': '#e74c3c',
            'accent': '#9b59b6',
            'text_primary': '#ecf0f1',
            'text_secondary': '#bdc3c7',
            'surface': '#302f2f',
            'surface_variant': '#34495e',
            'dropdown_text': '#000000'
        }
    }

    @classmethod
    def get_colors(cls, theme):
        return cls.COLORS[theme]
    
    @classmethod
    def _create_checkbox_indicators(cls, style, colors):
        """Create enhanced checkbox indicators"""
        try:
            # Simple, reliable checkbox styling
            style.configure("TCheckbutton",
                background=colors['bg'],
                foreground=colors['text_primary'],
                indicatorcolor=colors['entry_bg'],
                indicatorrelief="raised",
                indicatordiameter=18,
                indicatordepth=3
            )
            
            # Map checkbox states for better visual feedback
            style.map("TCheckbutton",
                background=[('active', colors['bg'])],
                foreground=[('active', colors['text_primary'])],
                indicatorcolor=[
                    ('selected', colors['highlight']),
                    ('active', colors['hover']),
                    ('!active', colors['entry_bg'])
                ],
                indicatorrelief=[
                    ('pressed', 'sunken'),
                    ('selected', 'sunken'),
                    ('!selected', 'raised')
                ]
            )
                
        except Exception as e:
            print(f"Warning: Could not create checkbox indicators: {e}")
            # Fall back to default checkbox styling
    
    @classmethod
    def update_custom_checkboxes(cls, root, colors):
        """Update colors of all CheckboxWithSymbol widgets in the application"""
        def update_checkbox_widgets(widget):
            if isinstance(widget, CheckboxWithSymbol):
                widget.configure_colors(
                    bg=colors['bg'],
                    fg=colors['text_primary'],
                    indicator_bg=colors['entry_bg'],
                    indicator_fg=colors['button_fg'],
                    selected_bg=colors['highlight'],
                    selected_fg=colors['button_fg']
                )
            
            # Recursively update child widgets
            for child in widget.winfo_children():
                update_checkbox_widgets(child)
        
        # Start from the root widget
        update_checkbox_widgets(root)
    

    
    @classmethod
    def apply_theme(cls, root, theme_var):
        """Apply theme to the entire application"""
        colors = cls.get_colors(theme_var.get())
        
        # Configure ttk styles
        style = ttk.Style()
        
        # Clear any existing theme to force refresh
        try:
            style.theme_use('default')
        except:
            pass
        
        # Create custom checkbox indicator with checkmark
        cls._create_checkbox_indicators(style, colors)
        
        # Configure common styles
        style.configure(".",
            background=colors['bg'],
            foreground=colors['fg'],
            fieldbackground=colors['entry_bg'],
            troughcolor=colors['border'],
            selectbackground=colors['highlight'],
            selectforeground=colors['button_fg']
        )
        
        # Configure button styles
        style.configure("TButton",
            background=colors['button_bg'],
            foreground=colors['button_fg'],
            borderwidth=2,
            focuscolor=colors['highlight']
        )
        style.map("TButton",
            background=[('active', colors['hover']), ('pressed', colors['hover'])],
            foreground=[('active', colors['button_fg']), ('pressed', colors['button_fg'])]
        )
        
        # Configure beveled button style for action buttons
        style.configure("Beveled.TButton",
            background=colors['button_bg'],
            foreground=colors['button_fg'],
            borderwidth=3,
            focuscolor=colors['highlight'],
            relief="raised"
        )
        style.map("Beveled.TButton",
            background=[('active', colors['hover']), ('pressed', colors['hover'])],
            foreground=[('active', colors['button_fg']), ('pressed', colors['button_fg'])],
            relief=[('pressed', 'sunken'), ('active', 'raised')]
        )
        
        # Force button colors to override any default styling
        style.layout("TButton", [
            ('Button.focus', {'children': [
                ('Button.padding', {'children': [
                    ('Button.label', {'sticky': 'nswe'})
                ], 'sticky': 'nswe'})
            ], 'sticky': 'nswe'})
        ])
        
        # Configure entry styles
        style.configure("TEntry",
            fieldbackground=colors['entry_bg'],
            foreground=colors['entry_fg'],
            borderwidth=1,
            insertcolor=colors['fg']
        )
        
        # Configure frame styles
        style.configure("TFrame",
            background=colors['bg']
        )
        
        # Configure label styles
        style.configure("TLabel",
            background=colors['bg'],
            foreground=colors['fg']
        )
        
        # Configure notebook styles
        style.configure("TNotebook",
            background=colors['bg'],
            borderwidth=0
        )
        
        # Force tab styling with more specific configuration
        style.configure("TNotebook.Tab",
            background=colors['surface'],
            foreground=colors['text_primary'],
            padding=[10, 5],
            borderwidth=1,
            bordercolor=colors['border'],
            lightcolor=colors['surface'],
            darkcolor=colors['border']
        )
        
        # Map all possible tab states
        style.map("TNotebook.Tab",
            background=[
                ('selected', colors['highlight']), 
                ('active', colors['hover']), 
                ('!active', colors['surface']),
                ('!selected', colors['surface'])
            ],
            foreground=[
                ('selected', colors['button_fg']), 
                ('active', colors['text_primary']), 
                ('!active', colors['text_primary']),
                ('!selected', colors['text_primary'])
            ]
        )
        
        # Force notebook tab colors to override any default styling
        style.layout("TNotebook.Tab", [
            ('Notebook.tab', {'children': [
                ('Notebook.padding', {'children': [
                    ('Notebook.label', {'sticky': 'nswe'})
                ], 'sticky': 'nswe'})
            ], 'sticky': 'nswe'})
        ])
        
        # Configure treeview styles
        style.configure("Treeview",
            background=colors['surface'],
            foreground=colors['text_primary'],
            fieldbackground=colors['surface'],
            borderwidth=1
        )
        style.configure("Treeview.Heading",
            background=colors['header'],
            foreground=colors['text_primary'],
            borderwidth=1
        )
        style.map("Treeview",
            background=[('selected', colors['highlight'])],
            foreground=[('selected', colors['button_fg'])]
        )
        
        # Configure combobox styles
        style.configure("TCombobox",
            fieldbackground=colors['entry_bg'],
            foreground=colors['dropdown_text'],
            background=colors['button_bg'],
            borderwidth=1
        )
        

        
        # Configure scrollbar styles
        style.configure("Vertical.TScrollbar",
            background=colors['border'],
            troughcolor=colors['bg'],
            borderwidth=0,
            arrowcolor=colors['text_secondary']
        )
        
        # Configure separator styles
        style.configure("TSeparator",
            background=colors['border']
        )
        
        # Configure labelframe styles
        style.configure("TLabelframe",
            background=colors['bg'],
            foreground=colors['text_primary'],
            borderwidth=1
        )
        style.configure("TLabelframe.Label",
            background=colors['bg'],
            foreground=colors['text_primary']
        )
        
        # Apply theme to root window
        root.configure(bg=colors['bg'])
        
        # Update custom checkboxes
        cls.update_custom_checkboxes(root, colors)
        
        # Force update of all widgets
        try:
            root.update_idletasks()
            
            # Force refresh of all ttk widgets
            for widget in root.winfo_children():
                try:
                    widget.update_idletasks()
                except:
                    pass
                    
            # Force refresh of notebook specifically
            for widget in root.winfo_children():
                if hasattr(widget, 'winfo_children'):
                    for child in widget.winfo_children():
                        if 'notebook' in str(type(child)).lower():
                            child.update_idletasks()
                            child.update()
        except:
            pass
