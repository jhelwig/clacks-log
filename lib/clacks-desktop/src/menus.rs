//! Application commands shared by native menus and the Linux presentation.

/// Stable identities for existing menu actions, independent of widget IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuCommand {
    /// Open the tag-defined logbook manager.
    ManageLogbooks,
    /// Open reusable stations.
    StationProfiles,
    /// Open reusable equipment.
    EquipmentConfigurations,
    /// Reload committed settings.
    RefreshSettings,
    /// Open enrollment/connection management.
    Connection,
    /// Configure WS Suite reception.
    WsSuite,
    /// Preview local appearance preferences.
    Appearance,
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
            Self::ManageLogbooks => "manage_logbooks",
            Self::StationProfiles => "station_profiles",
            Self::EquipmentConfigurations => "equipment_configurations",
            Self::RefreshSettings => "refresh_settings",
            Self::Connection => "connection",
            Self::WsSuite => "ws_suite",
            Self::Appearance => "appearance",
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

/// Ordered management commands shared by platform presentations.
pub(crate) const GROUPS: &[MenuGroup] = &[
    MenuGroup {
        title: "Settings",
        commands: &[
            Some(MenuCommand::ManageLogbooks),
            Some(MenuCommand::StationProfiles),
            Some(MenuCommand::EquipmentConfigurations),
            Some(MenuCommand::RefreshSettings),
            None,
            Some(MenuCommand::Connection),
            Some(MenuCommand::WsSuite),
            None,
            Some(MenuCommand::Appearance),
        ],
    },
    MenuGroup {
        title: "View",
        commands: &[Some(MenuCommand::ResetLayout)],
    },
    MenuGroup {
        title: "Help",
        commands: &[Some(MenuCommand::Diagnostics)],
    },
];

impl MenuCommand {
    /// Visible command text, including the current enrollment state.
    pub const fn label(self, enrolled: bool) -> &'static str {
        match self {
            Self::ManageLogbooks => "Manage logbooks…",
            Self::StationProfiles => "Station profiles…",
            Self::EquipmentConfigurations => "Equipment configurations…",
            Self::RefreshSettings => "Refresh settings",
            Self::Connection if enrolled => "Connection…",
            Self::Connection => "Connect…",
            Self::WsSuite => "WS Suite…",
            Self::Appearance => "Appearance…",
            Self::ResetLayout => "Reset workbench layout",
            Self::Diagnostics => "Workspace & diagnostics…",
            Self::Quit => "Quit Clacks Log",
        }
    }
}
