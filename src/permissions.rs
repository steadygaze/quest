/// Code related to verifying user permissions.

/// Mutually exclusive roles related to site administration.
enum AdminRole {
    /// Disabled account.
    DISABLED,
    /// Unprivileged user. May or may not be a questmaster.
    USER,
    /// Site-level moderator. Capable of making moderation decisions on any
    /// quest.
    SITE_MODERATOR,
    /// Site-level administrator. Elevated permissions compared to other
    /// moderators, including appointing new moderators.
    ADMINISTRATOR,
    /// Site operator. Complete and total moderator and technical permissions.
    OPERATOR,
}
