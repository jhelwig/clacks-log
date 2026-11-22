//! Application commands shared by native menus and the Linux presentation.

/// Stable identities for existing menu actions, independent of widget IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuCommand {
    /// Select text or all applied history matches, according to actual focus.
    SelectAll,
    /// Populate or refresh separate directory snapshots for selected saved contacts.
    LookupDirectory,
    /// Import contacts from an ADI file.
    ImportAdi,
    /// Export a saved contact scope to ADI files.
    ExportAdi,
    /// Open the unified desktop Settings editor.
    OpenSettings,
    /// Restore workbench pane sizes.
    ResetLayout,
    /// Show workspace identity and log locations.
    Diagnostics,
    /// Request the existing orderly close path.
    Quit,
}

impl MenuCommand {
    /// Stable, bounded native item identifier and metric category.
    pub const fn id(self) -> &'static str {
        match self {
            Self::SelectAll => "select_all",
            Self::LookupDirectory => "lookup_directory",
            Self::ImportAdi => "import_adi",
            Self::ExportAdi => "export_adi",
            Self::OpenSettings => "open_settings",
            Self::ResetLayout => "reset_layout",
            Self::Diagnostics => "diagnostics",
            Self::Quit => "quit",
        }
    }
}

#[cfg(any(target_os = "macos", windows))]
pub(crate) mod native;

/// One native menu and its ordered existing commands.
pub(crate) struct MenuGroup {
    /// Visible top-level menu name.
    pub title: &'static str,
    /// None marks a separator between related commands.
    pub commands: &'static [Option<MenuCommand>],
}

/// File operations shared by all menu presentations.
const FILE: MenuGroup = MenuGroup {
    title: "File",
    commands: &[Some(MenuCommand::ImportAdi), Some(MenuCommand::ExportAdi)],
};

/// Workbench display actions shared by all menu presentations.
const VIEW: MenuGroup = MenuGroup {
    title: "View",
    commands: &[Some(MenuCommand::ResetLayout)],
};

/// Diagnostic actions shared by all menu presentations.
const HELP: MenuGroup = MenuGroup {
    title: "Help",
    commands: &[Some(MenuCommand::Diagnostics)],
};

/// Native window menus following the platform's application menu, if any.
#[cfg(any(target_os = "macos", windows))]
pub(crate) const NATIVE_GROUPS: &[MenuGroup] = &[
    FILE,
    MenuGroup {
        title: "Edit",
        commands: &[
            Some(MenuCommand::SelectAll),
            None,
            Some(MenuCommand::LookupDirectory),
        ],
    },
    VIEW,
    #[cfg(windows)]
    MenuGroup {
        title: "Tools",
        commands: &[Some(MenuCommand::OpenSettings)],
    },
    HELP,
];

/// Append Preferences to Edit in the Linux fallback menu.
#[cfg(any(test, not(any(target_os = "macos", windows))))]
pub(crate) const FALLBACK_GROUPS: &[MenuGroup] = &[
    FILE,
    MenuGroup {
        title: "Edit",
        commands: &[
            Some(MenuCommand::SelectAll),
            None,
            Some(MenuCommand::LookupDirectory),
            None,
            Some(MenuCommand::OpenSettings),
        ],
    },
    VIEW,
    HELP,
];

impl MenuCommand {
    /// Visible command text, independent of enrollment state.
    pub const fn label(self) -> &'static str {
        match self {
            Self::SelectAll => "Select all",
            Self::LookupDirectory => "Look up directory information",
            Self::ImportAdi => "Import ADI…",
            Self::ExportAdi => "Export ADI…",
            #[cfg(windows)]
            Self::OpenSettings => "Options",
            #[cfg(not(windows))]
            Self::OpenSettings => "Settings…",
            Self::ResetLayout => "Reset workbench layout",
            Self::Diagnostics => "Workspace & diagnostics…",
            Self::Quit => "Quit Clacks Log",
        }
    }
}
