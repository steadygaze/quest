/// Code related to verifying user permissions.

/// Mutually exclusive roles related to moderation.
enum AdminRole {
    /// Unprivileged user. May or may not be a questmaster.
    User,
    /// Site-level moderator. Capable of making moderation decisions on any
    /// quest.
    SiteModerator,
    /// Site-level administrator. Elevated permissions compared to other
    /// moderators, including appointing new moderators.
    Administrator,
}

/// Mutually exclusive roles related to
enum TechRole {
    /// Unprivileged user.
    User,
    /// Technical administrator/site operator.
    Operator,
}
